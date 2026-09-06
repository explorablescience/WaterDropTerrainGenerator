use std::collections::HashMap;
use std::sync::Arc;

use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};

use crate::core::*;

#[derive(Default)]
pub struct Processor {
    active: Option<ActiveNode>
}

struct ActiveNode {
    node_id: GraphNodeId,
    work: ActiveWork
}

enum ActiveWork {
    Local {
        pending: HashMap<ChunkCoord, Task<Result<Vec<TileHandle>, NodeError>>>,
        done: HashMap<ChunkCoord, Vec<TileHandle>>
    },
    Global {
        native_resolution: usize,
        task: Task<Result<Vec<TileHandle>, NodeError>>
    }
}

enum PollOutcome {
    Pending,
    Advanced
}

impl Processor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Walks `node_id`'s dirty ancestors in topological order, evaluating one node's chunks (all
    /// in parallel) per call until it's fully cached, before moving to the next. `Ok(None)` means
    /// still processing; call again next frame to keep advancing or pick up the finished result.
    pub fn process(
        &mut self,
        topology: &Topology,
        chunk_grid: &ChunkGrid,
        pool: &mut Arc<TilePool>,
        cache: &mut Cache,
        node_id: GraphNodeId
    ) -> Result<Option<CacheEntry>, NodeError> {
        sync_pool_size(topology, chunk_grid, pool, cache, node_id)?;

        loop {
            if self.active.is_some() {
                if !self.active_is_live(cache) {
                    self.active = None;
                    continue;
                }
                match self.poll_active(cache)? {
                    PollOutcome::Pending => return Ok(None),
                    PollOutcome::Advanced => continue
                }
            }

            match next_to_spawn(topology, chunk_grid, cache, node_id)? {
                Some(candidate) => {
                    self.spawn(topology, chunk_grid, pool, cache, candidate)?;
                    continue;
                }
                None => return Ok(Some(gather(topology, chunk_grid, cache, node_id)?))
            }
        }
    }

    /// One-off, synchronous, uncached evaluation of `node_id` at an arbitrary `resolution` -
    /// the same machinery a `Global` node uses to bake a `Local` ancestor into its own pass,
    /// exposed directly for a caller that needs a node's output at a resolution decoupled from
    /// its own locality (e.g. an export action, independent of how the node is normally previewed).
    pub fn evaluate_once(
        &self,
        topology: &Topology,
        chunk_grid: &ChunkGrid,
        cache: &Cache,
        node_id: GraphNodeId,
        resolution: usize
    ) -> Result<Vec<TileHandle>, NodeError> {
        let working_size = required_working_size(topology, node_id, resolution)?;
        let margin = (working_size - resolution) / 2;
        let pool = TilePool::new(working_size);
        let ctx = global_context_with_margin(chunk_grid, resolution, margin);
        let tiles = evaluate_in_global_frame(topology, cache, chunk_grid, &pool, &ctx, node_id)?;
        Ok(crop_tiles(&tiles, working_size, resolution))
    }

    /// A node re-dirtied mid-flight loses its `Processing` cache entry - that's the cancellation signal.
    fn active_is_live(&self, cache: &Cache) -> bool {
        let Some(active) = &self.active else {
            return false;
        };
        match &active.work {
            ActiveWork::Global {
                native_resolution, ..
            } => cache.is_processing(active.node_id, EvalScope::Global(*native_resolution)),
            ActiveWork::Local { pending, done } => {
                match pending.keys().next().or_else(|| done.keys().next()) {
                    Some(chunk) => cache.is_processing(active.node_id, EvalScope::Chunk(*chunk)),
                    None => true
                }
            }
        }
    }

    fn poll_active(&mut self, cache: &mut Cache) -> Result<PollOutcome, NodeError> {
        let mut active = self.active.take().expect("checked by caller");
        let node_id = active.node_id;
        match &mut active.work {
            ActiveWork::Global {
                native_resolution,
                task
            } => match block_on(poll_once(task)) {
                None => {
                    self.active = Some(active);
                    Ok(PollOutcome::Pending)
                }
                Some(Err(e)) => {
                    cache.mark_dirty(node_id);
                    Err(e)
                }
                Some(Ok(tiles)) => {
                    cache.set(node_id, EvalScope::Global(*native_resolution), tiles);
                    Ok(PollOutcome::Advanced)
                }
            },
            ActiveWork::Local { pending, done } => {
                let ready: Vec<ChunkCoord> = pending.keys().copied().collect();
                for chunk in ready {
                    if let Some(task) = pending.get_mut(&chunk)
                        && let Some(result) = block_on(poll_once(task))
                    {
                        pending.remove(&chunk);
                        match result {
                            Ok(tiles) => {
                                done.insert(chunk, tiles);
                            }
                            Err(e) => {
                                cache.mark_dirty(node_id);
                                return Err(e);
                            }
                        }
                    }
                }
                if !pending.is_empty() {
                    self.active = Some(active);
                    return Ok(PollOutcome::Pending);
                }
                let ActiveWork::Local { done, .. } = &mut active.work else {
                    unreachable!()
                };
                for (chunk, tiles) in done.drain() {
                    cache.set(node_id, EvalScope::Chunk(chunk), tiles);
                }
                Ok(PollOutcome::Advanced)
            }
        }
    }

    fn spawn(
        &mut self,
        topology: &Topology,
        chunk_grid: &ChunkGrid,
        pool: &mut Arc<TilePool>,
        cache: &mut Cache,
        node_id: GraphNodeId
    ) -> Result<(), NodeError> {
        match topology.node(node_id)?.locality() {
            NodeLocality::Local => self.spawn_local(topology, chunk_grid, pool, cache, node_id),
            NodeLocality::Global { native_resolution } => {
                self.spawn_global(topology, chunk_grid, cache, node_id, native_resolution)
            }
        }
    }

    fn spawn_local(
        &mut self,
        topology: &Topology,
        chunk_grid: &ChunkGrid,
        pool: &Arc<TilePool>,
        cache: &mut Cache,
        node_id: GraphNodeId
    ) -> Result<(), NodeError> {
        let node = topology.node(node_id)?;
        let input_sockets = topology.inputs(node_id)?;

        enum Resolved {
            Local {
                ancestor: GraphNodeId,
                socket: usize
            },
            Global {
                ancestor: GraphNodeId,
                cropped: Vec<TileHandle>,
                native_resolution: usize,
                socket: usize,
                src_ctx: TileContext
            },
            Neutral
        }

        let mut resolved = Vec::with_capacity(input_sockets.len());
        for (socket_idx, input) in input_sockets.iter().enumerate() {
            match input {
                Some((ancestor, ancestor_socket)) => match topology.node(*ancestor)?.locality() {
                    NodeLocality::Local => resolved.push(Resolved::Local {
                        ancestor: *ancestor,
                        socket: *ancestor_socket
                    }),
                    NodeLocality::Global { native_resolution } => {
                        let working_size =
                            required_working_size(topology, *ancestor, native_resolution)?;
                        let (_, tiles) = cache
                            .get(*ancestor, EvalScope::Global(native_resolution))
                            .ok_or(NodeError::NodeNotEvaluated(*ancestor))?;
                        resolved.push(Resolved::Global {
                            ancestor: *ancestor,
                            cropped: crop_tiles(&tiles, working_size, native_resolution),
                            native_resolution,
                            socket: *ancestor_socket,
                            src_ctx: TileContext::for_global(chunk_grid, native_resolution)
                        });
                    }
                },
                None => {
                    let socket_desc = node.inputs().get(socket_idx);
                    if socket_desc.is_none_or(|s| s.required) {
                        return Err(NodeError::InputNotConnected {
                            node_id,
                            node: node.label().to_string(),
                            socket: socket_desc
                                .map(|s| s.name.to_string())
                                .unwrap_or_else(|| socket_idx.to_string())
                        });
                    }
                    resolved.push(Resolved::Neutral);
                }
            }
        }

        let snapshot: Arc<dyn Node> = Arc::from(node.clone_boxed());
        let margin = (pool.tile_length() - chunk_grid.tile_size()) / 2;
        let mut pending = HashMap::new();
        for chunk in chunk_grid.coords() {
            let ctx = chunk_grid.chunk_context(chunk, margin);
            let mut inputs = Vec::with_capacity(resolved.len());
            for r in &resolved {
                let tile = match r {
                    Resolved::Local { ancestor, socket } => {
                        let (_, tiles) = cache
                            .get(*ancestor, EvalScope::Chunk(chunk))
                            .ok_or(NodeError::NodeNotEvaluated(*ancestor))?;
                        tiles.get(*socket).cloned().ok_or_else(|| {
                            NodeError::OutputNotAvailable {
                                node: format!("{:?}", ancestor)
                            }
                        })?
                    }
                    Resolved::Global {
                        ancestor,
                        cropped,
                        native_resolution,
                        socket,
                        src_ctx
                    } => {
                        let src =
                            cropped
                                .get(*socket)
                                .ok_or_else(|| NodeError::OutputNotAvailable {
                                    node: format!("{:?}", ancestor)
                                })?;
                        resample_tile(pool, &ctx, src, *native_resolution, src_ctx)
                    }
                    Resolved::Neutral => Arc::new(pool.allocate())
                };
                inputs.push(tile);
            }

            let task_node = Arc::clone(&snapshot);
            let task_pool = Arc::clone(pool);
            let task = AsyncComputeTaskPool::get()
                .spawn(async move { task_node.process(&task_pool, &inputs, &ctx) });
            cache.mark_processing(node_id, EvalScope::Chunk(chunk));
            pending.insert(chunk, task);
        }

        self.active = Some(ActiveNode {
            node_id,
            work: ActiveWork::Local {
                pending,
                done: HashMap::new()
            }
        });
        Ok(())
    }

    fn spawn_global(
        &mut self,
        topology: &Topology,
        chunk_grid: &ChunkGrid,
        cache: &mut Cache,
        node_id: GraphNodeId,
        native_resolution: usize
    ) -> Result<(), NodeError> {
        let working_size = required_working_size(topology, node_id, native_resolution)?;
        let margin = (working_size - native_resolution) / 2;
        let ephemeral_pool = TilePool::new(working_size);
        let ctx = global_context_with_margin(chunk_grid, native_resolution, margin);

        let inputs = resolve_inputs_in_global_frame(
            topology,
            cache,
            chunk_grid,
            &ephemeral_pool,
            &ctx,
            node_id
        )?;
        let snapshot: Arc<dyn Node> = Arc::from(topology.node(node_id)?.clone_boxed());
        let task_pool = Arc::clone(&ephemeral_pool);
        let task = AsyncComputeTaskPool::get()
            .spawn(async move { snapshot.process(&task_pool, &inputs, &ctx) });

        cache.mark_processing(node_id, EvalScope::Global(native_resolution));
        self.active = Some(ActiveNode {
            node_id,
            work: ActiveWork::Global {
                native_resolution,
                task
            }
        });
        Ok(())
    }
}

fn sync_pool_size(
    topology: &Topology,
    chunk_grid: &ChunkGrid,
    pool: &mut Arc<TilePool>,
    cache: &mut Cache,
    node_id: GraphNodeId
) -> Result<(), NodeError> {
    if matches!(
        topology.node(node_id)?.locality(),
        NodeLocality::Global { .. }
    ) {
        return Ok(());
    }
    let required = required_working_size(topology, node_id, chunk_grid.tile_size())?;
    if required != pool.tile_length() {
        *pool = TilePool::new(required);
        cache.clear_all_chunks();
    }
    Ok(())
}

/// Every `Local` ancestor between `node_id` and the nearest `Global` boundary contributes its own
/// kernel margin, recursively - they all end up sharing one working-size tile.
fn required_working_size(
    topology: &Topology,
    node_id: GraphNodeId,
    base_size: usize
) -> Result<usize, NodeError> {
    let padding: usize = topology
        .collect_ancestors(node_id)?
        .into_iter()
        .map(|id| {
            let node = topology.node(id)?;
            Ok::<usize, NodeError>(match node.locality() {
                NodeLocality::Global { .. } if id != node_id => 0,
                _ => node.size().div_ceil(2)
            })
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum();
    Ok(base_size + 2 * padding)
}

fn next_to_spawn(
    topology: &Topology,
    chunk_grid: &ChunkGrid,
    cache: &Cache,
    node_id: GraphNodeId
) -> Result<Option<GraphNodeId>, NodeError> {
    for candidate in topological_order(topology, node_id)? {
        let ready = match topology.node(candidate)?.locality() {
            NodeLocality::Global { native_resolution } => cache
                .get(candidate, EvalScope::Global(native_resolution))
                .is_some(),
            NodeLocality::Local => chunk_grid
                .coords()
                .all(|c| cache.get(candidate, EvalScope::Chunk(c)).is_some())
        };
        if !ready {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

fn topological_order(
    topology: &Topology,
    node_id: GraphNodeId
) -> Result<Vec<GraphNodeId>, NodeError> {
    let ancestors = topology.collect_ancestors(node_id)?;
    let mut in_degree: HashMap<GraphNodeId, usize> = HashMap::new();
    for &id in &ancestors {
        let count = topology
            .inputs(id)?
            .iter()
            .flatten()
            .filter(|(from, _)| ancestors.contains(from))
            .count();
        in_degree.insert(id, count);
    }

    let mut ready: Vec<GraphNodeId> = in_degree
        .iter()
        .filter_map(|(&id, &d)| (d == 0).then_some(id))
        .collect();
    let mut order = Vec::with_capacity(ancestors.len());
    while let Some(id) = ready.pop() {
        order.push(id);
        for out in topology
            .outputs(id)?
            .iter()
            .filter(|o| ancestors.contains(*o))
        {
            let degree = in_degree
                .get_mut(out)
                .expect("every ancestor has an in_degree entry");
            *degree -= 1;
            if *degree == 0 {
                ready.push(*out);
            }
        }
    }

    if order.len() != ancestors.len() {
        return Err(NodeError::CyclicGraph);
    }
    Ok(order)
}

fn gather(
    topology: &Topology,
    chunk_grid: &ChunkGrid,
    cache: &Cache,
    node_id: GraphNodeId
) -> Result<CacheEntry, NodeError> {
    match topology.node(node_id)?.locality() {
        NodeLocality::Global { native_resolution } => {
            let working_size = required_working_size(topology, node_id, native_resolution)?;
            let (uuid, tiles) = cache
                .get(node_id, EvalScope::Global(native_resolution))
                .ok_or(NodeError::NodeNotEvaluated(node_id))?;
            Ok(CacheEntry::Global(
                uuid,
                crop_tiles(&tiles, working_size, native_resolution)
            ))
        }
        NodeLocality::Local => {
            let target_size = chunk_grid.tile_size();
            let working_size = required_working_size(topology, node_id, target_size)?;
            let crop_pool = (working_size != target_size).then(|| TilePool::new(target_size));

            let mut per_chunk = HashMap::new();
            for chunk in chunk_grid.coords() {
                let (uuid, tiles) = cache
                    .get(node_id, EvalScope::Chunk(chunk))
                    .ok_or(NodeError::NodeNotEvaluated(node_id))?;
                let cropped = match &crop_pool {
                    Some(p) => tiles
                        .iter()
                        .map(|t| crop_tile_into(p, t, working_size, target_size))
                        .collect(),
                    None => tiles
                };
                per_chunk.insert(chunk, (uuid, cropped));
            }
            Ok(CacheEntry::Local(per_chunk))
        }
    }
}

/// A `Local` ancestor baked into a `Global` node's pass is evaluated fresh here rather than reused
/// from its own per-chunk cache - it was never computed at this scope anywhere else.
fn evaluate_in_global_frame(
    topology: &Topology,
    cache: &Cache,
    chunk_grid: &ChunkGrid,
    pool: &Arc<TilePool>,
    ctx: &TileContext,
    node_id: GraphNodeId
) -> Result<Vec<TileHandle>, NodeError> {
    let node = topology.node(node_id)?;
    if let NodeLocality::Global { native_resolution } = node.locality() {
        let working_size = required_working_size(topology, node_id, native_resolution)?;
        let (_, tiles) = cache
            .get(node_id, EvalScope::Global(native_resolution))
            .ok_or(NodeError::NodeNotEvaluated(node_id))?;
        let cropped = crop_tiles(&tiles, working_size, native_resolution);
        let src_ctx = TileContext::for_global(chunk_grid, native_resolution);
        return Ok(cropped
            .iter()
            .map(|t| resample_tile(pool, ctx, t, native_resolution, &src_ctx))
            .collect());
    }
    let inputs = resolve_inputs_in_global_frame(topology, cache, chunk_grid, pool, ctx, node_id)?;
    node.process(pool, &inputs, ctx)
}

fn resolve_inputs_in_global_frame(
    topology: &Topology,
    cache: &Cache,
    chunk_grid: &ChunkGrid,
    pool: &Arc<TilePool>,
    ctx: &TileContext,
    node_id: GraphNodeId
) -> Result<Vec<TileHandle>, NodeError> {
    let node = topology.node(node_id)?;
    let input_sockets = topology.inputs(node_id)?;
    let mut inputs = Vec::with_capacity(input_sockets.len());
    for (socket_idx, input) in input_sockets.iter().enumerate() {
        match input {
            Some((ancestor, ancestor_socket)) => {
                let tiles =
                    evaluate_in_global_frame(topology, cache, chunk_grid, pool, ctx, *ancestor)?;
                inputs.push(tiles.get(*ancestor_socket).cloned().ok_or_else(|| {
                    NodeError::OutputNotAvailable {
                        node: format!("{:?}", ancestor)
                    }
                })?);
            }
            None => {
                let socket_desc = node.inputs().get(socket_idx);
                if socket_desc.is_none_or(|s| s.required) {
                    return Err(NodeError::InputNotConnected {
                        node_id,
                        node: node.label().to_string(),
                        socket: socket_desc
                            .map(|s| s.name.to_string())
                            .unwrap_or_else(|| socket_idx.to_string())
                    });
                }
                inputs.push(Arc::new(pool.allocate()));
            }
        }
    }
    Ok(inputs)
}

fn global_context_with_margin(
    chunk_grid: &ChunkGrid,
    native_resolution: usize,
    margin: usize
) -> TileContext {
    let base = TileContext::for_global(chunk_grid, native_resolution);
    if margin == 0 {
        return base;
    }
    let (sx, sy) = base.world_step;
    TileContext {
        chunk: None,
        world_origin: (
            base.world_origin.0 - margin as f32 * sx,
            base.world_origin.1 - margin as f32 * sy
        ),
        world_step: base.world_step,
        world_extent: (
            base.world_extent.0 + 2.0 * margin as f32 * sx,
            base.world_extent.1 + 2.0 * margin as f32 * sy
        )
    }
}

fn resample_tile(
    dst_pool: &Arc<TilePool>,
    dst_ctx: &TileContext,
    src: &TileHandle,
    src_size: usize,
    src_ctx: &TileContext
) -> TileHandle {
    let mut output = dst_pool.allocate();
    let s = output.size();
    for y in 0..s {
        for x in 0..s {
            let world = dst_ctx.world_pos(x, y);
            let (sx, sy) = src_ctx.to_texel(world);
            output[y * s + x] = bilinear_sample(src, src_size, sx, sy);
        }
    }
    Arc::new(output)
}

fn crop_tile_into(
    pool: &Arc<TilePool>,
    tile: &TileHandle,
    full_size: usize,
    target_size: usize
) -> TileHandle {
    if full_size == target_size {
        return Arc::clone(tile);
    }
    let cropped = crop_padding(tile, full_size, target_size);
    let mut out = pool.allocate();
    out.copy_from_slice(&cropped);
    Arc::new(out)
}

fn crop_tiles(tiles: &[TileHandle], full_size: usize, target_size: usize) -> Vec<TileHandle> {
    if full_size == target_size {
        return tiles.to_vec();
    }
    let pool = TilePool::new(target_size);
    tiles
        .iter()
        .map(|t| crop_tile_into(&pool, t, full_size, target_size))
        .collect()
}
