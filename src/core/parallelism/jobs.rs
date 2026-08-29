//! Background jobs for evaluating chunks of the currently-previewed node in a [`TerrainSession`]. Each chunk is evaluated in a separate background task, and the result is cached in the session's graph. When the previewed node changes, all pending jobs are dropped to avoid applying stale results.

use std::collections::HashMap;

use bevy::prelude::Resource;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, poll_once};

use crate::core::{
    graph::{GraphNodeId, NodeGraphProcessResult},
    node::NodeError,
    parallelism::TerrainSessionHolder,
    tiling::ChunkCoord
};

/// A collection of background tasks for evaluating chunks of the currently-previewed node in a
/// [`TerrainSession`]. Finished chunks are buffered in `staged` so a caller can swap in a whole
/// dirtied batch at once, rather than showing a fast chunk next to slower neighbors still on the
/// previous generation.
#[derive(Resource, Default)]
pub struct ChunkJobs {
    pending: HashMap<ChunkCoord, Task<Result<NodeGraphProcessResult, NodeError>>>,
    staged: HashMap<ChunkCoord, Result<NodeGraphProcessResult, NodeError>>
}
impl ChunkJobs {
    /// Spawns a background task to evaluate `node_id` at `chunk`, or polls the existing one.
    /// Always polls a pending task before checking the cache - a task's result lands in the cache
    /// as a side effect of running, so checking the cache first can see it already `Cached` and
    /// skip collecting the task, leaking its result forever.
    pub fn poll_or_spawn(
        &mut self,
        session: &TerrainSessionHolder,
        node_id: GraphNodeId,
        chunk: ChunkCoord
    ) -> Option<Result<NodeGraphProcessResult, NodeError>> {
        // If a task is already in flight for this chunk, poll it and return the result if it's done
        if let Some(task) = self.pending.get_mut(&chunk) {
            if let Some(result) = block_on(poll_once(task)) {
                self.pending.remove(&chunk);
                return Some(result);
            } else {
                return None; // still in flight
            }
        }

        // Already cached: a pure cache hit, cheap enough to do synchronously.
        if session.read().graph().is_chunk_cached(node_id, chunk) {
            return Some(session.read().graph().process_chunk_shared(node_id, chunk));
        }

        // Otherwise, spawn a new task to evaluate the chunk
        let session = session.clone();
        let task = AsyncComputeTaskPool::get()
            .spawn(async move { session.read().graph().process_chunk_shared(node_id, chunk) });
        self.pending.insert(chunk, task);
        None
    }

    /// Buffers a finished chunk's result until [`Self::take_staged`] releases the whole batch.
    pub fn stage_result(&mut self, chunk: ChunkCoord, result: Result<NodeGraphProcessResult, NodeError>) {
        self.staged.insert(chunk, result);
    }
    /// Drains every buffered result, for a caller that has confirmed the whole batch is ready.
    pub fn take_staged(&mut self) -> HashMap<ChunkCoord, Result<NodeGraphProcessResult, NodeError>> {
        std::mem::take(&mut self.staged)
    }

    /// Drops pending/staged jobs for chunks no longer in `live_chunks` (e.g. after a grid resize).
    pub fn retain_live(&mut self, live_chunks: &std::collections::HashSet<ChunkCoord>) {
        self.pending.retain(|chunk, _| live_chunks.contains(chunk));
        self.staged.retain(|chunk, _| live_chunks.contains(chunk));
    }

    /// Call when the previewed node changes, so a stale result isn't mistaken for the new one's.
    pub fn clear(&mut self) {
        self.pending.clear();
        self.staged.clear();
    }
}

/// Computes one `Global`-locality ancestor's whole-terrain pass as a background task, so a `Local`
/// node fanning out to [`ChunkJobs`] can barrier on it without blocking the caller. Ancestors are
/// processed one at a time (see `NodeGraph::pending_global_ancestors`), so at most one is pending.
#[derive(Resource, Default)]
pub struct GlobalPassJobs {
    pending: Option<(GraphNodeId, Task<Result<NodeGraphProcessResult, NodeError>>)>
}
impl GlobalPassJobs {
    /// Spawns a background task computing `ancestor`'s whole-terrain pass, or polls the existing
    /// one if it was already spawned for this same ancestor.
    pub fn poll_or_spawn(
        &mut self,
        session: &TerrainSessionHolder,
        ancestor: GraphNodeId
    ) -> Option<Result<NodeGraphProcessResult, NodeError>> {
        if let Some((pending_ancestor, task)) = &mut self.pending {
            if *pending_ancestor == ancestor {
                if let Some(result) = block_on(poll_once(task)) {
                    self.pending = None;
                    return Some(result);
                }
                return None; // still in flight
            }
            self.pending = None; // a different ancestor is now requested; drop the stale task
        }

        // `chunk` is a dummy: `process_chunk_shared` takes the `Global` branch for a
        // `Global`-locality node regardless of it, computing its whole-terrain pass instead.
        let session = session.clone();
        let task = AsyncComputeTaskPool::get().spawn(async move {
            session
                .read()
                .graph()
                .process_chunk_shared(ancestor, ChunkCoord(0, 0))
        });
        self.pending = Some((ancestor, task));
        None
    }

    /// Call when the previewed node changes, so a stale result isn't mistaken for the new one's.
    pub fn clear(&mut self) {
        self.pending = None;
    }
}
