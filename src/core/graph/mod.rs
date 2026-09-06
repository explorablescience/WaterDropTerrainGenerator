//! The graph engine connected to the node system, which handles evaluation, caching, and chunk processing.

use std::collections::HashSet;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use crate::core::evaluation::TilePool;
use crate::core::node::{Node, NodeError};
use crate::core::tiling::ChunkGrid;
use crate::core::{Cache, CacheEntry, Processor};

mod eval;
mod topology;

pub use topology::{GraphNodeId, Topology};

pub struct NodeGraph {
    pool: Arc<TilePool>,
    chunk_grid: ChunkGrid,
    topology: Topology,
    cache: Cache,
    processor: Processor,
    is_processing: bool
}
impl NodeGraph {
    pub fn new(chunk_grid: ChunkGrid) -> Self {
        Self {
            pool: TilePool::new(chunk_grid.tile_size()),
            chunk_grid,
            topology: Topology::default(),
            cache: Cache::new(),
            processor: Processor::new(),
            is_processing: false
        }
    }

    // Chunk grid management
    pub fn chunk_grid(&self) -> &ChunkGrid {
        &self.chunk_grid
    }
    pub fn set_chunk_grid(&mut self, chunk_grid: ChunkGrid) {
        self.chunk_grid = chunk_grid;
        self.pool = TilePool::new(chunk_grid.tile_size());
        self.cache = Cache::new();
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
        self.mark_dirty(node_id);
        self.topology.remove_node(node_id)?;
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
        self.mark_dirty(from_node);
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
        self.mark_dirty(from_node);
        Ok(self)
    }
    pub fn node(&self, id: GraphNodeId) -> Result<&dyn Node, NodeError> {
        self.topology.node(id)
    }
    pub fn node_mut(&mut self, id: GraphNodeId) -> Result<NodeMutGuard<'_>, NodeError> {
        self.topology.node(id)?;
        Ok(NodeMutGuard { graph: self, id })
    }
    /// Every edge currently in the graph, as `(from_node, from_socket, to_node, to_socket)`.
    pub fn edges(&self) -> impl Iterator<Item = (GraphNodeId, usize, GraphNodeId, usize)> + '_ {
        self.topology.edges()
    }

    /// Marks a node and all its descendants as dirty, indicating that they need to be re-evaluated.
    fn mark_dirty(&mut self, id: GraphNodeId) {
        let mut stack = vec![id];
        let mut visited = HashSet::new();
        while let Some(n) = stack.pop() {
            if !visited.insert(n) {
                continue;
            }
            self.cache.mark_dirty(n);
            if let Ok(outputs) = self.topology.outputs(n) {
                stack.extend(outputs.iter().copied());
            }
        }
    }
    /// Drives evaluation forward by one step; `Ok(None)` while still processing.
    pub fn get(&mut self, node_id: GraphNodeId) -> Result<Option<CacheEntry>, NodeError> {
        let result = self.processor.process(
            &self.topology,
            &self.chunk_grid,
            &mut self.pool,
            &mut self.cache,
            node_id
        );
        self.set_is_processing(self.processor.is_active());
        result
    }

    // Usefull for UI feedback
    pub(super) fn set_is_processing(&mut self, is_processing: bool) {
        self.is_processing = is_processing;
    }
    pub fn is_processing(&self) -> bool {
        self.is_processing
    }
    pub fn allocated_bytes(&self) -> usize {
        self.cache.allocated_bytes()
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
