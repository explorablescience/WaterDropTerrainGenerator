use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::core::{
    graph::topology::GraphNodeId,
    tiling::{ChunkCoord, TileHandle}
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey(pub u64); // hash(params, input keys, node_type, node_id)

/// A specific chunk, or the single whole-terrain pass (at a given `native_resolution`) used to
/// evaluate a `Global` node. Each scope has its own cache entry for every node, since the same `Local` node produces a different tile for each chunk it's evaluated for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvalScope {
    Chunk(ChunkCoord),
    Global(usize)
}

#[derive(Clone)]
pub enum NodeState {
    Dirty,
    Processing,
    Cached((CacheKey, Vec<TileHandle>)),
    Baked((CacheKey, PathBuf))
}

/// A node's cached output differs per [`EvalScope`], since the same `Local` node produces a
/// different tile for each chunk it's evaluated for.
#[derive(Default)]
pub struct EvalCache {
    states: Mutex<HashMap<(GraphNodeId, EvalScope), NodeState>>
}
impl EvalCache {
    pub fn state(&self, id: GraphNodeId, scope: EvalScope) -> NodeState {
        self.states
            .lock()
            .unwrap()
            .get(&(id, scope))
            .cloned()
            .unwrap_or(NodeState::Dirty)
    }

    pub fn set(&self, id: GraphNodeId, scope: EvalScope, state: NodeState) {
        self.states.lock().unwrap().insert((id, scope), state);
    }

    /// Drops every cached scope of `id` (every chunk's output, plus its global one if any).
    pub fn remove(&self, id: GraphNodeId) {
        self.states
            .lock()
            .unwrap()
            .retain(|(node_id, _), _| *node_id != id);
    }

    /// Whether `id` has a `Baked` entry in any scope - baked nodes are opaque to invalidation.
    pub fn is_baked(&self, id: GraphNodeId) -> bool {
        self.states
            .lock()
            .unwrap()
            .iter()
            .any(|((node_id, _), state)| *node_id == id && matches!(state, NodeState::Baked(_)))
    }

    /// Used when parameters or connections change, since that invalidates output for every chunk as well as the global pass.
    pub fn mark_all_dirty(&self, id: GraphNodeId) {
        for (_, state) in self
            .states
            .lock()
            .unwrap()
            .iter_mut()
            .filter(|((node_id, _), _)| *node_id == id)
        {
            *state = NodeState::Dirty;
        }
    }
    pub fn is_dirty(&self, id: GraphNodeId) -> bool {
        self.states
            .lock()
            .unwrap()
            .iter()
            .any(|((node_id, _), state)| *node_id == id && matches!(state, NodeState::Dirty))
    }

    /// `Global`-scoped entries live in their own pools, unrelated to the chunk pool, so they're left untouched.
    pub fn clear_chunk_states(&self) {
        self.states
            .lock()
            .unwrap()
            .retain(|(_, scope), _| matches!(scope, EvalScope::Global(_)));
    }

    /// Total heap bytes currently held by every distinct cached tile, across every node and every scope
    pub fn cached_bytes(&self) -> usize {
        let states = self.states.lock().unwrap();
        let mut seen = std::collections::HashSet::new();
        let mut total = 0usize;
        for state in states.values() {
            let NodeState::Cached((_, tiles)) = state else {
                continue;
            };
            for tile in tiles {
                if seen.insert(Arc::as_ptr(tile)) {
                    total += tile.size() * tile.size() * std::mem::size_of::<f32>();
                }
            }
        }
        total
    }
}

pub(super) fn cache_key_of(state: &NodeState) -> Option<CacheKey> {
    match state {
        NodeState::Cached((k, _)) | NodeState::Baked((k, _)) => Some(*k),
        _ => None
    }
}
