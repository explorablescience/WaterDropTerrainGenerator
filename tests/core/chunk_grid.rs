use waterdrop_terrain_generator::core::chunk_grid::{ChunkCoord, ChunkGrid};

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
        vec![ChunkCoord(0, 0), ChunkCoord(1, 0), ChunkCoord(0, 1), ChunkCoord(1, 1)]
    );
}

#[test]
fn world_extent_scales_with_chunk_count_tile_size_and_world_scale() {
    let grid = ChunkGrid::new(4, 2, 32, 0.5);
    assert_eq!(grid.world_extent(), (4.0 * 32.0 * 0.5, 2.0 * 32.0 * 0.5));
}

#[test]
fn chunk_context_offsets_the_origin_by_the_requested_margin() {
    let grid = ChunkGrid::new(2, 1, 8, 1.0);
    let ctx = grid.chunk_context(ChunkCoord(1, 0), 2);
    // Chunk 1's core starts at world x = 8; a 2-texel margin at world_scale 1.0 pulls the
    // buffer's own (0, 0) texel two world units further back.
    assert_eq!(ctx.world_origin, (6.0, -2.0));
    assert_eq!(ctx.world_step, (1.0, 1.0));
}

#[test]
fn whole_context_covers_the_full_extent_at_the_requested_resolution() {
    let grid = ChunkGrid::new(2, 2, 8, 1.0);
    let ctx = grid.whole_context(4);
    assert_eq!(ctx.chunk, None);
    assert_eq!(ctx.world_origin, (0.0, 0.0));
    // The whole 16x16-world-unit extent is covered by 4 texels per axis.
    assert_eq!(ctx.world_step, (4.0, 4.0));
}
