use waterdrop_terrain_generator::core::tiling::{ChunkCoord, ChunkGrid};

#[test]
fn single_covers_the_whole_terrain_in_one_chunk_spanning_one_world_unit() {
    let grid = ChunkGrid::single(128);
    assert_eq!(grid.chunk_count(), 1);
    assert_eq!(grid.world_extent(), (1.0, 1.0));
}

#[test]
fn chunk_count_is_the_product_of_both_axes() {
    let grid = ChunkGrid::new(3, 2, 16, 1.0);
    assert_eq!(grid.chunk_count(), 6);
    assert_eq!(grid.coords().count(), 6);
}

#[test]
fn coords_are_listed_in_row_major_order() {
    let grid = ChunkGrid::new(2, 2, 16, 1.0);
    let coords: Vec<ChunkCoord> = grid.coords().collect();
    assert_eq!(
        coords,
        vec![
            ChunkCoord(0, 0),
            ChunkCoord(1, 0),
            ChunkCoord(0, 1),
            ChunkCoord(1, 1)
        ]
    );
}

#[test]
fn world_extent_scales_with_chunk_count_tile_size_and_world_scale() {
    let grid = ChunkGrid::new(4, 2, 32, 0.5);
    assert_eq!(grid.world_extent(), (4.0 * 32.0 * 0.5, 2.0 * 32.0 * 0.5));
}

#[test]
fn world_space_is_centered_on_the_whole_grid_not_a_corner() {
    // An odd chunk count has a single, unambiguous center chunk - its core should span
    // symmetrically around world (0, 0).
    let grid = ChunkGrid::new(3, 3, 8, 1.0);
    let ctx = grid.chunk_context(ChunkCoord(1, 1), 0);
    assert_eq!(
        ctx.world_origin,
        (-4.0, -4.0),
        "the center chunk's core should start at -half its own width"
    );

    // An even chunk count centers on the shared corner of the four middle chunks instead.
    let grid = ChunkGrid::new(2, 2, 8, 1.0);
    let ctx = grid.chunk_context(ChunkCoord(1, 1), 0);
    assert_eq!(
        ctx.world_origin,
        (0.0, 0.0),
        "chunk (1, 1) of a 2x2 grid should start exactly at the origin"
    );
}

#[test]
fn chunk_context_offsets_the_origin_by_the_requested_margin() {
    let grid = ChunkGrid::new(2, 1, 8, 1.0);
    let ctx = grid.chunk_context(ChunkCoord(1, 0), 2);
    // World extent is 16x8; chunk 1's core starts at world x = 8 before centering, which becomes
    // x = 0 once centered (extent_x / 2 = 8); y centers the same way, to -4. A 2-texel margin at
    // world_scale 1.0 then pulls the buffer's own (0, 0) texel two world units further back on
    // each axis.
    assert_eq!(ctx.world_origin, (-2.0, -6.0));
    assert_eq!(ctx.world_step, (1.0, 1.0));
}
