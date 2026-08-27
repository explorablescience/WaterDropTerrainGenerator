use std::sync::{Arc, RwLock};
use std::time::Duration;

use bevy::{platform::collections::HashMap, prelude::*};

use crate::core::{
    chunk_grid::{ChunkCoord, ChunkGrid},
    graph::{GraphNodeId, NodeGraph, NodeGraphProcessResult},
    node_error::NodeError,
    node_message::{MessageLifetime, NodeMessage, TimedNodeMessage},
    terrain_export::assemble_terrain,
    tile_allocator::TileHandle
};

#[derive(Resource, Default, Clone)]
pub struct TerrainGraphHolder(pub Arc<RwLock<TerrainGraph>>);
impl TerrainGraphHolder {
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, TerrainGraph> {
        self.0.read().unwrap()
    }

    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, TerrainGraph> {
        self.0.write().unwrap()
    }
}

pub struct TerrainGraph {
    graph: NodeGraph,
    generations: HashMap<GraphNodeId, u32>,
    /// Like `generations`, but keyed per chunk for a chunked preview (see [`Self::process_chunk`]).
    chunk_generations: HashMap<(GraphNodeId, ChunkCoord), u32>,
    /// Forces a refresh when the selection changes even if the newly selected node's generation hasn't advanced.
    displayed_node: Option<GraphNodeId>,
    pub selected_node: Option<GraphNodeId>,
    /// An error persists until the next `on_action`/`set_param` call on that node; a success confirmation fades out on its own.
    action_messages: HashMap<GraphNodeId, TimedNodeMessage>
}
impl Default for TerrainGraph {
    fn default() -> Self {
        Self {
            graph: NodeGraph::new(ChunkGrid::new(4, 4, 128, 1.0 / 128.0)),
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

    /// Replaces whatever feedback was shown before for `node_id`.
    pub fn set_action_result(&mut self, node_id: GraphNodeId, result: Result<String, NodeError>) {
        let timed = match result {
            Ok(text) => TimedNodeMessage::new(
                NodeMessage::info(text),
                MessageLifetime::Timed(Self::ACTION_MESSAGE_DURATION)
            ),
            Err(err) => TimedNodeMessage::new(
                NodeMessage {
                    severity: err.severity(),
                    text: err.to_string()
                },
                MessageLifetime::Persistent
            )
        };
        self.action_messages.insert(node_id, timed);
    }

    pub fn clear_action_message(&mut self, node_id: GraphNodeId) {
        self.action_messages.remove(&node_id);
    }

    /// Filters out feedback that has already expired.
    pub fn action_message(&self, node_id: GraphNodeId) -> Option<&NodeMessage> {
        self.action_messages
            .get(&node_id)
            .filter(|m| !m.is_expired())
            .map(|m| &m.message)
    }

    /// `None` unless the feedback is timed and still live.
    pub fn action_message_remaining(&self, node_id: GraphNodeId) -> Option<Duration> {
        self.action_messages
            .get(&node_id)
            .and_then(TimedNodeMessage::remaining)
    }

    pub fn prune_expired_messages(&mut self) {
        self.action_messages.retain(|_, m| !m.is_expired());
    }

    /// Returns the new output tiles if a new generation is available since the last call, `Ok(None)` if not (or still processing).
    pub fn process(
        &mut self,
        node_id: GraphNodeId
    ) -> Result<Option<(u32, Vec<TileHandle>)>, NodeError> {
        // Reselecting an already-cached, unchanged node must still redisplay it, or the previous selection's mesh would stay on screen.
        let just_selected = self.displayed_node != Some(node_id);
        let generation = self.generations.get(&node_id).copied().unwrap_or(0);
        match self.graph.process(node_id) {
            Ok(NodeGraphProcessResult::Processed(new_generation, output_tiles)) => {
                if !just_selected && new_generation <= generation {
                    return Ok(None);
                }

                self.generations.insert(node_id, new_generation);
                self.displayed_node = Some(node_id);
                Ok(Some((new_generation, output_tiles)))
            }
            Ok(NodeGraphProcessResult::Processing) => Ok(None),
            Err(e) => Err(e)
        }
    }

    /// Reports `true` at most once per selection change - calling it marks `node_id` as now displayed - so the caller can force every chunk of a newly selected node to redraw once.
    pub fn note_selection(&mut self, node_id: GraphNodeId) -> bool {
        let just_selected = self.displayed_node != Some(node_id);
        self.displayed_node = Some(node_id);
        just_selected
    }

    /// `force` bypasses the "is this new" check - pass the result of [`Self::note_selection`].
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
                    return Ok(None);
                }

                self.chunk_generations.insert(key, new_generation);
                Ok(Some((new_generation, output_tiles)))
            }
            Ok(NodeGraphProcessResult::Processing) => Ok(None),
            Err(e) => Err(e)
        }
    }

    pub fn graph(&self) -> &NodeGraph {
        &self.graph
    }
    pub fn graph_mut(&mut self) -> &mut NodeGraph {
        &mut self.graph
    }

    /// Unlike `process`, this isn't cached against the preview's "new generation" bookkeeping.
    pub fn export_stitched_png(
        &mut self,
        node_id: GraphNodeId,
        path: &std::path::Path
    ) -> Result<(), NodeError> {
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
