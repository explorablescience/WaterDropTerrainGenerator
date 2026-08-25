use waterdrop_terrain_editor::core::graph::{NodeGraph, NodeGraphProcessResult};
use waterdrop_terrain_editor::nodes::{NodeErosion, NodeGeneratorFlat, NodeGeneratorPerlin};

#[test]
fn test_node_graph_connections() {
    let mut graph = NodeGraph::new(32);
    let (node_a, node_b) = (
        graph.add_node(Box::new(NodeGeneratorPerlin::default())),
        graph.add_node(Box::new(NodeErosion::default()))
    );

    // Valid connection
    let result = graph.connect(node_a, 0, node_b, 0);
    assert!(result.is_ok(), "Graph connection should succeed");

    // Invalid connection: NodeErosion has only one input socket (index 0)
    let result_invalid = graph.connect(node_a, 0, node_b, 1);
    assert!(
        result_invalid.is_err(),
        "Graph connection should fail due to invalid socket index"
    );
}

#[test]
fn test_node_graph_validation() {
    let mut graph = NodeGraph::new(32);
    let (node_a, node_b, node_c) = (
        graph.add_node(Box::new(NodeGeneratorPerlin::default())),
        graph.add_node(Box::new(NodeErosion::default())),
        graph.add_node(Box::new(NodeErosion::default()))
    );
    graph
        .connect(node_a, 0, node_b, 0)
        .and_then(|g| g.connect(node_b, 0, node_c, 0))
        .expect("Graph connections should succeed");
}

#[test]
fn test_node_graph_cycle_detection() {
    let mut graph = NodeGraph::new(32);
    let (node_a, node_b) = (
        graph.add_node(Box::new(NodeErosion::default())),
        graph.add_node(Box::new(NodeErosion::default()))
    );
    graph
        .connect(node_a, 0, node_b, 0)
        .and_then(|g| g.connect(node_b, 0, node_a, 0)) // This creates a cycle
        .expect("Graph connections should succeed");

    // Run the graph and expect a cycle detection error
    let result = graph.process(node_b);
    assert!(result.is_err(), "Graph validation should fail due to cycle");
}

#[test]
fn test_node_graph_remove_node_disconnects_edges() {
    let mut graph = NodeGraph::new(32);
    let (source, erosion) = (
        graph.add_node(Box::new(NodeGeneratorFlat)),
        graph.add_node(Box::new(NodeErosion::default()))
    );
    graph
        .connect(source, 0, erosion, 0)
        .expect("Graph connection should succeed");

    graph
        .remove_node(source)
        .expect("Removing an existing node should succeed");

    // The dangling input edge should be gone, so erosion now has an unconnected input.
    let result = graph.process(erosion);
    assert!(
        result.is_err(),
        "Processing should fail once the upstream node feeding erosion is removed"
    );

    // The node itself is gone too.
    assert!(
        graph.node(source).is_err(),
        "Removed node should no longer be reachable"
    );
}

#[test]
fn test_node_graph_remove_node_resets_cached_topo() {
    let mut graph = NodeGraph::new(32);
    let (source, erosion) = (
        graph.add_node(Box::new(NodeGeneratorFlat)),
        graph.add_node(Box::new(NodeErosion::default()))
    );
    graph
        .connect(source, 0, erosion, 0)
        .expect("Graph connection should succeed");

    graph
        .remove_node(source)
        .expect("Removing an existing node should succeed");

    // The cached topo order from before the removal must not be reused.
    let result = graph.process(erosion);
    assert!(
        result.is_err(),
        "Processing should require re-validation after a node is removed"
    );
}

#[test]
fn test_node_graph_remove_node_unknown_id_errors() {
    let mut graph = NodeGraph::new(32);
    let node = graph.add_node(Box::new(NodeGeneratorFlat));
    graph
        .remove_node(node)
        .expect("First removal should succeed");

    let result = graph.remove_node(node);
    assert!(
        result.is_err(),
        "Removing an already-removed node should fail"
    );
}

#[test]
fn test_node_graph_process_grows_internal_tile_size_for_padding() {
    let tile_size = 8;
    let mut graph = NodeGraph::new(8);
    let (source, erosion) = (
        graph.add_node(Box::new(NodeGeneratorFlat)),
        graph.add_node(Box::new(NodeErosion::default())) // size() == 3 -> padding of 2
    );
    graph
        .connect(source, 0, erosion, 0)
        .expect("Graph connection should succeed");

    let outputs = graph
        .process(erosion)
        .expect("Graph processing should succeed");
    let outputs = match outputs {
        NodeGraphProcessResult::Processed(_, outputs) => outputs,
        _ => panic!("Graph processing should have completed")
    };

    // internal_tile_size = tile_size + 2*padding(erosion) + 2*padding(source)
    //                     = 8 + 2*2 + 2*0 = 12
    let expected_internal_tile_size = tile_size + 2 * 2;
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        outputs[0].len(),
        expected_internal_tile_size * expected_internal_tile_size
    );
}
