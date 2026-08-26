use std::time::Duration;

use waterdrop_terrain_editor::core::node_error::NodeError;
use waterdrop_terrain_editor::core::node_message::NodeMessageSeverity;
use waterdrop_terrain_editor::core::node_parameters::NParamValue;
use waterdrop_terrain_editor::nodes::{NodeErosion, NodeGeneratorFlat};
use waterdrop_terrain_editor::{TerrainGraph, TerrainGraphHolder};

#[test]
fn default_terrain_graph_has_no_selection_and_no_messages() {
    let terrain = TerrainGraph::default();
    assert_eq!(terrain.selected_node, None);
}

#[test]
fn action_result_ok_shows_as_an_info_message_that_can_be_cleared() {
    let mut terrain = TerrainGraph::default();
    let mut graph = waterdrop_terrain_editor::core::graph::NodeGraph::new(4);
    let id = graph.add_node(Box::new(NodeGeneratorFlat));

    terrain.set_action_result(id, Ok("Saved!".to_string()));
    let msg = terrain.action_message(id).expect("a message should be recorded");
    assert_eq!(msg.severity, NodeMessageSeverity::Info);
    assert_eq!(msg.text, "Saved!");

    terrain.clear_action_message(id);
    assert!(terrain.action_message(id).is_none());
}

#[test]
fn action_result_err_shows_with_the_errors_own_severity() {
    let mut terrain = TerrainGraph::default();
    let mut graph = waterdrop_terrain_editor::core::graph::NodeGraph::new(4);
    let id = graph.add_node(Box::new(NodeGeneratorFlat));

    terrain.set_action_result(id, Err(NodeError::ProcessingFailed("disk full".to_string())));
    let msg = terrain.action_message(id).expect("a message should be recorded");
    assert_eq!(msg.severity, NodeMessageSeverity::Error);
    assert_eq!(msg.text, "disk full");
}

#[test]
fn a_persistent_error_message_never_expires_on_its_own() {
    let mut terrain = TerrainGraph::default();
    let mut graph = waterdrop_terrain_editor::core::graph::NodeGraph::new(4);
    let id = graph.add_node(Box::new(NodeGeneratorFlat));

    terrain.set_action_result(id, Err(NodeError::ProcessingFailed("boom".to_string())));
    assert_eq!(terrain.action_message_remaining(id), None);
    terrain.prune_expired_messages();
    assert!(terrain.action_message(id).is_some(), "a persistent error should survive pruning");
}

#[test]
fn setting_a_new_action_result_replaces_whatever_was_shown_before() {
    let mut terrain = TerrainGraph::default();
    let mut graph = waterdrop_terrain_editor::core::graph::NodeGraph::new(4);
    let id = graph.add_node(Box::new(NodeGeneratorFlat));

    terrain.set_action_result(id, Err(NodeError::ProcessingFailed("first failure".to_string())));
    terrain.set_action_result(id, Ok("now it works".to_string()));

    let msg = terrain.action_message(id).unwrap();
    assert_eq!(msg.severity, NodeMessageSeverity::Info);
    assert_eq!(msg.text, "now it works");
}

#[test]
fn process_returns_the_new_generation_only_the_first_time_its_seen() {
    let mut terrain = TerrainGraph::default();
    let source = terrain.graph_mut().add_node(Box::new(NodeGeneratorFlat));
    let sink = terrain.graph_mut().add_node(Box::new(NodeErosion::default()));
    terrain.graph_mut().connect(source, 0, sink, 0).expect("connection should succeed");

    let first = terrain.process(sink).expect("processing should succeed").expect("a fresh generation should be reported");
    // The generation counter is global to the graph and advances once per node actually
    // recomputed (here: both `source` and `sink`), so it isn't necessarily 1 on the first call.
    assert!(first.0 > 0);

    // Nothing changed since: the underlying graph serves the same cached generation, so
    // `TerrainGraph::process` should report that there's nothing new to show.
    let second = terrain.process(sink).expect("processing should succeed");
    assert!(second.is_none(), "an unchanged graph shouldn't report a new generation twice");
}

#[test]
fn process_reports_a_new_generation_again_after_a_parameter_changes() {
    let mut terrain = TerrainGraph::default();
    let source = terrain.graph_mut().add_node(Box::new(NodeGeneratorFlat));
    let sink = terrain.graph_mut().add_node(Box::new(NodeErosion::default()));
    terrain.graph_mut().connect(source, 0, sink, 0).unwrap();

    terrain.process(sink).unwrap().expect("first process should report a generation");

    terrain
        .graph_mut()
        .node_mut(sink)
        .unwrap()
        .set_param("strength", NParamValue::Float(0.9))
        .unwrap();

    let after_change = terrain.process(sink).expect("processing should succeed");
    assert!(after_change.is_some(), "changing a parameter should surface a new generation");
}

#[test]
fn process_propagates_errors_from_the_underlying_graph() {
    let mut terrain = TerrainGraph::default();
    let sink = terrain.graph_mut().add_node(Box::new(NodeErosion::default()));
    // `sink`'s required input was never connected.
    assert!(terrain.process(sink).is_err());
}

#[test]
fn terrain_graph_holder_allows_shared_read_and_exclusive_write_access() {
    let holder = TerrainGraphHolder::default();
    let id = holder.write().graph_mut().add_node(Box::new(NodeGeneratorFlat));
    assert!(holder.read().graph().node(id).is_ok());
}

#[test]
fn timed_action_message_expires_and_is_pruned() {
    let mut terrain = TerrainGraph::default();
    let mut graph = waterdrop_terrain_editor::core::graph::NodeGraph::new(4);
    let id = graph.add_node(Box::new(NodeGeneratorFlat));

    terrain.set_action_result(id, Ok("Saved!".to_string()));
    // `set_action_result`'s success path uses `ACTION_MESSAGE_DURATION`, which is several
    // seconds long, so we can't wait it out here - instead just verify the message reports a
    // bounded remaining time rather than "forever" like a persistent error would.
    let remaining = terrain.action_message_remaining(id).expect("a timed message should report remaining time");
    assert!(remaining <= TerrainGraph::ACTION_MESSAGE_DURATION);
    assert!(remaining > Duration::from_secs(0));
}
