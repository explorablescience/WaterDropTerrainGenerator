use std::sync::{Arc, RwLock};
use std::time::Duration;
use wde::prelude::*;

use bevy::{platform::collections::HashMap, prelude::*};

use crate::core::{
    graph::{GraphNodeId, NodeGraph, NodeGraphProcessResult},
    node::{NodeError, NodeMessage, NodeMessageLog},
    tiling::{ChunkCoord, ChunkGrid, TileHandle}
};

/// A thread-safe wrapper around [`TerrainSession`] that can be stored as a Bevy resource.
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

/// The state of the node graph and its cached output tiles for the current session (see [`TerrainSessionHolder`]).
pub struct TerrainSession {
    graph: NodeGraph,
    chunk_generations: HashMap<(GraphNodeId, ChunkCoord), u32>,
    displayed_node: Option<GraphNodeId>,
    pub selected_node: Option<GraphNodeId>,
    messages: NodeMessageLog
}
impl Default for TerrainSession {
    fn default() -> Self {
        Self {
            graph: NodeGraph::new(ChunkGrid::new(2, 2, TILE_RESOLUTIONS[2], 0.1)),
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
    pub fn action_message(&self, node_id: GraphNodeId) -> Option<&NodeMessage> {
        self.messages.get(node_id)
    }
    pub fn action_message_remaining(&self, node_id: GraphNodeId) -> Option<Duration> {
        self.messages.remaining(node_id)
    }
    pub fn prune_expired_messages(&mut self) {
        self.messages.prune_expired();
    }

    /// Processes synchronously the output of `node_id` for `chunk`, returning the new generation and output tiles if the chunk was actually recomputed. If `force` is true, the chunk will be considered recomputed even if its generation hasn't advanced.
    pub fn process_sync(
        &mut self,
        node_id: GraphNodeId,
        chunk: ChunkCoord
    ) -> Result<Option<(u32, Vec<TileHandle>)>, NodeError> {
        let _span = debug_span!("process_chunk", node_id = ?node_id, chunk = ?chunk).entered();
        let result = self.graph.process_chunk(node_id, chunk);
        self.apply_chunk_result(node_id, chunk, result)
    }
    /// Applies the result of a chunk processing operation, updating the internal state and returning the new generation and output tiles if the chunk was actually recomputed. If `force` is true, the chunk will be considered recomputed even if its generation hasn't advanced.
    pub fn apply_chunk_result(
        &mut self,
        node_id: GraphNodeId,
        chunk: ChunkCoord,
        result: Result<NodeGraphProcessResult, NodeError>
    ) -> Result<Option<(u32, Vec<TileHandle>)>, NodeError> {
        let _span = debug_span!("apply_chunk_result", node_id = ?node_id, chunk = ?chunk).entered();
        let key = (node_id, chunk);
        match result? {
            NodeGraphProcessResult::Processed(new_generation, output_tiles) => {
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

    /// Replaces the whole graph (e.g. after loading a project), discarding all per-chunk
    /// generation bookkeeping, selection and messages tied to the previous graph's node ids.
    pub fn reset_graph(&mut self, graph: NodeGraph) {
        self.graph = graph;
        self.chunk_generations.clear();
        self.displayed_node = None;
        self.selected_node = None;
        self.messages = NodeMessageLog::default();
    }
}
