use std::sync::Arc;

use waterdrop_terrain_generator::core::{
    ChunkCoord, ComputeTarget, NParamValue, Node, NodeError, TileContext, TileHandle, TilePool
};
use waterdrop_terrain_generator::nodes::Combine;

/// `compute_target: Cpu` forces the CPU path regardless of `chunk` - no render world (and
/// therefore no GPU dispatcher) exists in a plain `#[test]`.
fn cpu_ctx() -> TileContext {
    TileContext {
        chunk: Some(ChunkCoord(0, 0)),
        world_origin: (0.0, 0.0),
        world_step: (1.0, 1.0),
        world_extent: (2.0, 2.0),
        compute_target: ComputeTarget::Cpu
    }
}

fn tile(pool: &Arc<TilePool>, values: &[f32]) -> TileHandle {
    let mut t = pool.allocate();
    t.copy_from_slice(values);
    Arc::new(t)
}

fn color_tile(pool: &Arc<TilePool>, r: &[f32], g: &[f32], b: &[f32]) -> TileHandle {
    let mut t = pool.allocate_channels(3);
    t.plane_mut(0).copy_from_slice(r);
    t.plane_mut(1).copy_from_slice(g);
    t.plane_mut(2).copy_from_slice(b);
    Arc::new(t)
}

fn set_method(node: &mut Combine, method: &str) {
    node.set_param("method", NParamValue::Enum(method.to_string()))
        .unwrap();
}

fn assert_close(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 1e-5, "{a} != {e}");
    }
}

#[test]
fn blend_defaults_to_an_even_average() {
    let pool = TilePool::new(2);
    let a = tile(&pool, &[1.0, 2.0, 3.0, 4.0]);
    let b = tile(&pool, &[5.0, 6.0, 7.0, 8.0]);
    let out = Combine::default()
        .process(&pool, &[a, b], &cpu_ctx())
        .unwrap();
    assert_close(&out[0], &[3.0, 4.0, 5.0, 6.0]);
}

#[test]
fn blend_factor_interpolates_between_a_and_b() {
    let pool = TilePool::new(1);
    let a = tile(&pool, &[0.0]);
    let b = tile(&pool, &[4.0]);
    let mut node = Combine::default();
    node.set_param("strength", NParamValue::Float(0.25))
        .unwrap();
    let out = node.process(&pool, &[a, b], &cpu_ctx()).unwrap();
    assert_close(&out[0], &[1.0]);
}

#[test]
fn add_sums_a_and_strength_scaled_b() {
    let pool = TilePool::new(2);
    let a = tile(&pool, &[1.0, 2.0, 3.0, 4.0]);
    let b = tile(&pool, &[5.0, 6.0, 7.0, 8.0]);
    let mut node = Combine::default();
    set_method(&mut node, "Add");
    let out = node.process(&pool, &[a, b], &cpu_ctx()).unwrap();
    assert_close(&out[0], &[6.0, 8.0, 10.0, 12.0]);
}

#[test]
fn subtract_removes_strength_scaled_b_from_a() {
    let pool = TilePool::new(2);
    let a = tile(&pool, &[2.0, 4.0, 6.0, 8.0]);
    let b = tile(&pool, &[1.0, 1.0, 1.0, 1.0]);
    let mut node = Combine::default();
    set_method(&mut node, "Subtract");
    let out = node.process(&pool, &[a, b], &cpu_ctx()).unwrap();
    assert_close(&out[0], &[1.0, 3.0, 5.0, 7.0]);
}

#[test]
fn multiply_at_full_amount_is_a_times_b() {
    let pool = TilePool::new(2);
    let a = tile(&pool, &[1.0, 2.0, 3.0, 4.0]);
    let b = tile(&pool, &[2.0, 2.0, 2.0, 2.0]);
    let mut node = Combine::default();
    set_method(&mut node, "Multiply");
    let out = node.process(&pool, &[a, b], &cpu_ctx()).unwrap();
    assert_close(&out[0], &[2.0, 4.0, 6.0, 8.0]);
}

#[test]
fn screen_matches_the_photographic_formula() {
    let pool = TilePool::new(2);
    let a = tile(&pool, &[0.0, 0.5, 1.0, 0.2]);
    let b = tile(&pool, &[0.0, 0.5, 0.0, 0.3]);
    let mut node = Combine::default();
    set_method(&mut node, "Screen");
    let out = node.process(&pool, &[a, b], &cpu_ctx()).unwrap();
    assert_close(&out[0], &[0.0, 0.75, 1.0, 0.44]);
}

#[test]
fn difference_is_the_absolute_gap() {
    let pool = TilePool::new(2);
    let a = tile(&pool, &[5.0, 1.0, 3.0, 10.0]);
    let b = tile(&pool, &[2.0, 4.0, 3.0, 7.0]);
    let mut node = Combine::default();
    set_method(&mut node, "Difference");
    let out = node.process(&pool, &[a, b], &cpu_ctx()).unwrap();
    assert_close(&out[0], &[3.0, 3.0, 0.0, 3.0]);
}

#[test]
fn combines_multi_channel_color_tiles_elementwise_per_channel() {
    let pool = TilePool::new(1);
    let a = color_tile(&pool, &[1.0], &[2.0], &[3.0]);
    let b = color_tile(&pool, &[5.0], &[6.0], &[7.0]);
    let mut node = Combine::default();
    set_method(&mut node, "Add");
    let out = node.process(&pool, &[a, b], &cpu_ctx()).unwrap();
    assert_close(out[0].plane(0), &[6.0]);
    assert_close(out[0].plane(1), &[8.0]);
    assert_close(out[0].plane(2), &[10.0]);
}

#[test]
fn switching_method_resets_strength_to_that_methods_own_default() {
    let mut node = Combine::default();
    assert_eq!(node.get_param("strength"), Some(NParamValue::Float(0.5)));

    set_method(&mut node, "Add");
    assert_eq!(node.get_param("strength"), Some(NParamValue::Float(1.0)));

    node.set_param("strength", NParamValue::Float(1.8)).unwrap();
    set_method(&mut node, "Blend");
    assert_eq!(node.get_param("strength"), Some(NParamValue::Float(0.5)));
}

#[test]
fn desc_params_reflects_the_selected_methods_own_strength_range() {
    let mut node = Combine::default();
    let blend_strength = node
        .desc_params()
        .iter()
        .find(|d| d.key == "strength")
        .unwrap();
    assert_eq!(blend_strength.label, "Blend Factor");

    set_method(&mut node, "Add");
    let add_strength = node
        .desc_params()
        .iter()
        .find(|d| d.key == "strength")
        .unwrap();
    assert_eq!(add_strength.label, "Strength");
}

#[test]
fn setting_an_unknown_method_fails() {
    let mut node = Combine::default();
    let err = node
        .set_param("method", NParamValue::Enum("Overlay".to_string()))
        .unwrap_err();
    assert!(matches!(err, NodeError::ProcessingFailed(_)));
}

#[test]
fn setting_an_unknown_param_key_fails() {
    let mut node = Combine::default();
    assert!(node.set_param("nope", NParamValue::Float(1.0)).is_err());
}
