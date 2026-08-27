//! The recursive scheduler behind [`NodeGraph::process`]/[`NodeGraph::process_chunk`]: walks a
//! node's inputs depth-first, evaluating and caching each one per [`EvalScope`], and evicts
//! chunk-scoped inputs once their last consumer has read them.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

use crate::core::graph::state::{CacheKey, EvalScope, NodeState, cache_key_of};
use crate::core::graph::topology::GraphNodeId;
use crate::core::graph::{NodeGraph, NodeGraphProcessResult};
use crate::core::node::{Node, NodeError, NodeLocality};
use crate::core::tiling::{ChunkCoord, TileContext, TilePool};

impl NodeGraph {
    fn refcount(&self, id: GraphNodeId, scope: EvalScope) -> usize {
        let Ok(outputs) = self.topology.outputs(id) else {
            return 0;
        };
        outputs
            .iter()
            .filter(|o| {
                matches!(
                    self.cache.state(**o, scope),
                    NodeState::Dirty | NodeState::Processing
                )
            })
            .count()
    }

    /// No-op for `Global` scope: its result is reused across every chunk, so it stays cached until explicitly invalidated rather than evicted per-consumer.
    fn try_evict(&mut self, id: GraphNodeId, scope: EvalScope) {
        if matches!(scope, EvalScope::Global) || self.refcount(id, scope) > 0 {
            return;
        }
        if matches!(self.cache.state(id, scope), NodeState::Cached(_)) {
            self.cache.set(id, scope, NodeState::Dirty); // tiles freed via TileHandle's own drop
        }
    }

    /// Stops at a `Global` ancestor: its kernel padding is handled entirely within its own whole-terrain pass.
    fn collect_ancestors(&self, node_id: GraphNodeId) -> Result<HashSet<GraphNodeId>, NodeError> {
        self.topology.node(node_id)?;
        let mut seen = HashSet::from([node_id]);
        let mut stack = vec![node_id];
        while let Some(id) = stack.pop() {
            if id != node_id
                && matches!(
                    self.topology.node(id)?.locality(),
                    NodeLocality::Global { .. }
                )
            {
                continue;
            }
            for (from_node, _) in self.topology.inputs(id)?.iter().flatten() {
                if seen.insert(*from_node) {
                    stack.push(*from_node);
                }
            }
        }
        Ok(seen)
    }

    fn required_internal_tile_size(&self, node_id: GraphNodeId) -> Result<usize, NodeError> {
        let padding: usize = self
            .collect_ancestors(node_id)?
            .iter()
            .map(|&id| {
                let node = self.topology.node(id)?;
                Ok::<usize, NodeError>(match node.locality() {
                    NodeLocality::Global { .. } => 0,
                    NodeLocality::Local => node.size().div_ceil(2)
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum();
        Ok(self.chunk_grid.tile_size() + 2 * padding)
    }

    pub fn process_chunk(
        &mut self,
        node_id: GraphNodeId,
        chunk: ChunkCoord
    ) -> Result<NodeGraphProcessResult, NodeError> {
        if let NodeLocality::Global { native_resolution } = self.topology.node(node_id)?.locality()
        {
            let global_pool = TilePool::new(native_resolution);
            let global_ctx = TileContext::for_global(native_resolution);
            return self.process_scoped(node_id, EvalScope::Global, &global_pool, &global_ctx);
        }

        let internal_tile_size = self.required_internal_tile_size(node_id)?;
        if internal_tile_size != self.pool.tile_length() {
            // Cached tiles were allocated for the previous tile size; can't mix with the new pool.
            self.pool = TilePool::new(internal_tile_size);
            self.cache.clear_chunk_states();
        }
        let margin = (internal_tile_size - self.chunk_grid.tile_size()) / 2;
        let ctx = self.chunk_grid.chunk_context(chunk, margin);
        let pool = self.pool.clone();
        self.process_scoped(node_id, EvalScope::Chunk(chunk), &pool, &ctx)
    }

    /// Predates chunking; kept as the degenerate `1x1` case rather than removed.
    pub fn process(&mut self, node_id: GraphNodeId) -> Result<NodeGraphProcessResult, NodeError> {
        self.process_chunk(node_id, ChunkCoord(0, 0))
    }

    fn process_scoped(
        &mut self,
        node_id: GraphNodeId,
        scope: EvalScope,
        pool: &Arc<TilePool>,
        ctx: &TileContext
    ) -> Result<NodeGraphProcessResult, NodeError> {
        match self.cache.state(node_id, scope) {
            NodeState::Cached((_, tiles)) => {
                return Ok(NodeGraphProcessResult::Processed(
                    self.generation,
                    tiles.clone()
                ));
            }
            NodeState::Baked(_) => todo!("load baked tiles from disk"),
            // Re-entering a node still on the call stack means the dependency graph has a cycle.
            NodeState::Processing => return Err(NodeError::CyclicGraph),
            NodeState::Dirty => {}
        }

        self.cache.set(node_id, scope, NodeState::Processing);

        // Failure past this point must not leave the node stuck in `Processing`, or future calls would see a false cycle instead of retrying.
        let result = self.process_scoped_body(node_id, scope, pool, ctx);
        if result.is_err() {
            self.cache.set(node_id, scope, NodeState::Dirty);
        }
        result
    }

    fn process_scoped_body(
        &mut self,
        node_id: GraphNodeId,
        scope: EvalScope,
        pool: &Arc<TilePool>,
        ctx: &TileContext
    ) -> Result<NodeGraphProcessResult, NodeError> {
        let inputs = self.topology.inputs(node_id)?.to_vec();
        let mut input_tiles = Vec::with_capacity(inputs.len());
        let mut input_keys = Vec::with_capacity(inputs.len());
        // (node, scope) each input tile came from, for eviction below once this node's output is produced.
        let mut consumed = Vec::with_capacity(inputs.len());

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
                input_tiles.push(Arc::new(pool.allocate()));
                input_keys.push(None);
                continue;
            };

            let from_locality = self.topology.node(*from_node)?.locality();
            let (child_scope, child_pool, child_ctx) = match from_locality {
                NodeLocality::Global { native_resolution } => (
                    EvalScope::Global,
                    TilePool::new(native_resolution),
                    TileContext::for_global(native_resolution)
                ),
                NodeLocality::Local => (scope, pool.clone(), *ctx)
            };

            let tile =
                match self.process_scoped(*from_node, child_scope, &child_pool, &child_ctx)? {
                    NodeGraphProcessResult::Processed(_, tiles) => match tiles.get(*from_socket) {
                        Some(tile) => tile.clone(),
                        None => {
                            return Err(NodeError::OutputNotAvailable {
                                node: self.topology.node(*from_node)?.label().to_string()
                            });
                        }
                    },
                    NodeGraphProcessResult::Processing => {
                        return Ok(NodeGraphProcessResult::Processing);
                    }
                };
            input_tiles.push(tile);
            input_keys.push(cache_key_of(self.cache.state(*from_node, child_scope)));
            consumed.push((*from_node, child_scope));
        }

        let node = self.topology.node(node_id)?;
        let key = compute_cache_key(node, node_id, &input_keys);
        let output = node.process(pool, &input_tiles, ctx)?;
        self.last_activity = Some(Instant::now());

        self.cache
            .set(node_id, scope, NodeState::Cached((key, output.clone())));
        self.generation += 1;

        for (from_node, from_scope) in consumed {
            if matches!(from_scope, EvalScope::Chunk(_)) {
                self.try_evict(from_node, from_scope);
            }
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
