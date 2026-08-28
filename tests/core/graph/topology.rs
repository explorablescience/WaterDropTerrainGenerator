use std::sync::Arc;

use waterdrop_terrain_generator::core::graph::{NodeGraph, NodeGraphProcessResult};
use waterdrop_terrain_generator::core::node::{
    Node, NodeCategory, NodeError, NodeIcon, NodeLocality, NodePortType, NodeSocket
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

/// A `Global` source with a configurable resolution, used to test that connecting across
/// locality/resolution boundaries is always accepted - the graph engine resamples automatically.
#[derive(Debug)]
struct FakeGlobalHeightSource {
    native_resolution: usize
}
impl Node for FakeGlobalHeightSource {
    fn label(&self) -> &str {
        "Fake Global Height Source"
    }
    fn category(&self) -> NodeCategory {
        NodeCategory::Generation
    }
    fn icon(&self) -> NodeIcon {
        TEST_ICON
    }
    fn locality(&self) -> NodeLocality {
        NodeLocality::Global {
            native_resolution: self.native_resolution
        }
    }
    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: NodePortType::Height,
            required: true
        }]
    }
}

#[test]
fn connecting_a_global_node_directly_into_a_local_socket_succeeds() {
    // No manual placement node needed: the graph engine resamples a `Global` ancestor's
    // whole-terrain tile into whatever frame a `Local` consumer needs.
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let source = graph.add_node(Box::new(FakeGlobalHeightSource {
        native_resolution: 8
    }));
    let sink = graph.add_node(Box::new(Erosion::default()));

    assert!(graph.connect(source, 0, sink, 0).is_ok());
}

#[test]
fn connecting_two_global_nodes_with_different_native_resolution_succeeds() {
    // Resampling handles resolution mismatches between two `Global` nodes just as it does between
    // a `Global` node and a `Local` one.
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let source = graph.add_node(Box::new(FakeGlobalHeightSource {
        native_resolution: 8
    }));
    let sink = graph.add_node(Box::new(HydraulicErosion::default())); // native_resolution: 256

    assert!(graph.connect(source, 0, sink, 0).is_ok());
}

#[test]
fn connecting_a_local_node_into_a_global_nodes_input_succeeds() {
    // The graph's normal "bake a local generator once at global scope" pattern: a `Global` node
    // pulling in a `Local` ancestor evaluates it once, in its own real-world-aligned frame.
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let source = graph.add_node(Box::new(Perlin::default()));
    let sink = graph.add_node(Box::new(HydraulicErosion::default()));

    assert!(graph.connect(source, 0, sink, 0).is_ok());
}

#[test]
fn connecting_mismatched_socket_types_fails() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let source = graph.add_node(Box::new(FakeHeightSource));
    let sink = graph.add_node(Box::new(FakeMaskSink));

    let result = graph.connect(source, 0, sink, 0);
    assert!(matches!(result, Err(NodeError::SocketTypeMismatch { .. })));
}

#[test]
fn connecting_to_an_out_of_range_output_socket_fails() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
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
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let source = graph.add_node(Box::new(FakeHeightSource));
    let sink = graph.add_node(Box::new(Erosion::default()));

    let result = graph.connect(source, 0, sink, 5);
    assert!(matches!(result, Err(NodeError::InputSocketNotFound { .. })));
}

#[test]
fn optional_unconnected_input_is_fed_a_neutral_zero_tile() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
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
fn disconnecting_an_edge_that_does_not_exist_fails() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let source = graph.add_node(Box::new(Flat));
    let sink = graph.add_node(Box::new(Erosion::default()));

    let result = graph.disconnect(source, 0, sink, 0);
    assert!(matches!(result, Err(NodeError::NotConnected { .. })));
}

#[test]
fn processing_an_unknown_node_id_fails() {
    let mut graph = NodeGraph::new(ChunkGrid::new(1, 1, 4, 1.0 / 4.0));
    let id = graph.add_node(Box::new(Flat));
    graph.remove_node(id).unwrap();
    assert!(graph.process(id).is_err());
}
