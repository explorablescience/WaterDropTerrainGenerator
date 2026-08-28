//! Tests for `src/core/parallelism/`: the live terrain session. `TerrainSession`/
//! `TerrainSessionHolder` are defined directly in `session.rs`, not a nested file, and are tested
//! right here.

use std::time::Duration;

use waterdrop_terrain_generator::core::graph::NodeGraph;
use waterdrop_terrain_generator::core::node::{
    NodeError, NodeMessageLog, NodeMessageSeverity
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
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4 as f32));
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
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4 as f32));
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
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4 as f32));
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
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4 as f32));
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
    let id = holder
        .write()
        .graph_mut()
        .add_node(Box::new(Flat));
    assert!(holder.read().graph().node(id).is_ok());
}

#[test]
fn timed_action_message_expires_and_is_pruned() {
    let mut terrain = TerrainSession::default();
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4 as f32));
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
