//! Fans chunk evaluation out onto Bevy's compute task pool instead of blocking a frame for the
//! sum of every chunk's cost. Pairs with `NodeGraph::process_chunk_shared`.

use std::collections::HashMap;

use bevy::prelude::Resource;
use bevy::tasks::{ComputeTaskPool, Task, block_on, poll_once};

use crate::core::{
    graph::{GraphNodeId, NodeGraphProcessResult},
    node::NodeError,
    session::TerrainSessionHolder,
    tiling::ChunkCoord
};

/// One in-flight (or just-finished) background chunk evaluation per [`ChunkCoord`], for whichever
/// node is currently being previewed. Call [`Self::clear`] when that node changes.
#[derive(Resource, Default)]
pub struct ChunkJobs {
    pending: HashMap<ChunkCoord, Task<Result<NodeGraphProcessResult, NodeError>>>
}
impl ChunkJobs {
    /// Spawns a job for `chunk` if none is in flight; otherwise returns its result once finished.
    /// Caller must ensure `NodeGraph::prepare_for_parallel_eval(node_id)` already ran (see
    /// `NodeGraph::needs_parallel_prepare`) before calling this for any chunk of `node_id`.
    pub fn poll_or_spawn(
        &mut self,
        session: &TerrainSessionHolder,
        node_id: GraphNodeId,
        chunk: ChunkCoord
    ) -> Option<Result<NodeGraphProcessResult, NodeError>> {
        if let Some(task) = self.pending.get_mut(&chunk) {
            let result = block_on(poll_once(task));
            if result.is_some() {
                self.pending.remove(&chunk);
            }
            return result;
        }

        let session = session.clone();
        let task = ComputeTaskPool::get()
            .spawn(async move { session.read().graph().process_chunk_shared(node_id, chunk) });
        self.pending.insert(chunk, task);
        None
    }

    /// Drops pending jobs for chunks no longer in `live_chunks` (e.g. after a grid resize).
    pub fn retain_live(&mut self, live_chunks: &std::collections::HashSet<ChunkCoord>) {
        self.pending.retain(|chunk, _| live_chunks.contains(chunk));
    }

    /// Call when the previewed node changes, so a stale result isn't mistaken for the new one's.
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}
