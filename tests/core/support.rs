//! Shared test helpers for driving the async `NodeGraph::get` API from a plain `#[test]` fn.

use bevy::tasks::{AsyncComputeTaskPool, TaskPool};

/// `NodeGraph::get` spawns chunk jobs on Bevy's async compute task pool, which panics if used
/// before it's initialized. Idempotent, so every test that calls `get` can call this unconditionally.
pub fn init_task_pool() {
    AsyncComputeTaskPool::get_or_init(TaskPool::new);
}

/// Repeatedly calls `f` (e.g. `|| graph.get(id)`) until it returns `Ok(Some(_))` or `Err(_)`.
pub fn poll_until_ready<T, E>(mut f: impl FnMut() -> Result<Option<T>, E>) -> Result<T, E> {
    for _ in 0..1_000_000 {
        if let Some(value) = f()? {
            return Ok(value);
        }
        std::thread::yield_now();
    }
    panic!("timed out waiting for graph evaluation to finish");
}
