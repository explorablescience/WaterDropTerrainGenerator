use std::sync::Arc;

use waterdrop_terrain_editor::core::node::Node;
use waterdrop_terrain_editor::core::tile_allocator::TilePool;
use waterdrop_terrain_editor::nodes::NodeErosion;

#[test]
fn test_node_erosion() {
    let pool = TilePool::new(9); // 3x3 tile
    let mut input = pool.allocate();
    input.copy_from_slice(&[0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]);

    let erosion = NodeErosion::default();
    let output = erosion
        .process(&pool, &[Arc::new(input)])
        .expect("erosion should succeed on a matching tile");

    // The centre texel should have moved halfway towards its neighbours' average (0.0).
    assert_eq!(output.len(), 1);
    assert!((output[0][4] - 0.5).abs() < 1e-6);
    // A corner texel (no raised neighbour) should stay untouched.
    assert_eq!(output[0][0], 0.0);
}
