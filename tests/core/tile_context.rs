use waterdrop_terrain_generator::core::tile_context::TileContext;

#[test]
fn local_pos_is_an_alias_of_world_pos() {
    let ctx = TileContext::for_global(16);
    assert_eq!(ctx.local_pos(5, 9), ctx.world_pos(5, 9));
}

#[test]
fn to_texel_is_the_inverse_of_world_pos() {
    let ctx = TileContext::for_global(16);
    for (x, y) in [(0, 0), (3, 7), (15, 15)] {
        let pos = ctx.world_pos(x, y);
        let (tx, ty) = ctx.to_texel(pos);
        assert!((tx - x as f32).abs() < 1e-5, "expected texel x {}, got {}", x, tx);
        assert!((ty - y as f32).abs() < 1e-5, "expected texel y {}, got {}", y, ty);
    }
}

#[test]
fn to_local_maps_a_placements_own_origin_back_to_zero() {
    // A world position exactly at a placement's `position` should map to local (0, 0), regardless
    // of `scale`.
    let local = TileContext::to_local((3.0, -2.0), (3.0, -2.0), 5.0);
    assert_eq!(local, (0.0, 0.0));
}

#[test]
fn to_local_divides_by_scale() {
    let local = TileContext::to_local((4.0, 4.0), (0.0, 0.0), 2.0);
    assert_eq!(local, (2.0, 2.0));
}

#[test]
fn to_local_and_to_texel_compose_into_the_manual_integration_formula() {
    // The pipeline an integration node runs: world -> a global input's local space -> that
    // input's own fractional texel coordinates - matches doing the arithmetic by hand.
    let input_ctx = TileContext::for_global(8);
    let world = (10.0, -4.0);
    let (position, scale) = ((2.0, 0.0), 4.0);

    let local = TileContext::to_local(world, position, scale);
    let (sx, sy) = input_ctx.to_texel(local);

    let expected_local = ((world.0 - position.0) / scale, (world.1 - position.1) / scale);
    let expected_sx = (expected_local.0 + 0.5) * 8.0;
    let expected_sy = (expected_local.1 + 0.5) * 8.0;
    assert!((sx - expected_sx).abs() < 1e-5);
    assert!((sy - expected_sy).abs() < 1e-5);
}

#[test]
fn normalize_maps_the_extents_edges_to_zero_and_one() {
    let ctx = TileContext::for_global(4);
    assert_eq!(ctx.normalize((-0.5, -0.5)), (0.0, 0.0));
    assert_eq!(ctx.normalize((0.5, 0.5)), (1.0, 1.0));
    assert_eq!(ctx.normalize((0.0, 0.0)), (0.5, 0.5));
}
