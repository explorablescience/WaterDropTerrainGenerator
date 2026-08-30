struct Params {
    tile_size: vec4<u32>,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<storage, read> input_a: array<f32>;
@group(0) @binding(3) var<storage, read> input_b: array<f32>;

// NaN is the only non-equal-to-itself float value in IEEE 754, so this doubles as a NaN check.
fn is_finite(x: f32) -> bool {
    return x == x;
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = params.tile_size.x;
    if gid.x >= size || gid.y >= size {
        return;
    }
    let idx = gid.y * size + gid.x;

    var sum = 0.0;
    var count = 0.0;
    let a = input_a[idx];
    let b = input_b[idx];
    if is_finite(a) {
        sum = sum + a;
        count = count + 1.0;
    }
    if is_finite(b) {
        sum = sum + b;
        count = count + 1.0;
    }

    if count > 0.0 {
        output[idx] = sum / count;
    } else {
        output[idx] = bitcast<f32>(0x7fc00000u); // quiet NaN
    }
}
