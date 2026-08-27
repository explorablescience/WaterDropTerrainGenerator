use std::collections::HashMap;
use std::path::PathBuf;

use waterdrop_terrain_generator::core::chunk_grid::ChunkGrid;
use waterdrop_terrain_generator::core::graph::NodeGraph;
use waterdrop_terrain_generator::core::node_parameters::NParamValue;
use waterdrop_terrain_generator::core::node_registry;
use waterdrop_terrain_generator::core::project::{load_project, save_project};
use waterdrop_terrain_generator::nodes::{NodeErosion, NodeGeneratorPerlin};

/// A path in the system temp dir unique to this test process/run, so parallel test runs never
/// collide on the same file. Removed by the caller once the test is done with it.
fn temp_project_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("wdtg-test-{}-{}.wdtg", std::process::id(), name))
}

#[test]
fn round_trips_nodes_params_positions_and_edges() {
    let path = temp_project_path("round-trip");

    let mut graph = NodeGraph::new(32);
    let source = graph.add_node(Box::new(NodeGeneratorPerlin::default()));
    let sink = graph.add_node(Box::new(NodeErosion::default()));
    graph
        .connect(source, 0, sink, 0)
        .expect("connecting a valid pair of sockets should succeed");
    graph
        .node_mut(source)
        .expect("just-added node should exist")
        .set_param("frequency", NParamValue::Float(2.5))
        .expect("Perlin Generator should have a 'frequency' parameter");

    let mut positions = HashMap::new();
    positions.insert(source, [10.0, 20.0]);
    positions.insert(sink, [110.0, 20.0]);

    save_project(&path, &graph, &positions).expect("saving a valid graph should succeed");
    let mut built = load_project(&path, 32).expect("loading what was just saved should succeed");
    std::fs::remove_file(&path).ok();

    let ids: Vec<_> = built.graph.node_ids().collect();
    assert_eq!(ids.len(), 2, "both nodes should be present after reload");

    let loaded_source = ids
        .iter()
        .copied()
        .find(|&id| built.graph.node(id).unwrap().label() == "Perlin Generator")
        .expect("the Perlin Generator node should be present after reload");
    let loaded_sink = ids
        .iter()
        .copied()
        .find(|&id| built.graph.node(id).unwrap().label() == "Erosion")
        .expect("the Erosion node should be present after reload");

    assert_eq!(
        built
            .graph
            .node(loaded_source)
            .unwrap()
            .get_param("frequency"),
        Some(NParamValue::Float(2.5)),
        "parameter value set before saving should round-trip"
    );
    assert_eq!(
        built.positions[&loaded_source],
        [10.0, 20.0],
        "editor position should round-trip"
    );
    assert_eq!(built.positions[&loaded_sink], [110.0, 20.0]);
    assert_eq!(
        built.edges,
        vec![(loaded_source, 0, loaded_sink, 0)],
        "connection should round-trip"
    );

    // The reloaded graph should be as processable as the original: erosion requires its input to
    // be connected, and it is.
    assert!(
        built.graph.process(loaded_sink).is_ok(),
        "reloaded graph should process successfully"
    );
}

#[test]
fn empty_graph_round_trips_to_no_nodes() {
    let path = temp_project_path("empty");

    let graph = NodeGraph::new(32);
    save_project(&path, &graph, &HashMap::new()).expect("saving an empty graph should succeed");
    let built = load_project(&path, 32).expect("loading an empty project should succeed");
    std::fs::remove_file(&path).ok();

    assert_eq!(built.graph.node_ids().count(), 0);
    assert!(built.edges.is_empty());
}

#[test]
fn unsaved_node_position_defaults_to_origin() {
    let path = temp_project_path("missing-position");

    let mut graph = NodeGraph::new(32);
    let node = graph.add_node(Box::new(NodeGeneratorPerlin::default()));

    // Deliberately omit `node`'s position, as if the caller's UI never had one for it.
    save_project(&path, &graph, &HashMap::new())
        .expect("saving should succeed even without positions");
    let built = load_project(&path, 32).expect("loading should succeed");
    std::fs::remove_file(&path).ok();

    let loaded = built
        .graph
        .node_ids()
        .next()
        .expect("the node should still be saved");
    assert_eq!(built.positions[&loaded], [0.0, 0.0]);
    let _ = node; // only used to build the graph above
}

#[test]
fn unknown_node_type_fails_to_load() {
    let path = temp_project_path("bad-type");
    std::fs::write(
        &path,
        r#"{"version":1,"nodes":[{"id":0,"node_type":"Does Not Exist","position":[0.0,0.0],"params":[]}],"edges":[]}"#
    )
    .expect("writing the test fixture should succeed");

    let result = load_project(&path, 32);
    std::fs::remove_file(&path).ok();
    assert!(
        result.is_err(),
        "loading a project with an unregistered node type should fail"
    );
}

#[test]
fn malformed_json_fails_to_load() {
    let path = temp_project_path("malformed");
    std::fs::write(&path, "not valid json").expect("writing the test fixture should succeed");

    let result = load_project(&path, 32);
    std::fs::remove_file(&path).ok();
    assert!(
        result.is_err(),
        "loading malformed JSON should fail rather than panic"
    );
}

#[test]
fn missing_file_fails_to_load() {
    let path = temp_project_path("does-not-exist");
    assert!(load_project(&path, 32).is_err());
}

#[test]
fn legacy_v1_project_without_a_chunk_grid_loads_as_a_single_chunk() {
    let path = temp_project_path("legacy-v1");
    std::fs::write(&path, r#"{"version":1,"nodes":[],"edges":[]}"#)
        .expect("writing the test fixture should succeed");

    let built =
        load_project(&path, 64).expect("a v1 project without a chunk_grid field should still load");
    std::fs::remove_file(&path).ok();

    let grid = built.graph.chunk_grid();
    assert_eq!((grid.chunks_x(), grid.chunks_y()), (1, 1));
    assert_eq!(
        grid.tile_size(),
        64,
        "the caller-supplied tile size should back the fallback grid"
    );
}

#[test]
fn chunk_grid_round_trips_through_save_and_load() {
    let path = temp_project_path("chunk-grid-round-trip");

    let graph = NodeGraph::new(ChunkGrid::new(3, 2, 16, 0.5));
    save_project(&path, &graph, &HashMap::new()).expect("saving should succeed");
    let built = load_project(&path, 16).expect("loading should succeed");
    std::fs::remove_file(&path).ok();

    let grid = built.graph.chunk_grid();
    assert_eq!((grid.chunks_x(), grid.chunks_y()), (3, 2));
    assert_eq!(grid.tile_size(), 16);
    assert_eq!(grid.world_scale(), 0.5);
}

/// Regression test for a bug where `NodeGeneratorFlat::label()` returned a different string than
/// the label it was registered under in `node_registry`. `core::project` identifies a saved
/// node's type by matching `Node::label()` against each `NodeDescriptor::label`, so any node type
/// where the two diverge silently fails to round-trip ("Unknown node type '...'") even though it
/// appears correctly in the "Add Node" menu.
#[test]
fn every_registered_node_labels_itself_consistently_with_its_registry_entry() {
    let mismatched: Vec<String> = node_registry::registered_nodes()
        .filter_map(|descriptor| {
            let instance = (descriptor.factory)();
            (instance.label() != descriptor.label).then(|| {
                format!(
                    "registered as '{}' but Node::label() returns '{}'",
                    descriptor.label,
                    instance.label()
                )
            })
        })
        .collect();

    assert!(
        mismatched.is_empty(),
        "every registered node type must return its own registry label from Node::label(), \
         so saved projects can find it again by type name; mismatches: {:?}",
        mismatched
    );
}
