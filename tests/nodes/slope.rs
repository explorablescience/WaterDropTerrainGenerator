use std::sync::Arc;

use waterdrop_terrain_generator::core::{
    ChunkCoord, ComputeTarget, Node, TileContext, TileHandle, TilePool
};
use waterdrop_terrain_generator::nodes::Slope;

fn cpu_ctx() -> TileContext {
    TileContext {
        chunk: Some(ChunkCoord(0, 0)),
        world_origin: (0.0, 0.0),
        world_step: (1.0, 1.0),
        world_extent: (3.0, 3.0),
        terrain_height: 1.0,
        compute_target: ComputeTarget::Cpu
    }
}

fn tile(pool: &Arc<TilePool>, values: &[f32]) -> TileHandle {
    let mut t = pool.allocate();
    t.copy_from_slice(values);
    Arc::new(t)
}

fn assert_close(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 1e-4, "{a} != {e}");
    }
}

#[test]
fn flat_terrain_has_zero_slope() {
    let pool = TilePool::new(3);
    let height = tile(&pool, &[1.0; 9]);
    let out = Slope.process(&pool, &[height], &cpu_ctx()).unwrap();
    assert_close(&out[0], &[0.0; 9]);
}

#[test]
fn linear_ramp_has_uniform_slope() {
    let pool = TilePool::new(3);
    let height = tile(&pool, &[0.0, 1.0, 2.0, 0.0, 1.0, 2.0, 0.0, 1.0, 2.0]);
    let out = Slope.process(&pool, &[height], &cpu_ctx()).unwrap();
    // A uniform dh/dx of 1.0 (dh/dz = 0) is a 45-degree incline, half of the normalized [0, 1] range.
    assert_close(&out[0], &[0.5; 9]);
}

#[test]
fn declares_a_one_texel_kernel_radius() {
    assert_eq!(Slope.size(), 1);
}
