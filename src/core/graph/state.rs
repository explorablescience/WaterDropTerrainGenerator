use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::core::{
    graph::topology::GraphNodeId,
    tiling::{ChunkCoord, TileHandle}
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey(pub u64); // hash(params, input keys, node_type, node_id)

/// A specific chunk, or the single whole-terrain pass used to evaluate a `Global` node (and any `Local` node pulled into that pass as an input).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvalScope {
    Chunk(ChunkCoord),
    Global
}

pub enum NodeState {
    Dirty,
    Processing,
    Cached((CacheKey, Vec<TileHandle>)),
    Baked((CacheKey, PathBuf))
}

/// A node's cached output differs per [`EvalScope`], since the same `Local` node produces a different tile for each chunk it's evaluated for.
#[derive(Default)]
pub struct EvalCache {
    states: HashMap<(GraphNodeId, EvalScope), NodeState>
}
impl EvalCache {
    pub fn state(&self, id: GraphNodeId, scope: EvalScope) -> &NodeState {
        self.states.get(&(id, scope)).unwrap_or(&NodeState::Dirty)
    }

    pub fn set(&mut self, id: GraphNodeId, scope: EvalScope, state: NodeState) {
        self.states.insert((id, scope), state);
    }

    /// Drops every cached scope of `id` (every chunk's output, plus its global one if any).
    pub fn remove(&mut self, id: GraphNodeId) {
        self.states.retain(|(node_id, _), _| *node_id != id);
    }

    /// Whether `id` has a `Baked` entry in any scope - baked nodes are opaque to invalidation.
    pub fn is_baked(&self, id: GraphNodeId) -> bool {
        self.states
            .iter()
            .any(|((node_id, _), state)| *node_id == id && matches!(state, NodeState::Baked(_)))
    }

    /// Used when parameters or connections change, since that invalidates output for every chunk as well as the global pass.
    pub fn mark_all_dirty(&mut self, id: GraphNodeId) {
        for (_, state) in self
            .states
            .iter_mut()
            .filter(|((node_id, _), _)| *node_id == id)
        {
            *state = NodeState::Dirty;
        }
    }

    /// `Global`-scoped entries live in their own pools, unrelated to the chunk pool, so they're left untouched.
    pub fn clear_chunk_states(&mut self) {
        self.states
            .retain(|(_, scope), _| matches!(scope, EvalScope::Global));
    }

    /// Total heap bytes currently held by every distinct cached tile, across every node and every scope
    pub fn cached_bytes(&self) -> usize {
        let mut seen = std::collections::HashSet::new();
        let mut total = 0usize;
        for state in self.states.values() {
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
