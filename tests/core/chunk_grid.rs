use waterdrop_terrain_generator::core::chunk_grid::{ChunkCoord, ChunkGrid};
use waterdrop_terrain_generator::core::tile_context::TileContext;

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

#[test]
fn global_context_is_centered_on_its_own_local_grid_independent_of_the_chunk_grid() {
    // A `Global` node's context never depends on the terrain's chunk grid at all - it's always
    // the same fixed, self-centered `[-0.5, 0.5)` domain, however the terrain itself is laid out.
    let ctx = TileContext::for_global(4);
    assert_eq!(ctx.chunk, None);
    assert_eq!(ctx.world_origin, (-0.5, -0.5));
    assert_eq!(ctx.world_step, (0.25, 0.25));
    assert_eq!(ctx.world_extent, (1.0, 1.0));

    // Its center texel sits at (or right next to) local (0, 0), not the grid's own origin.
    let (cx, cy) = ctx.world_pos(2, 2);
    assert!(
        cx.abs() < 1e-6 && cy.abs() < 1e-6,
        "texel (2, 2) of a 4x4 global context should be at local (0, 0)"
    );
}
