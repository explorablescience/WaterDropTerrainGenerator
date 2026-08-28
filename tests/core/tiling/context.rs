use waterdrop_terrain_generator::core::tiling::{ChunkGrid, TileContext};

#[test]
fn global_context_covers_the_same_real_world_extent_as_the_chunk_grid() {
    // A `Global` node's frame isn't some arbitrary self-contained domain - it's the exact same
    // real world extent the chunked terrain covers, just evaluated as one whole-terrain tile at
    // its own `native_resolution` instead of being split across chunks. That's what lets a
    // `Local` ancestor pulled into a `Global` pass sample the same coordinates it would per chunk.
    let grid = ChunkGrid::new(2, 3, 8, 0.5);
    let expected_extent = grid.world_extent();

    let ctx = TileContext::for_global(&grid, 4);
    assert_eq!(ctx.chunk, None);
    assert_eq!(ctx.world_extent, expected_extent);
    assert_eq!(
        ctx.world_origin,
        (-expected_extent.0 * 0.5, -expected_extent.1 * 0.5)
    );
    assert_eq!(
        ctx.world_step,
        (expected_extent.0 / 4.0, expected_extent.1 / 4.0)
    );
}

#[test]
fn global_context_is_centered_on_the_terrain_regardless_of_native_resolution() {
    let grid = ChunkGrid::new(1, 1, 4, 1.0);

    // Its center texel sits at (or right next to) world (0, 0), the same origin the chunk grid
    // itself is centered on - whatever resolution it's sampled at.
    for native_resolution in [4usize, 16, 64] {
        let ctx = TileContext::for_global(&grid, native_resolution);
        let center = native_resolution / 2;
        let (cx, cy) = ctx.world_pos(center, center);
        assert!(
            cx.abs() < 1e-4 && cy.abs() < 1e-4,
            "native_resolution {}: expected the center texel near world (0, 0), got ({}, {})",
            native_resolution,
            cx,
            cy
        );
    }
}

#[test]
fn to_texel_is_the_inverse_of_world_pos() {
    let grid = ChunkGrid::new(2, 1, 4, 1.0);
    let ctx = TileContext::for_global(&grid, 16);
    for (x, y) in [(0, 0), (3, 7), (15, 15)] {
        let pos = ctx.world_pos(x, y);
        let (tx, ty) = ctx.to_texel(pos);
        assert!(
            (tx - x as f32).abs() < 1e-4,
            "expected texel x {}, got {}",
            x,
            tx
        );
        assert!(
            (ty - y as f32).abs() < 1e-4,
            "expected texel y {}, got {}",
            y,
            ty
        );
    }
}
