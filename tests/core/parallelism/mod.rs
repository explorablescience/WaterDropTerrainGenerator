//! Tests for `src/core/parallelism/`: the live terrain session. `TerrainSession`/
//! `TerrainSessionHolder` are defined directly in `session.rs`, not a nested file, and are tested
//! right here.

use std::sync::mpsc;
use std::time::Duration;

use waterdrop_terrain_generator::core::graph::{GraphNodeId, NodeGraph};
use waterdrop_terrain_generator::core::node::{
    NParamValue, NodeError, NodeMessageLog, NodeMessageSeverity
};
use waterdrop_terrain_generator::core::tiling::ChunkGrid;
use waterdrop_terrain_generator::nodes::*;
use waterdrop_terrain_generator::{TerrainSession, TerrainSessionHolder};

#[test]
fn default_terrain_session_has_no_selection_and_no_messages() {
    let terrain = TerrainSession::default();
    assert_eq!(terrain.selected_node, None);
}

#[test]
fn action_result_ok_shows_as_an_info_message_that_can_be_cleared() {
    let mut terrain = TerrainSession::default();
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let id = graph.add_node(Box::new(Flat));

    terrain.set_action_result(id, Ok("Saved!".to_string()));
    let msg = terrain
        .action_message(id)
        .expect("a message should be recorded");
    assert_eq!(msg.severity, NodeMessageSeverity::Info);
    assert_eq!(msg.text, "Saved!");

    terrain.clear_action_message(id);
    assert!(terrain.action_message(id).is_none());
}

#[test]
fn action_result_err_shows_with_the_errors_own_severity() {
    let mut terrain = TerrainSession::default();
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let id = graph.add_node(Box::new(Flat));

    terrain.set_action_result(
        id,
        Err(NodeError::ProcessingFailed("disk full".to_string()))
    );
    let msg = terrain
        .action_message(id)
        .expect("a message should be recorded");
    assert_eq!(msg.severity, NodeMessageSeverity::Error);
    assert_eq!(msg.text, "disk full");
}

#[test]
fn a_persistent_error_message_never_expires_on_its_own() {
    let mut terrain = TerrainSession::default();
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let id = graph.add_node(Box::new(Flat));

    terrain.set_action_result(id, Err(NodeError::ProcessingFailed("boom".to_string())));
    assert_eq!(terrain.action_message_remaining(id), None);
    terrain.prune_expired_messages();
    assert!(
        terrain.action_message(id).is_some(),
        "a persistent error should survive pruning"
    );
}

#[test]
fn setting_a_new_action_result_replaces_whatever_was_shown_before() {
    let mut terrain = TerrainSession::default();
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let id = graph.add_node(Box::new(Flat));

    terrain.set_action_result(
        id,
        Err(NodeError::ProcessingFailed("first failure".to_string()))
    );
    terrain.set_action_result(id, Ok("now it works".to_string()));

    let msg = terrain.action_message(id).unwrap();
    assert_eq!(msg.severity, NodeMessageSeverity::Info);
    assert_eq!(msg.text, "now it works");
}

#[test]
fn terrain_session_holder_allows_shared_read_and_exclusive_write_access() {
    let holder = TerrainSessionHolder::default();
    let id = holder.write().graph_mut().add_node(Box::new(Flat));
    assert!(holder.read().graph().node(id).is_ok());
}

#[test]
fn timed_action_message_expires_and_is_pruned() {
    let mut terrain = TerrainSession::default();
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let id = graph.add_node(Box::new(Flat));

    terrain.set_action_result(id, Ok("Saved!".to_string()));
    // `set_action_result`'s success path uses `NodeMessageLog::ACTION_MESSAGE_DURATION`, which is
    // several seconds long, so we can't wait it out here - instead just verify the message
    // reports a bounded remaining time rather than "forever" like a persistent error would.
    let remaining = terrain
        .action_message_remaining(id)
        .expect("a timed message should report remaining time");
    assert!(remaining <= NodeMessageLog::ACTION_MESSAGE_DURATION);
    assert!(remaining > Duration::from_secs(0));
}

// Regression coverage for a self-deadlock that used to freeze the whole app: `TerrainSessionHolder`
// wraps a plain `std::sync::RwLock`, which is not reentrant. A `match` expression keeps every
// temporary created in its scrutinee alive until the match itself ends - including across its own
// arms - so `match holder.read()... { Ok(true) => { holder.write()... } ... }` keeps the read guard
// held while the `Ok(true)` arm blocks forever on `.write()`. This bit `update_render_chunks_local`
// (src/render/generate_chunks_local.rs) via `NodeGraph::needs_parallel_prepare` /
// `prepare_for_parallel_eval`, whenever a `Global`-locality ancestor wasn't cached yet - e.g. right
// after wiring one in, or after editing an upstream node's params invalidated its cache.

/// Runs `f` on a background thread and reports whether it finished within `timeout`, without
/// blocking the test forever if `f` deadlocks (the thread is simply leaked in that case).
fn run_with_timeout(timeout: Duration, f: impl FnOnce() + Send + 'static) -> bool {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        f();
        let _ = tx.send(());
    });
    rx.recv_timeout(timeout).is_ok()
}

/// Perlin -> Hydraulic Erosion (Global, uncached) -> Combine, with only Combine's first input
/// wired - mirrors both bug reports: a fresh `Global` ancestor connected into a node with a
/// dangling second input/output.
fn build_perlin_erosion_combine_graph() -> (TerrainSessionHolder, GraphNodeId, GraphNodeId) {
    let holder = TerrainSessionHolder::default();
    let (perlin, combine) = {
        let mut session = holder.write();
        let graph = session.graph_mut();
        let perlin = graph.add_node(Box::new(Perlin::default()));
        // keep the simulation itself cheap; only the locking matters here
        let erosion = HydraulicErosion {
            droplets: 10,
            ..HydraulicErosion::default()
        };
        let erosion = graph.add_node(Box::new(erosion));
        let combine = graph.add_node(Box::new(Combine::default()));
        graph
            .connect(perlin, 0, erosion, 0)
            .expect("perlin -> erosion");
        graph
            .connect(erosion, 0, combine, 0)
            .expect("erosion -> combine");
        (perlin, combine)
    };
    (holder, perlin, combine)
}

#[test]
fn safe_read_then_write_sequence_does_not_deadlock_on_uncached_global_ancestor() {
    let (holder, _perlin, combine) = build_perlin_erosion_combine_graph();

    // `needs_parallel_prepare` is `Ok(true)`: the erosion node's `Global` scope has never been
    // evaluated yet, so this exercises the exact branch that used to deadlock.
    assert!(matches!(
        holder.read().graph().needs_parallel_prepare(combine),
        Ok(true)
    ));

    let completed = run_with_timeout(Duration::from_secs(5), move || {
        // The fix: bind the read guard's result to its own statement first, so it drops before
        // the `match` (and its `.write()` arm) even starts.
        let needs_prepare = holder.read().graph().needs_parallel_prepare(combine);
        if let Ok(true) = needs_prepare {
            holder
                .write()
                .graph_mut()
                .prepare_for_parallel_eval(combine)
                .ok();
        }
    });
    assert!(
        completed,
        "reading then writing across separate statements must not deadlock"
    );
}

#[test]
fn read_guard_held_across_match_arms_self_deadlocks_on_write() {
    let (holder, _perlin, combine) = build_perlin_erosion_combine_graph();

    // The historical bug, reproduced directly against `TerrainSessionHolder`'s lock: the read
    // guard produced by the scrutinee is kept alive for the whole `match`, so the `Ok(true)` arm's
    // `.write()` call blocks on a lock this same thread is still holding.
    let completed = run_with_timeout(Duration::from_secs(2), move || {
        if let Ok(true) = holder.read().graph().needs_parallel_prepare(combine) {
            holder
                .write()
                .graph_mut()
                .prepare_for_parallel_eval(combine)
                .ok();
        }
    });
    assert!(
        !completed,
        "this pattern is expected to self-deadlock - if it now completes, either `RwLock`'s \
         behavior or `match` temporary scoping has changed and this test (and the comment above \
         `safe_read_then_write_sequence_does_not_deadlock_on_uncached_global_ancestor`) should be \
         revisited"
    );
}

#[test]
fn dirtying_a_cached_global_ancestor_flips_needs_parallel_prepare_back_to_true() {
    let (holder, perlin, combine) = build_perlin_erosion_combine_graph();

    // Prime the cache: after a successful prepare, the erosion node's `Global` scope is cached.
    holder
        .write()
        .graph_mut()
        .prepare_for_parallel_eval(combine)
        .expect("prepare should succeed: perlin -> erosion is fully wired");
    assert!(matches!(
        holder.read().graph().needs_parallel_prepare(combine),
        Ok(false)
    ));

    // Editing the upstream Perlin node invalidates everything downstream, including erosion's
    // cached `Global` entry - this is what made scenario 2 (editing an ancestor's params while a
    // downstream node using a `Global` node is displayed/pinned) hit the same deadlocking branch.
    holder
        .write()
        .graph_mut()
        .node_mut(perlin)
        .expect("perlin should still exist")
        .set_param("frequency", NParamValue::Float(2.5))
        .expect("frequency is a valid Perlin param");
    assert!(matches!(
        holder.read().graph().needs_parallel_prepare(combine),
        Ok(true)
    ));

    let completed = run_with_timeout(Duration::from_secs(5), move || {
        let needs_prepare = holder.read().graph().needs_parallel_prepare(combine);
        if let Ok(true) = needs_prepare {
            holder
                .write()
                .graph_mut()
                .prepare_for_parallel_eval(combine)
                .ok();
        }
    });
    assert!(
        completed,
        "re-preparing after an ancestor's params were edited must not deadlock"
    );
}
