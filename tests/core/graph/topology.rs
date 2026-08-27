use std::sync::Arc;

use waterdrop_terrain_generator::core::graph::{NodeGraph, NodeGraphProcessResult};
use waterdrop_terrain_generator::core::node::{
    Node, NodeCategory, NodeError, NodeIcon, NodePortType, NodeSocket
};
use waterdrop_terrain_generator::core::tiling::{ChunkGrid, TileContext, TileHandle, TilePool};
use waterdrop_terrain_generator::nodes::*;

const TEST_ICON: NodeIcon = NodeIcon {
    id: "test-icon",
    png_bytes: &[]
};

/// A minimal source node with a single required `Height` output, used to exercise graph wiring
/// without depending on any of the "real" nodes' own processing behaviour.
#[derive(Debug, Default)]
struct FakeHeightSource;
impl Node for FakeHeightSource {
    fn label(&self) -> &str {
        "Fake Height Source"
    }
    fn category(&self) -> NodeCategory {
        NodeCategory::Generation
    }
    fn icon(&self) -> NodeIcon {
        TEST_ICON
    }
    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: NodePortType::Height,
            required: true
        }]
    }
    fn process(
        &self,
        pool: &Arc<TilePool>,
        _inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![Arc::new(pool.allocate())])
    }
}

/// A sink node with a single required `Mask` input, used to prove that connecting mismatched
/// socket types is rejected.
#[derive(Debug, Default)]
struct FakeMaskSink;
impl Node for FakeMaskSink {
    fn label(&self) -> &str {
        "Fake Mask Sink"
    }
    fn category(&self) -> NodeCategory {
        NodeCategory::Modification
    }
    fn icon(&self) -> NodeIcon {
        TEST_ICON
    }
    fn inputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Mask",
            dtype: NodePortType::Mask,
            required: true
        }]
    }
}

/// A sink node with a single *optional* `Height` input, whose output tells the test whether the
/// input tile it received was the pool's zero-filled neutral tile.
#[derive(Debug, Default)]
struct FakeOptionalSink;
impl Node for FakeOptionalSink {
    fn label(&self) -> &str {
        "Fake Optional Sink"
    }
    fn category(&self) -> NodeCategory {
        NodeCategory::Modification
    }
    fn icon(&self) -> NodeIcon {
        TEST_ICON
    }
    fn inputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: NodePortType::Height,
            required: false
        }]
    }
    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: NodePortType::Height,
            required: true
        }]
    }
    fn process(
        &self,
        _pool: &Arc<TilePool>,
        inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![inputs[0].clone()])
    }
}

#[test]
fn connecting_mismatched_socket_types_fails() {
    let mut graph = NodeGraph::new(ChunkGrid::single(4));
    let source = graph.add_node(Box::new(FakeHeightSource));
    let sink = graph.add_node(Box::new(FakeMaskSink));

    let result = graph.connect(source, 0, sink, 0);
    assert!(matches!(result, Err(NodeError::SocketTypeMismatch { .. })));
}

#[test]
fn connecting_to_an_out_of_range_output_socket_fails() {
    let mut graph = NodeGraph::new(ChunkGrid::single(4));
    let source = graph.add_node(Box::new(FakeHeightSource));
    let sink = graph.add_node(Box::new(Erosion::default()));

    let result = graph.connect(source, 5, sink, 0);
    assert!(matches!(
        result,
        Err(NodeError::OutputSocketNotFound { .. })
    ));
}

#[test]
fn connecting_to_an_out_of_range_input_socket_fails() {
    let mut graph = NodeGraph::new(ChunkGrid::single(4));
    let source = graph.add_node(Box::new(FakeHeightSource));
    let sink = graph.add_node(Box::new(Erosion::default()));

    let result = graph.connect(source, 0, sink, 5);
    assert!(matches!(result, Err(NodeError::InputSocketNotFound { .. })));
}

#[test]
fn optional_unconnected_input_is_fed_a_neutral_zero_tile() {
    let mut graph = NodeGraph::new(ChunkGrid::single(4));
    let sink = graph.add_node(Box::new(FakeOptionalSink));

    let result = graph
        .process(sink)
        .expect("processing with an unconnected optional input should succeed");
    let NodeGraphProcessResult::Processed(_, outputs) = result else {
        panic!("expected the graph to finish processing")
    };
    assert!(outputs[0].iter().all(|&v| v == 0.0));
}

#[test]
fn connecting_a_second_source_to_an_occupied_input_replaces_the_first() {
    let mut graph = NodeGraph::new(ChunkGrid::single(4));
    let a = graph.add_node(Box::new(Flat));
    let b = graph.add_node(Box::new(Perlin::default()));
    let sink = graph.add_node(Box::new(Erosion::default()));

    graph
        .connect(a, 0, sink, 0)
        .expect("first connection should succeed");
    graph
        .connect(b, 0, sink, 0)
        .expect("replacing the first connection should succeed");

    let inputs = graph.inputs(sink).expect("sink should exist");
    assert_eq!(
        inputs,
        &[Some((b, 0))],
        "the input should now come from b, not a"
    );
}

#[test]
fn disconnect_removes_the_edge_and_leaves_the_input_socket_empty() {
    let mut graph = NodeGraph::new(ChunkGrid::single(4));
    let source = graph.add_node(Box::new(Flat));
    let sink = graph.add_node(Box::new(Erosion::default()));
    graph.connect(source, 0, sink, 0).unwrap();

    graph
        .disconnect(source, 0, sink, 0)
        .expect("disconnecting an existing edge should succeed");
    assert_eq!(graph.inputs(sink).unwrap(), &[None]);

    // Processing should now fail again since the required input is gone.
    assert!(graph.process(sink).is_err());
}

#[test]
fn disconnecting_an_edge_that_does_not_exist_fails() {
    let mut graph = NodeGraph::new(ChunkGrid::single(4));
    let source = graph.add_node(Box::new(Flat));
    let sink = graph.add_node(Box::new(Erosion::default()));

    let result = graph.disconnect(source, 0, sink, 0);
    assert!(matches!(result, Err(NodeError::NotConnected { .. })));
}

#[test]
fn node_ids_skips_removed_nodes_but_keeps_surviving_ids_stable() {
    let mut graph = NodeGraph::new(ChunkGrid::single(4));
    let a = graph.add_node(Box::new(Flat));
    let b = graph.add_node(Box::new(Flat));
    let c = graph.add_node(Box::new(Flat));
    graph.remove_node(b).unwrap();

    let ids: Vec<_> = graph.node_ids().collect();
    assert_eq!(ids, vec![a, c]);
}

#[test]
fn processing_an_unknown_node_id_fails() {
    let mut graph = NodeGraph::new(ChunkGrid::single(4));
    let id = graph.add_node(Box::new(Flat));
    graph.remove_node(id).unwrap();
    assert!(graph.process(id).is_err());
}
