//! The live terrain project: a bevy `Resource` wrapping [`NodeGraph`] with the preview/generation
//! bookkeeping, whole-terrain export, and persistence that turn the generic graph engine into
//! *this app's* concept of a terrain project.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use bevy::{platform::collections::HashMap, prelude::*};

use crate::core::{
    graph::{GraphNodeId, NodeGraph, NodeGraphProcessResult},
    node::{NodeError, NodeMessage, NodeMessageLog},
    tiling::{ChunkCoord, ChunkGrid, TileHandle, save_heightmap_png}
};

mod export;
mod jobs;
mod project;

pub use export::assemble_terrain;
pub use jobs::ChunkJobs;
pub use project::{BuiltGraph, load_project, save_project};

#[derive(Resource, Default, Clone)]
pub struct TerrainSessionHolder(pub Arc<RwLock<TerrainSession>>);
impl TerrainSessionHolder {
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, TerrainSession> {
        self.0.read().unwrap()
    }

    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, TerrainSession> {
        self.0.write().unwrap()
    }
}

/// Engine tile resolution is picked from this fixed set of power-of-two texel sizes (up to the engine's 4096 cap) rather than typed freely.
pub const TILE_RESOLUTIONS: &[usize] = &[64, 128, 256, 512, 1024, 2048, 4096];

pub struct TerrainSession {
    graph: NodeGraph,
    generations: HashMap<GraphNodeId, u32>,
    /// Like `generations`, but keyed per chunk for a chunked preview (see [`Self::process_chunk`]).
    chunk_generations: HashMap<(GraphNodeId, ChunkCoord), u32>,
    /// Forces a refresh when the selection changes even if the newly selected node's generation hasn't advanced.
    displayed_node: Option<GraphNodeId>,
    pub selected_node: Option<GraphNodeId>,
    /// Feedback from the most recent `on_action`/`set_param` call on each node.
    messages: NodeMessageLog
}
impl Default for TerrainSession {
    fn default() -> Self {
        Self {
            graph: NodeGraph::new(ChunkGrid::new(2, 2, TILE_RESOLUTIONS[2], 0.1)),
            generations: HashMap::new(),
            chunk_generations: HashMap::new(),
            displayed_node: None,
            selected_node: None,
            messages: NodeMessageLog::default()
        }
    }
}
impl TerrainSession {
    /// Replaces whatever feedback was shown before for `node_id`.
    pub fn set_action_result(&mut self, node_id: GraphNodeId, result: Result<String, NodeError>) {
        self.messages.set_result(node_id, result);
    }

    pub fn clear_action_message(&mut self, node_id: GraphNodeId) {
        self.messages.clear(node_id);
    }

    /// Filters out feedback that has already expired.
    pub fn action_message(&self, node_id: GraphNodeId) -> Option<&NodeMessage> {
        self.messages.get(node_id)
    }

    /// `None` unless the feedback is timed and still live.
    pub fn action_message_remaining(&self, node_id: GraphNodeId) -> Option<Duration> {
        self.messages.remaining(node_id)
    }

    pub fn prune_expired_messages(&mut self) {
        self.messages.prune_expired();
    }

    /// Returns the new output tiles if a new generation is available since the last call, `Ok(None)` if not (or still processing).
    pub fn process(
        &mut self,
        node_id: GraphNodeId
    ) -> Result<Option<(u32, Vec<TileHandle>)>, NodeError> {
        // Reselecting an already-cached, unchanged node must still redisplay it, or the previous selection's mesh would stay on screen.
        let just_selected = self.displayed_node != Some(node_id);
        let generation = self.generations.get(&node_id).copied().unwrap_or(0);
        match self.graph.process(node_id)? {
            NodeGraphProcessResult::Processed(new_generation, output_tiles) => {
                if !just_selected && new_generation <= generation {
                    return Ok(None);
                }

                self.generations.insert(node_id, new_generation);
                self.displayed_node = Some(node_id);
                Ok(Some((new_generation, output_tiles)))
            }
            NodeGraphProcessResult::Processing => Ok(None)
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
        let result = self.graph.process_chunk(node_id, chunk);
        self.apply_chunk_result(node_id, chunk, force, result)
    }

    /// Applies an already-computed chunk `result` to the "is this new" bookkeeping - the
    /// counterpart of [`Self::process_chunk`] for callers that computed the result themselves
    /// (e.g. [`ChunkJobs::poll_or_spawn`]).
    pub fn apply_chunk_result(
        &mut self,
        node_id: GraphNodeId,
        chunk: ChunkCoord,
        force: bool,
        result: Result<NodeGraphProcessResult, NodeError>
    ) -> Result<Option<(u32, Vec<TileHandle>)>, NodeError> {
        let key = (node_id, chunk);
        let generation = self.chunk_generations.get(&key).copied().unwrap_or(0);
        match result? {
            NodeGraphProcessResult::Processed(new_generation, output_tiles) => {
                if !force && new_generation <= generation {
                    return Ok(None);
                }

                self.chunk_generations.insert(key, new_generation);
                Ok(Some((new_generation, output_tiles)))
            }
            NodeGraphProcessResult::Processing => Ok(None)
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
        save_heightmap_png(&data, width, height, path).map_err(NodeError::from)
    }
}
