use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::core::node_error::NodeError;
use crate::core::{
    graph::{
        state::{CacheKey, EvalCache, NodeState, cache_key_of},
        topology::{GraphNodeId, Topology}
    },
    node::Node,
    tile_allocator::{TileHandle, TilePool}
};

pub enum NodeGraphProcessResult {
    Processed(u32, Vec<TileHandle>),
    Processing
}

pub struct NodeGraph {
    pool: Arc<TilePool>,
    /// Requested output tile size; the pool's actual tile size may be larger to
    /// accommodate the padding required by the nodes feeding the last processed node.
    tile_size: usize,
    topology: Topology,
    cache: EvalCache,
    generation: u32,
    /// When a node was last actually recomputed (as opposed to served from cache), used to
    /// drive a brief "Processing" indicator in the UI. `None` until the first computation.
    last_activity: Option<Instant>
}

impl NodeGraph {
    /// How long the "Processing" indicator stays lit after the most recent recomputation.
    /// Evaluation itself is synchronous and finishes within the frame it starts, so this is
    /// purely a UI hold time - long enough to read, short enough to feel live.
    const PROCESSING_INDICATOR_HOLD: Duration = Duration::from_millis(400);

    pub fn new(tile_size: usize) -> Self {
        Self {
            pool: TilePool::new(tile_size),
            tile_size,
            topology: Topology::default(),
            cache: EvalCache::default(),
            generation: 0,
            last_activity: None
        }
    }

    /// The tile pool currently backing this graph's node outputs.
    pub fn pool(&self) -> &Arc<TilePool> {
        &self.pool
    }

    /// Whether a node was recomputed recently enough that the UI should show "Processing"
    /// rather than "Idle".
    pub fn is_processing(&self) -> bool {
        self.last_activity
            .is_some_and(|t| t.elapsed() < Self::PROCESSING_INDICATOR_HOLD)
    }

    pub fn add_node(&mut self, node: Box<dyn Node>) -> GraphNodeId {
        let id = self.topology.add_node(node);
        self.cache.set(id, NodeState::Dirty);
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

    /// Every node id still present in the graph, in ascending order.
    pub fn node_ids(&self) -> impl Iterator<Item = GraphNodeId> + '_ {
        self.topology.node_ids()
    }

    /// `node_id`'s inputs, indexed by input socket: `Some((from_node, from_socket))` for a wired
    /// socket, `None` for an unconnected one.
    pub fn inputs(&self, node_id: GraphNodeId) -> Result<&[Option<(GraphNodeId, usize)>], NodeError> {
        self.topology.inputs(node_id)
    }

    /// Requested output tile size
    pub fn tile_size(&self) -> usize {
        self.tile_size
    }

    /// Guard marks the node (and its downstream) dirty when mutation is done.
    pub fn node_mut(&mut self, id: GraphNodeId) -> Result<NodeMutGuard<'_>, NodeError> {
        self.topology.node(id)?; // validate existence up front
        Ok(NodeMutGuard { graph: self, id })
    }

    /// Marks a node dirty and propagates downstream, stopping at baked nodes.
    fn mark_dirty(&mut self, id: GraphNodeId) {
        let mut stack = vec![id];
        let mut visited = HashSet::new();
        while let Some(n) = stack.pop() {
            if !visited.insert(n) {
                continue;
            }
            if matches!(self.cache.state(n), NodeState::Baked(..)) {
                continue; // opaque to invalidation
            }
            self.cache.set(n, NodeState::Dirty);
            if let Ok(outputs) = self.topology.outputs(n) {
                stack.extend(outputs.iter().copied());
            }
        }
    }

    /// Downstream nodes still depending on `id`'s cached output.
    fn refcount(&self, id: GraphNodeId) -> usize {
        let Ok(outputs) = self.topology.outputs(id) else {
            return 0;
        };
        outputs
            .iter()
            .filter(|o| {
                matches!(
                    self.cache.state(**o),
                    NodeState::Dirty | NodeState::Processing
                )
            })
            .count()
    }

    /// Drops a node's cached tiles once nothing dirty still needs them.
    fn try_evict(&mut self, id: GraphNodeId) {
        if self.refcount(id) > 0 {
            return;
        }
        if matches!(self.cache.state(id), NodeState::Cached(_)) {
            self.cache.set(id, NodeState::Dirty); // tiles freed via TileHandle's own drop
        }
    }

    /// Nodes feeding into `node_id`, transitively, including `node_id` itself.
    fn collect_ancestors(&self, node_id: GraphNodeId) -> Result<HashSet<GraphNodeId>, NodeError> {
        self.topology.node(node_id)?; // validate existence up front
        let mut seen = HashSet::from([node_id]);
        let mut stack = vec![node_id];
        while let Some(id) = stack.pop() {
            for (from_node, _) in self.topology.inputs(id)?.iter().flatten() {
                if seen.insert(*from_node) {
                    stack.push(*from_node);
                }
            }
        }
        Ok(seen)
    }

    /// Tile size the pool needs so that every node feeding `node_id` has enough padding
    /// around the requested `tile_size` output to sample its kernel without going out of bounds.
    fn required_internal_tile_size(&self, node_id: GraphNodeId) -> Result<usize, NodeError> {
        let padding: usize = self
            .collect_ancestors(node_id)?
            .iter()
            .map(|&id| self.topology.node(id).map(|n| n.size().div_ceil(2)))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum();
        Ok(self.tile_size + 2 * padding)
    }

    pub fn process(&mut self, node_id: GraphNodeId) -> Result<NodeGraphProcessResult, NodeError> {
        let internal_tile_size = self.required_internal_tile_size(node_id)?;
        if internal_tile_size != self.pool.tile_length() {
            // Cached tiles were allocated for the previous tile size; they can't be mixed
            // with tiles from the new pool, so start over.
            self.pool = TilePool::new(internal_tile_size);
            self.cache = EvalCache::default();
        }
        self.process_node(node_id)
    }

    fn process_node(&mut self, node_id: GraphNodeId) -> Result<NodeGraphProcessResult, NodeError> {
        match self.cache.state(node_id) {
            NodeState::Cached((_, tiles)) => {
                return Ok(NodeGraphProcessResult::Processed(
                    self.generation,
                    tiles.clone()
                ));
            }
            NodeState::Baked(_) => todo!("load baked tiles from disk"),
            // Re-entering a node that is still on the current call stack means the
            // dependency graph has a cycle back to it.
            NodeState::Processing => return Err(NodeError::CyclicGraph),
            NodeState::Dirty => {}
        }

        self.cache.set(node_id, NodeState::Processing);

        // Any failure past this point must not leave the node stuck in `Processing`
        // forever - that would make every future call see it as an unresolved cycle
        // instead of retrying, even after the user fixes the underlying issue (e.g. by
        // connecting a missing input).
        let result = self.process_node_body(node_id);
        if result.is_err() {
            self.cache.set(node_id, NodeState::Dirty);
        }
        result
    }

    fn process_node_body(
        &mut self,
        node_id: GraphNodeId
    ) -> Result<NodeGraphProcessResult, NodeError> {
        let inputs = self.topology.inputs(node_id)?.to_vec();
        let mut input_tiles = Vec::with_capacity(inputs.len());
        let mut input_keys = Vec::with_capacity(inputs.len());

        for (socket, input) in inputs.iter().enumerate() {
            let Some((from_node, from_socket)) = input else {
                let node = self.topology.node(node_id)?;
                let socket_desc = node.inputs().get(socket);
                if socket_desc.is_none_or(|s| s.required) {
                    return Err(NodeError::InputNotConnected {
                        node_id,
                        node: node.label().to_string(),
                        socket: socket_desc
                            .map(|s| s.name.to_string())
                            .unwrap_or_else(|| socket.to_string())
                    });
                }
                // Optional and unconnected: feed the node a neutral, zero-filled tile.
                input_tiles.push(Arc::new(self.pool.allocate()));
                input_keys.push(None);
                continue;
            };
            match self.process_node(*from_node)? {
                NodeGraphProcessResult::Processed(_, tiles) => {
                    let tile = match tiles.get(*from_socket) {
                        Some(tile) => tile.clone(),
                        None => {
                            return Err(NodeError::OutputNotAvailable {
                                node: self.topology.node(*from_node)?.label().to_string()
                            });
                        }
                    };
                    input_tiles.push(tile);
                }
                NodeGraphProcessResult::Processing => return Ok(NodeGraphProcessResult::Processing)
            }
            input_keys.push(cache_key_of(self.cache.state(*from_node)));
        }

        let node = self.topology.node(node_id)?;
        let key = compute_cache_key(node, node_id, &input_keys);
        let output = node.process(&self.pool, &input_tiles)?;
        self.last_activity = Some(Instant::now());

        self.cache
            .set(node_id, NodeState::Cached((key, output.clone())));
        self.generation += 1;

        for input in inputs.into_iter().flatten() {
            self.try_evict(input.0);
        }

        Ok(NodeGraphProcessResult::Processed(self.generation, output))
    }
}

fn compute_cache_key(
    node: &dyn Node,
    node_id: GraphNodeId,
    input_keys: &[Option<CacheKey>]
) -> CacheKey {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    node_id.0.hash(&mut hasher);
    node.params_hash().hash(&mut hasher);
    input_keys.hash(&mut hasher);
    CacheKey(hasher.finish())
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
