use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::core::TileHandle;
use crate::core::graph::GraphNodeId;
use crate::core::tiling::ChunkCoord;

pub type CacheUuid = u64;

/// Resolution is part of the key so a `Global` node re-evaluated at a new `native_resolution`
/// can't reuse a stale-sized entry (`mark_dirty` never reaches ancestors, only descendants).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvalScope {
    Chunk(ChunkCoord),
    Global(usize)
}

enum CacheState {
    Processing,
    Cached(CacheUuid, Vec<TileHandle>)
}

/// `Local` bundles every chunk; `Global` is the node's single pass.
pub enum CacheEntry {
    Local(HashMap<ChunkCoord, (CacheUuid, Vec<TileHandle>)>),
    Global(CacheUuid, Vec<TileHandle>)
}

/// A missing entry means dirty; there's no explicit `Dirty` state.
#[derive(Default)]
pub struct Cache {
    last_uuid: CacheUuid,
    entries: HashMap<(GraphNodeId, EvalScope), CacheState>
}
impl Cache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_processing(&mut self, node_id: GraphNodeId, scope: EvalScope) {
        self.entries
            .insert((node_id, scope), CacheState::Processing);
    }
    pub fn is_processing(&self, node_id: GraphNodeId, scope: EvalScope) -> bool {
        matches!(
            self.entries.get(&(node_id, scope)),
            Some(CacheState::Processing)
        )
    }

    /// Drops every scope of `node_id`.
    pub fn mark_dirty(&mut self, node_id: GraphNodeId) {
        self.entries.retain(|(id, _), _| *id != node_id);
    }

    pub fn get(
        &self,
        node_id: GraphNodeId,
        scope: EvalScope
    ) -> Option<(CacheUuid, Vec<TileHandle>)> {
        match self.entries.get(&(node_id, scope)) {
            Some(CacheState::Cached(uuid, tiles)) => Some((*uuid, tiles.clone())),
            _ => None
        }
    }
    pub fn set(
        &mut self,
        node_id: GraphNodeId,
        scope: EvalScope,
        tiles: Vec<TileHandle>
    ) -> CacheUuid {
        self.last_uuid += 1;
        self.entries
            .insert((node_id, scope), CacheState::Cached(self.last_uuid, tiles));
        self.last_uuid
    }

    /// Drops every `Chunk`-scoped entry graph-wide; `Global`-scoped ones are untouched.
    pub fn clear_all_chunks(&mut self) {
        self.entries
            .retain(|(_, scope), _| matches!(scope, EvalScope::Global(_)));
    }

    /// Heap bytes held by every distinct cached tile.
    pub fn allocated_bytes(&self) -> usize {
        let mut seen = HashSet::new();
        self.entries
            .values()
            .filter_map(|state| match state {
                CacheState::Cached(_, tiles) => Some(tiles),
                CacheState::Processing => None
            })
            .flatten()
            .filter(|tile| seen.insert(Arc::as_ptr(tile)))
            .map(|tile| tile.size() * tile.size() * std::mem::size_of::<f32>())
            .sum()
    }
}
