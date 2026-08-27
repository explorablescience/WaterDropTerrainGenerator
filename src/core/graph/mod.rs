//! The graph engine: a pure, evaluation-scheduling layer on top of [`node::Node`](crate::core::node::Node).
//! [`topology`] holds connectivity, [`state`] holds the per-scope cache, and [`eval`] is the
//! recursive scheduler tying them together; this module itself is [`NodeGraph`]'s public CRUD
//! shell (construction, node/edge mutation, dirty propagation).

use std::collections::HashSet;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
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
    /// Backs `Local` nodes only; `Global` nodes allocate from their own pool (see `eval::process_scoped`), so resizing this one doesn't disturb their cached results.
    pool: Arc<TilePool>,
    chunk_grid: ChunkGrid,
    topology: Topology,
    cache: EvalCache,
    generation: u32,
    /// Last actual recompute (not cache hit); drives the UI's "Processing" indicator. `None` until first computation.
    last_activity: Option<Instant>
}

impl NodeGraph {
    /// Evaluation is synchronous and finishes within the frame it starts, so this is purely a UI hold time.
    const PROCESSING_INDICATOR_HOLD: Duration = Duration::from_millis(400);

    pub fn new(chunk_grid: ChunkGrid) -> Self {
        Self {
            pool: TilePool::new(chunk_grid.tile_size()),
            chunk_grid,
            topology: Topology::default(),
            cache: EvalCache::default(),
            generation: 0,
            last_activity: None
        }
    }

    pub fn pool(&self) -> &Arc<TilePool> {
        &self.pool
    }

    /// Across every chunk and every node.
    pub fn cached_bytes(&self) -> usize {
        self.cache.cached_bytes()
    }

    pub fn chunk_grid(&self) -> &ChunkGrid {
        &self.chunk_grid
    }

    /// Keeps every node and connection intact; only cached tiles and pool are invalidated, since they were sized for the old grid.
    pub fn set_chunk_grid(&mut self, chunk_grid: ChunkGrid) {
        self.chunk_grid = chunk_grid;
        self.pool = TilePool::new(chunk_grid.tile_size());
        self.cache = EvalCache::default();
    }

    /// Requested output tile size (core, non-margin texels per chunk edge).
    pub fn tile_size(&self) -> usize {
        self.chunk_grid.tile_size()
    }

    pub fn is_processing(&self) -> bool {
        self.last_activity
            .is_some_and(|t| t.elapsed() < Self::PROCESSING_INDICATOR_HOLD)
    }

    pub fn add_node(&mut self, node: Box<dyn Node>) -> GraphNodeId {
        self.topology.add_node(node)
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

    /// In ascending order.
    pub fn node_ids(&self) -> impl Iterator<Item = GraphNodeId> + '_ {
        self.topology.node_ids()
    }

    /// Indexed by input socket: `Some((from_node, from_socket))` for a wired socket, `None` for an unconnected one.
    pub fn inputs(
        &self,
        node_id: GraphNodeId
    ) -> Result<&[Option<(GraphNodeId, usize)>], NodeError> {
        self.topology.inputs(node_id)
    }

    /// Guard marks the node (and its downstream) dirty when mutation is done.
    pub fn node_mut(&mut self, id: GraphNodeId) -> Result<NodeMutGuard<'_>, NodeError> {
        self.topology.node(id)?;
        Ok(NodeMutGuard { graph: self, id })
    }

    /// Propagates downstream, stopping at baked nodes.
    fn mark_dirty(&mut self, id: GraphNodeId) {
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
}

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
