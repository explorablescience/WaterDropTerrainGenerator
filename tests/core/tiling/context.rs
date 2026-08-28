use waterdrop_terrain_generator::core::tiling::TileContext;

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
        assert!(
            (tx - x as f32).abs() < 1e-5,
            "expected texel x {}, got {}",
            x,
            tx
        );
        assert!(
            (ty - y as f32).abs() < 1e-5,
            "expected texel y {}, got {}",
            y,
            ty
        );
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

    let expected_local = (
        (world.0 - position.0) / scale,
        (world.1 - position.1) / scale
    );
    let expected_sx = (expected_local.0 + 0.5) * 8.0;
    let expected_sy = (expected_local.1 + 0.5) * 8.0;
    assert!((sx - expected_sx).abs() < 1e-5);
    assert!((sy - expected_sy).abs() < 1e-5);
}
