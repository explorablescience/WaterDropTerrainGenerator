use std::sync::Arc;

use waterdrop_terrain_generator::core::graph::NodeGraph;
use waterdrop_terrain_generator::core::node::{
    Node, NodeCategory, NodeError, NodeIcon, NodePortType, NodeSocket
};
use waterdrop_terrain_generator::core::session::assemble_terrain;
use waterdrop_terrain_generator::core::tiling::{ChunkGrid, TileContext, TileHandle, TilePool};

const TEST_ICON: NodeIcon = NodeIcon {
    id: "test-icon",
    png_bytes: &[]
};

/// A node whose output encodes each texel's world-space x position, so a test can check that
/// `assemble_terrain` places each chunk's cropped core at the right offset in the stitched buffer.
#[derive(Debug, Default)]
struct FakeWorldXMarker;
impl Node for FakeWorldXMarker {
    fn label(&self) -> &str {
        "Fake World X Marker"
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
        ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        let mut output = pool.allocate();
        let s = output.size();
        for y in 0..s {
            for x in 0..s {
                output[y * s + x] = ctx.world_pos(x, y).0;
            }
        }
        Ok(vec![Arc::new(output)])
    }
}

#[test]
fn assembles_each_chunks_core_region_at_its_grid_offset() {
    let mut graph = NodeGraph::new(ChunkGrid::new(2, 1, 4, 1.0));
    let node = graph.add_node(Box::new(FakeWorldXMarker));

    let (data, width, height) =
        assemble_terrain(&mut graph, node).expect("assembling should succeed");
    assert_eq!((width, height), (8, 4));

    // World space is centered on the whole grid, so the 8-wide combined extent spans [-4, 4).
    for y in 0..height {
        let row = &data[y * width..(y + 1) * width];
        assert_eq!(
            row,
            [-4.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0],
            "row {y} should be a continuous world-x ramp across both chunks, not each chunk restarting at 0"
        );
    }
}

#[test]
fn a_single_chunk_grid_assembles_to_exactly_the_tile_size() {
    let mut graph = NodeGraph::new(ChunkGrid::single(4));
    let node = graph.add_node(Box::new(FakeWorldXMarker));

    let (data, width, height) =
        assemble_terrain(&mut graph, node).expect("assembling should succeed");
    assert_eq!((width, height), (4, 4));
    assert_eq!(data.len(), 16);
}
