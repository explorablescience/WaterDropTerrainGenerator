use waterdrop_terrain_editor::core::tile_allocator::TilePool;

#[test]
fn allocated_tile_is_zero_filled_and_sized_to_the_pool() {
    let pool = TilePool::new(4);
    let tile = pool.allocate();
    // `size()` is the tile's side length (texels per row/column), not its total element count.
    assert_eq!(tile.size(), 4);
    assert_eq!(tile.len(), 16);
    assert!(tile.iter().all(|&v| v == 0.0));
    assert!(!tile.is_empty());
}

#[test]
fn tile_length_reports_the_side_length_the_pool_was_created_with() {
    let pool = TilePool::new(7);
    assert_eq!(pool.tile_length(), 7);
}

#[test]
fn deref_mut_allows_writing_into_the_tile() {
    let pool = TilePool::new(2);
    let mut tile = pool.allocate();
    tile[0] = 1.0;
    tile[3] = 2.0;
    assert_eq!(&*tile, &[1.0, 0.0, 0.0, 2.0]);
}

#[test]
fn dropped_tiles_are_returned_to_the_pool_and_reused() {
    let pool = TilePool::new(2);
    {
        let mut tile = pool.allocate();
        tile[0] = 42.0;
        // `tile` drops here, returning its buffer to the pool's free list.
    }
    // A fresh allocation reuses the freed buffer rather than growing the pool, so the byte
    // footprint should not have doubled.
    let bytes_after_first_round_trip = pool.allocated_bytes();
    let reused = pool.allocate();
    assert_eq!(pool.allocated_bytes(), bytes_after_first_round_trip);
    // The reused buffer keeps whatever was written to it before - callers are expected to
    // overwrite it fully, not rely on it being zeroed again.
    assert_eq!(reused[0], 42.0);
}

#[test]
fn allocated_bytes_grows_only_when_the_pool_actually_allocates_new_memory() {
    let pool = TilePool::new(3);
    assert_eq!(pool.allocated_bytes(), 0);

    let a = pool.allocate();
    let expected_tile_bytes = 3 * 3 * std::mem::size_of::<f32>();
    assert_eq!(pool.allocated_bytes(), expected_tile_bytes);

    let b = pool.allocate();
    assert_eq!(pool.allocated_bytes(), 2 * expected_tile_bytes);

    drop(a);
    drop(b);
    // Freeing tiles doesn't shrink the pool's reserved footprint.
    assert_eq!(pool.allocated_bytes(), 2 * expected_tile_bytes);
}
