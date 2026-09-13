use std::sync::Arc;

use waterdrop_terrain_generator::core::{
    ChunkCoord, ComputeTarget, NParamValue, Node, NodeError, TileContext, TileHandle, TilePool
};
use waterdrop_terrain_generator::nodes::SatMap;

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

fn set_preset(node: &mut SatMap, preset: &str) {
    node.set_param("preset", NParamValue::Enum(preset.to_string()))
        .unwrap();
}

fn assert_close(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 1e-5, "{a} != {e}");
    }
}

#[test]
fn grayscale_maps_height_linearly_to_all_three_channels() {
    let pool = TilePool::new(2);
    let height = tile(&pool, &[0.0, 0.25, 0.75, 1.0]);
    let out = SatMap::default()
        .process(&pool, &[height], &cpu_ctx())
        .unwrap();
    assert_close(out[0].plane(0), &[0.0, 0.25, 0.75, 1.0]);
    assert_close(out[0].plane(1), &[0.0, 0.25, 0.75, 1.0]);
    assert_close(out[0].plane(2), &[0.0, 0.25, 0.75, 1.0]);
}

#[test]
fn height_outside_the_min_max_range_clamps_to_the_gradient_ends() {
    let pool = TilePool::new(2);
    let height = tile(&pool, &[-5.0, 0.0, 1.0, 50.0]);
    let out = SatMap::default()
        .process(&pool, &[height], &cpu_ctx())
        .unwrap();
    assert_close(out[0].plane(0), &[0.0, 0.0, 1.0, 1.0]);
}

#[test]
fn min_and_max_height_remap_the_input_range_before_sampling() {
    let pool = TilePool::new(1);
    let height = tile(&pool, &[5.0]);
    let mut node = SatMap::default();
    node.set_param("min_height", NParamValue::Float(0.0))
        .unwrap();
    node.set_param("max_height", NParamValue::Float(10.0))
        .unwrap();
    let out = node.process(&pool, &[height], &cpu_ctx()).unwrap();
    assert_close(out[0].plane(0), &[0.5]);
    assert_close(out[0].plane(1), &[0.5]);
    assert_close(out[0].plane(2), &[0.5]);
}

#[test]
fn switching_preset_changes_the_sampled_color() {
    let pool = TilePool::new(1);
    let height = tile(&pool, &[0.0]);
    let mut node = SatMap::default();
    set_preset(&mut node, "Volcanic");
    let out = node.process(&pool, &[height], &cpu_ctx()).unwrap();
    assert_close(out[0].plane(0), &[0.02]);
    assert_close(out[0].plane(1), &[0.02]);
    assert_close(out[0].plane(2), &[0.02]);
}

#[test]
fn setting_an_unknown_preset_fails() {
    let mut node = SatMap::default();
    let err = node
        .set_param("preset", NParamValue::Enum("Nonexistent".to_string()))
        .unwrap_err();
    assert!(matches!(err, NodeError::ProcessingFailed(_)));
}

#[test]
fn setting_an_unknown_param_key_fails() {
    let mut node = SatMap::default();
    assert!(node.set_param("nope", NParamValue::Float(1.0)).is_err());
}
