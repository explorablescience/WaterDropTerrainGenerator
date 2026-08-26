//! Library crate for the terrain generator.

use std::sync::{Arc, RwLock};

/// Whether WaterDropEngine's own built-in UI menu items (e.g. "Engine/*", "Camera/*", "PBR/*") are shown.
pub const DEBUG_MODE: bool = false;
use std::time::Duration;

use bevy::{platform::collections::HashMap, prelude::*};

use crate::core::{
    chunk_grid::ChunkCoord,
    graph::{GraphNodeId, NodeGraph, NodeGraphProcessResult},
    node_error::NodeError,
    node_message::{MessageLifetime, NodeMessage, TimedNodeMessage},
    terrain_export::assemble_terrain,
    tile_allocator::TileHandle
};

pub mod core;
pub mod nodes;
pub mod render;
pub mod ui;

/// Resource that holds the terrain graph and its state.
#[derive(Resource, Default, Clone)]
pub struct TerrainGraphHolder(pub Arc<RwLock<TerrainGraph>>);
impl TerrainGraphHolder {
    /// Returns a read-only reference to the terrain graph.
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, TerrainGraph> {
        self.0.read().unwrap()
    }

    /// Returns a mutable reference to the terrain graph.
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, TerrainGraph> {
        self.0.write().unwrap()
    }
}

/// Represents the terrain graph, which consists of a node graph and its associated state.
pub struct TerrainGraph {
    /// The underlying node graph that defines the terrain generation process.
    graph: NodeGraph,
    /// Mapping from a node's unique id to its latest resulting generation.
    generations: HashMap<GraphNodeId, u32>,
    /// Like `generations`, but keyed per chunk for a chunked preview (see [`Self::process_chunk`]).
    chunk_generations: HashMap<(GraphNodeId, ChunkCoord), u32>,
    /// The node whose output the preview last displayed, if any. Used to force a refresh when
    /// the selection changes even if the newly selected node's generation hasn't advanced.
    displayed_node: Option<GraphNodeId>,
    /// The currently selected node in the graph editor, if any.
    pub selected_node: Option<GraphNodeId>,
    /// Feedback from the most recent `on_action`/`set_param` call on each node: an error persists
    /// until the next call on that node, while a success confirmation fades out on its own.
    action_messages: HashMap<GraphNodeId, TimedNodeMessage>
}
impl Default for TerrainGraph {
    fn default() -> Self {
        Self {
            graph: NodeGraph::new(128),
            generations: HashMap::new(),
            chunk_generations: HashMap::new(),
            displayed_node: None,
            selected_node: None,
            action_messages: HashMap::new()
        }
    }
}
impl TerrainGraph {
    /// How long a success confirmation from `on_action` stays visible before it fades out.
    pub const ACTION_MESSAGE_DURATION: Duration = Duration::from_secs(3);

    /// Records the feedback of an `on_action`/`set_param` call on `node_id`, replacing whatever
    /// was shown before it. `Ok` carries the confirmation text to show (fades out on its own);
    /// `Err` carries the failure to show (persists until the next call on this node).
    pub fn set_action_result(&mut self, node_id: GraphNodeId, result: Result<String, NodeError>) {
        let timed = match result {
            Ok(text) => TimedNodeMessage::new(
                NodeMessage::info(text),
                MessageLifetime::Timed(Self::ACTION_MESSAGE_DURATION)
            ),
            Err(err) => TimedNodeMessage::new(
                NodeMessage { severity: err.severity(), text: err.to_string() },
                MessageLifetime::Persistent
            )
        };
        self.action_messages.insert(node_id, timed);
    }

    /// Drops any action feedback currently shown for `node_id`, e.g. after an action succeeds
    /// without a confirmation message of its own, so a stale error from a previous attempt
    /// doesn't linger.
    pub fn clear_action_message(&mut self, node_id: GraphNodeId) {
        self.action_messages.remove(&node_id);
    }

    /// The still-live action feedback for `node_id`, if any.
    pub fn action_message(&self, node_id: GraphNodeId) -> Option<&NodeMessage> {
        self.action_messages
            .get(&node_id)
            .filter(|m| !m.is_expired())
            .map(|m| &m.message)
    }

    /// Time left before `node_id`'s action feedback expires on its own, if it's timed and still
    /// live. Used to schedule a repaint so the UI updates the moment the message should disappear.
    pub fn action_message_remaining(&self, node_id: GraphNodeId) -> Option<Duration> {
        self.action_messages.get(&node_id).and_then(TimedNodeMessage::remaining)
    }

    /// Drops any action feedback that has expired, so it doesn't linger in memory forever.
    pub fn prune_expired_messages(&mut self) {
        self.action_messages.retain(|_, m| !m.is_expired());
    }

    /// Processes the terrain graph starting from the specified node ID.
    /// Each instance of a node graph contains an internal state that tracks the latest generation of output tiles for each node.
    ///
    /// # Returns
    /// - `Ok(Some((generation, output_tiles)))` if the graph has been processed successfully and new output tiles are available since the last processing.
    /// - `Ok(None)` if the graph is still processing or if no new output tiles are available since the last processing.
    /// - `Err(NodeError)` if an error occurs during processing.
    pub fn process(
        &mut self,
        node_id: GraphNodeId
    ) -> Result<Option<(u32, Vec<TileHandle>)>, NodeError> {
        // A node whose output isn't the one currently shown must be (re)displayed even if its
        // own generation hasn't advanced - otherwise reselecting an already-cached node that
        // hasn't changed since it was last shown would leave the previous selection's mesh on
        // screen.
        let just_selected = self.displayed_node != Some(node_id);
        let generation = self.generations.get(&node_id).copied().unwrap_or(0);
        match self.graph.process(node_id) {
            Ok(NodeGraphProcessResult::Processed(new_generation, output_tiles)) => {
                if !just_selected && new_generation <= generation {
                    // No new generation is available
                    return Ok(None);
                }

                // New generation is available, or the selection just changed to this node
                self.generations.insert(node_id, new_generation);
                self.displayed_node = Some(node_id);
                Ok(Some((new_generation, output_tiles)))
            }
            Ok(NodeGraphProcessResult::Processing) => {
                // Graph is still processing
                Ok(None)
            }
            Err(e) => Err(e)
        }
    }

    /// Whether `node_id` differs from the node the preview last displayed. Reports `true` at most
    /// once per selection change - calling it marks `node_id` as now displayed - so the caller can
    /// use it to force every chunk of a newly selected node to redraw once, even one whose own
    /// cached generation hasn't advanced (e.g. reselecting a node that was already computed before
    /// the selection moved away from it).
    pub fn note_selection(&mut self, node_id: GraphNodeId) -> bool {
        let just_selected = self.displayed_node != Some(node_id);
        self.displayed_node = Some(node_id);
        just_selected
    }

    /// Like [`Self::process`], but for one chunk of a chunked preview: tracks generations per
    /// `(node, chunk)` pair instead of per node, so every chunk gets its own "is this new"
    /// bookkeeping. `force` bypasses that check - pass the result of [`Self::note_selection`].
    ///
    /// # Returns
    /// Same contract as [`Self::process`], scoped to `chunk`.
    pub fn process_chunk(
        &mut self,
        node_id: GraphNodeId,
        chunk: ChunkCoord,
        force: bool
    ) -> Result<Option<(u32, Vec<TileHandle>)>, NodeError> {
        let key = (node_id, chunk);
        let generation = self.chunk_generations.get(&key).copied().unwrap_or(0);
        match self.graph.process_chunk(node_id, chunk) {
            Ok(NodeGraphProcessResult::Processed(new_generation, output_tiles)) => {
                if !force && new_generation <= generation {
                    // No new generation is available
                    return Ok(None);
                }

                self.chunk_generations.insert(key, new_generation);
                Ok(Some((new_generation, output_tiles)))
            }
            Ok(NodeGraphProcessResult::Processing) => {
                // Graph is still processing
                Ok(None)
            }
            Err(e) => Err(e)
        }
    }

    pub fn graph(&self) -> &NodeGraph {
        &self.graph
    }
    pub fn graph_mut(&mut self) -> &mut NodeGraph {
        &mut self.graph
    }

    /// Evaluates every chunk of the graph for `node_id` and stitches the results into one PNG
    /// covering the whole terrain, saved to `path`. A one-shot export, unlike `process`'s
    /// per-chunk preview: it isn't cached against the preview's "new generation" bookkeeping.
    pub fn export_stitched_png(&mut self, node_id: GraphNodeId, path: &std::path::Path) -> Result<(), NodeError> {
        let (data, width, height) = assemble_terrain(&mut self.graph, node_id)?;

        let mut img = image::GrayImage::new(width as u32, height as u32);
        for (pixel, &value) in img.pixels_mut().zip(data.iter()) {
            pixel.0 = [(value.clamp(0.0, 1.0) * 255.0).round() as u8];
        }

        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("Failed to create directory '{}': {}", dir.display(), e))?;
        }
        img.save(path)
            .map_err(|e| format!("Failed to save '{}': {}", path.display(), e).into())
    }
}
