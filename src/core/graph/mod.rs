//! The graph engine connected to the node system, which handles evaluation, caching, and chunk processing.

use std::collections::HashSet;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::core::node::{Node, NodeError};
use crate::core::tiling::{ChunkGrid, TileHandle, TilePool};

mod eval;
mod state;
mod topology;

pub use state::{CacheKey, NodeState};
pub use topology::GraphNodeId;

use state::EvalCache;
use topology::Topology;

pub enum NodeGraphProcessResult {
    Processed(u32, Vec<TileHandle>),
    Processing
}

pub struct NodeGraph {
    pool: Arc<TilePool>,
    chunk_grid: ChunkGrid,
    topology: Topology,
    cache: EvalCache,
    generation: AtomicU32,
    last_activity: Mutex<Option<Instant>>,
    last_dirty: Mutex<Option<Instant>>
}
impl NodeGraph {
    const PROCESSING_INDICATOR_HOLD: Duration = Duration::from_millis(400);
    const REPROCESS_COOLDOWN: Duration = Duration::from_millis(10);

    pub fn new(chunk_grid: ChunkGrid) -> Self {
        Self {
            pool: TilePool::new(chunk_grid.tile_size()),
            chunk_grid,
            topology: Topology::default(),
            cache: EvalCache::default(),
            generation: AtomicU32::new(0),
            last_activity: Mutex::new(None),
            last_dirty: Mutex::new(None)
        }
    }

    /// Returns whether the graph has been marked dirty since the last time it was processed, and is ready to be reprocessed.
    pub fn should_reprocess_cooldown(&self) -> bool {
        self.last_dirty
            .lock()
            .unwrap()
            .is_some_and(|t| t.elapsed() < Self::REPROCESS_COOLDOWN)
    }

    // Generation management
    pub(super) fn increment_generation(&self) -> u32 {
        self.generation.fetch_add(1, Ordering::Relaxed) + 1
    }

    // Chunk grid management
    pub fn chunk_grid(&self) -> &ChunkGrid {
        &self.chunk_grid
    }
    pub fn set_chunk_grid(&mut self, chunk_grid: ChunkGrid) {
        self.chunk_grid = chunk_grid;
        self.pool = TilePool::new(chunk_grid.tile_size());
        self.cache = EvalCache::default();
    }
    pub fn tile_size(&self) -> usize {
        self.chunk_grid.tile_size()
    }

    // Graph topology management
    pub fn add_node(&mut self, node: Box<dyn Node>) -> GraphNodeId {
        let id = self.topology.add_node(node);
        self.mark_dirty(id);
        id
    }
    pub fn remove_node(&mut self, node_id: GraphNodeId) -> Result<(), NodeError> {
        self.mark_dirty(node_id); // invalidate consumers before the node disappears
        self.topology.remove_node(node_id)?;
        self.cache.remove(node_id);
        Ok(())
    }
    pub fn connect(
        &mut self,
        from_node: GraphNodeId,
        from_socket: usize,
        to_node: GraphNodeId,
        to_socket: usize
    ) -> Result<&mut Self, NodeError> {
        self.topology
            .connect(from_node, from_socket, to_node, to_socket)?;
        self.mark_dirty(to_node);
        Ok(self)
    }
    pub fn disconnect(
        &mut self,
        from_node: GraphNodeId,
        from_socket: usize,
        to_node: GraphNodeId,
        to_socket: usize
    ) -> Result<&mut Self, NodeError> {
        self.topology
            .disconnect(from_node, from_socket, to_node, to_socket)?;
        self.mark_dirty(to_node);
        Ok(self)
    }
    pub fn node(&self, id: GraphNodeId) -> Result<&dyn Node, NodeError> {
        self.topology.node(id)
    }
    /// Every edge currently in the graph, as `(from_node, from_socket, to_node, to_socket)`.
    pub fn edges(&self) -> impl Iterator<Item = (GraphNodeId, usize, GraphNodeId, usize)> + '_ {
        self.topology.edges()
    }
    pub fn node_mut(&mut self, id: GraphNodeId) -> Result<NodeMutGuard<'_>, NodeError> {
        self.topology.node(id)?;
        Ok(NodeMutGuard { graph: self, id })
    }

    /// Propagates downstream, stopping at baked nodes.
    fn mark_dirty(&mut self, id: GraphNodeId) {
        *self.last_dirty.lock().unwrap() = Some(Instant::now());
        let mut stack = vec![id];
        let mut visited = HashSet::new();
        while let Some(n) = stack.pop() {
            if !visited.insert(n) {
                continue;
            }
            if self.cache.is_baked(n) {
                continue; // opaque to invalidation
            }
            self.cache.mark_all_dirty(n);
            if let Ok(outputs) = self.topology.outputs(n) {
                stack.extend(outputs.iter().copied());
            }
        }
    }
    fn is_dirty(&self, id: GraphNodeId) -> bool {
        self.cache.is_dirty(id)
    }
    pub fn is_or_ancestor_dirty(&self, id: GraphNodeId) -> bool {
        if self.is_dirty(id) {
            return true;
        }
        if let Ok(inputs) = self.topology.inputs(id) {
            for input in inputs {
                if input.is_none() {
                    continue;
                }
                if self.is_or_ancestor_dirty(input.unwrap().0) {
                    return true;
                }
            }
        }
        false
    }

    // Usefull for UI feedback
    pub(super) fn set_is_processing(&self) {
        *self.last_activity.lock().unwrap() = Some(Instant::now());
    }
    pub fn is_processing(&self) -> bool {
        self.last_activity
            .lock()
            .unwrap()
            .is_some_and(|t| t.elapsed() < Self::PROCESSING_INDICATOR_HOLD)
    }
    pub fn cached_bytes(&self) -> usize {
        self.cache.cached_bytes()
    }
}

/// A guard that marks a node as dirty when dropped, to ensure that any changes made to the node are reflected in the graph's state.
pub struct NodeMutGuard<'a> {
    graph: &'a mut NodeGraph,
    id: GraphNodeId
}
impl<'a> Deref for NodeMutGuard<'a> {
    type Target = dyn Node;
    fn deref(&self) -> &Self::Target {
        self.graph
            .topology
            .node(self.id)
            .expect("Validated in node_mut")
    }
}
impl<'a> DerefMut for NodeMutGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.graph
            .topology
            .node_mut(self.id)
            .expect("Validated in node_mut")
    }
}
impl Drop for NodeMutGuard<'_> {
    fn drop(&mut self) {
        self.graph.mark_dirty(self.id);
    }
}
