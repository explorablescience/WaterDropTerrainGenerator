use std::{collections::HashMap, sync::Arc};

use crate::core::{TileBuffer, graph::GraphNodeId};

type CacheUuid = u64;
enum CacheState {
    Dirty,
    Processing,
    Cached((CacheUuid, Arc<TileBuffer>)),
}
pub type CacheEntry = (CacheUuid, Arc<TileBuffer>);


#[derive(Default)]
pub struct Cache {
    last_uuid: CacheUuid,
    entries: HashMap<GraphNodeId, CacheState>,
}
impl Cache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_dirty(&mut self, node_id: GraphNodeId) {
        self.entries.insert(node_id, CacheState::Dirty);
    }
    pub fn mark_processing(&mut self, node_id: GraphNodeId) {
        self.entries.insert(node_id, CacheState::Processing);
    }
    pub fn is_dirty(&self, node_id: GraphNodeId) -> bool {
        matches!(self.entries.get(&node_id), Some(CacheState::Dirty))
    }

    /// Returns the cached entry for the given node id, if it exists and is not dirty. The returned entry is a pair of (unique id, tile buffer), where the unique id can be used to determine if the cached entry is still valid or changed since it was last used.
    pub fn get(&self, node_id: GraphNodeId) -> Option<CacheEntry> {
        match self.entries.get(&node_id) {
            Some(CacheState::Dirty) => None,
            Some(CacheState::Processing) => None,
            Some(CacheState::Cached(buffer)) => Some(buffer.clone()),
            None => None,
        }
    }
    pub fn set(&mut self, node_id: GraphNodeId, buffer: Arc<TileBuffer>) {
        self.last_uuid += 1;
        self.entries.insert(node_id, CacheState::Cached((self.last_uuid, buffer)));
    }
}
