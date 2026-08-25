use std::collections::HashMap;
use std::path::PathBuf;

use crate::core::{graph::topology::GraphNodeId, tile_allocator::TileHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey(pub u64); // hash(params, input keys, node_type, node_id)

pub enum NodeState {
    Dirty,
    Processing,
    Cached((CacheKey, Vec<TileHandle>)),
    Baked((CacheKey, PathBuf))
}

/// Represents the evaluation state of a node graph, including cached outputs and processing status.
#[derive(Default)]
pub struct EvalCache {
    states: HashMap<GraphNodeId, NodeState>
}
impl EvalCache {
    pub fn state(&self, id: GraphNodeId) -> &NodeState {
        self.states.get(&id).unwrap_or(&NodeState::Dirty)
    }

    pub fn set(&mut self, id: GraphNodeId, state: NodeState) {
        self.states.insert(id, state);
    }

    pub fn remove(&mut self, id: GraphNodeId) {
        self.states.remove(&id);
    }
}

/// Returns the cache key of a node state if it is cached or baked, otherwise returns None.
pub(super) fn cache_key_of(state: &NodeState) -> Option<CacheKey> {
    match state {
        NodeState::Cached((k, _)) | NodeState::Baked((k, _)) => Some(*k),
        _ => None
    }
}
