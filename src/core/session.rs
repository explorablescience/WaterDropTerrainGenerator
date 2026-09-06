use std::sync::{Arc, RwLock};
use std::time::Duration;

use bevy::prelude::*;

use crate::core::*;

/// Engine tile resolution is picked from this fixed set of power-of-two texel sizes (up to the engine's 4096 cap) rather than typed freely.
pub const TILE_RESOLUTIONS: &[usize] = &[64, 128, 256, 512, 1024, 2048, 4096];

/// A thread-safe wrapper around [`TerrainInstance`] that can be stored as a Bevy resource.
#[derive(Resource, Default, Clone)]
pub struct TerrainInstanceHolder(pub Arc<RwLock<TerrainInstance>>);
impl TerrainInstanceHolder {
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, TerrainInstance> {
        self.0.read().unwrap()
    }

    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, TerrainInstance> {
        self.0.write().unwrap()
    }
}

/// A single terrain instance, including its node graph, selected node, and per-node messages.
/// This is the core data structure that the editor and engine operate on.
pub struct TerrainInstance {
    graph: NodeGraph,
    selected_node: Option<GraphNodeId>,
    messages: NodeMessageLog,
}
impl Default for TerrainInstance {
    fn default() -> Self {
        Self {
            graph: NodeGraph::new(ChunkGrid::new(1, 1, TILE_RESOLUTIONS[5], 0.01)),
            selected_node: None,
            messages: NodeMessageLog::default(),
        }
    }
}
impl TerrainInstance {
    // Messages management
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
    pub fn drop_expired_messages(&mut self) {
        self.messages.drop_expired();
    }

    // Graph accessors
    pub fn graph(&self) -> &NodeGraph {
        &self.graph
    }
    pub fn graph_mut(&mut self) -> &mut NodeGraph {
        &mut self.graph
    }
    /// Resets the entire cache except for the node graph itself, which is preserved.
    /// Used when loading a new project or creating a new one.
    pub fn reset_graph(&mut self, graph: NodeGraph) {
        self.graph = graph;
        self.selected_node = None;
        self.messages = NodeMessageLog::default();
    }

    /// Gets the cached output of a node, processing it if necessary.
    /// Returns Ok(CacheEntry), Ok(None) if the node is being processed, or Err(NodeError) if processing failed.
    pub fn get(&mut self, node_id: GraphNodeId) -> Result<Option<CacheEntry>, NodeError> {
        self.graph.get(node_id)
    }

    // Selected node accessors
    pub fn selected_node(&self) -> Option<GraphNodeId> {
        self.selected_node
    }
    pub fn set_selected_node(&mut self, node_id: Option<GraphNodeId>) {
        self.selected_node = node_id;
    }
}
