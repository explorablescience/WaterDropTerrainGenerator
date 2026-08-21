use waterdrop_terrain_editor::core::graph::NodeGraph;
use waterdrop_terrain_editor::nodes::{NodeErosion, NodeGeneratorFlat, NodeGeneratorPerlin};

#[test]
fn test_node_graph_connections() {
    let mut graph = NodeGraph::default();
    let (node_a, node_b) = (
        graph.add_node(Box::new(NodeGeneratorPerlin::default())),
        graph.add_node(Box::new(NodeErosion::default())),
    );

    // Valid connection
    let result = graph.connect(node_a, 0, node_b, 0);
    assert!(result.is_ok(), "Graph connection should succeed");

    // Invalid connection: NodeErosion has only one input socket (index 0)
    let result_invalid = graph.connect(node_a, 0, node_b, 1);
    assert!(result_invalid.is_err(), "Graph connection should fail due to invalid socket index");
}

#[test]
fn test_node_graph_validation() {
    let mut graph = NodeGraph::new();
    let (node_a, node_b, node_c) = (
        graph.add_node(Box::new(NodeGeneratorPerlin::default())),
        graph.add_node(Box::new(NodeErosion::default())),
        graph.add_node(Box::new(NodeErosion::default())),
    );
    graph
        .connect(node_a, 0, node_b, 0)
        .and_then(|g| g.connect(node_b, 0, node_c, 0))
        .expect("Graph connections should succeed");
}

#[test]
fn test_node_graph_cycle_detection() {
    let mut graph = NodeGraph::new();
    let (node_a, node_b) = (
        graph.add_node(Box::new(NodeErosion::default())),
        graph.add_node(Box::new(NodeErosion::default())),
    );
    graph
        .connect(node_a, 0, node_b, 0)
        .and_then(|g| g.connect(node_b, 0, node_a, 0)) // This creates a cycle
        .expect("Graph connections should succeed");

    // Validate the graph and expect an error due to the cycle
    let result = graph.validate(node_b);
    assert!(result.is_err(), "Graph validation should fail due to cycle");
}

#[test]
fn test_node_graph_process_requires_prior_validate() {
    let mut graph = NodeGraph::new();
    let (source, erosion) = (
        graph.add_node(Box::new(NodeGeneratorFlat)),
        graph.add_node(Box::new(NodeErosion::default())),
    );
    graph
        .connect(source, 0, erosion, 0)
        .expect("Graph connection should succeed");

    let result = graph.process(erosion, 8);
    assert!(
        result.is_err(),
        "Processing before validate() should fail"
    );
}

#[test]
fn test_node_graph_process_grows_internal_tile_size_for_padding() {
    let mut graph = NodeGraph::new();
    let (source, erosion) = (
        graph.add_node(Box::new(NodeGeneratorFlat)),
        graph.add_node(Box::new(NodeErosion::default())), // size() == 3 -> padding of 2
    );
    graph
        .connect(source, 0, erosion, 0)
        .expect("Graph connection should succeed");
    graph.validate(erosion).expect("Graph should be valid");

    let tile_size = 8;
    let outputs = graph
        .process(erosion, tile_size)
        .expect("Graph processing should succeed");

    // internal_tile_size = tile_size + 2*padding(erosion) + 2*padding(source)
    //                     = 8 + 2*2 + 2*0 = 12
    let expected_internal_tile_size = tile_size + 2 * 2;
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        outputs[0].len(),
        expected_internal_tile_size * expected_internal_tile_size
    );
}
