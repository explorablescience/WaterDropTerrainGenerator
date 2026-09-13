struct Params {
    strength: vec4<f32>,
    method: vec4<u32>, // [method_id, tile_size, channels, unused]
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<storage, read> input_a: array<f32>;
@group(0) @binding(3) var<storage, read> input_b: array<f32>;

// Must match `CombineMethod::gpu_id` in combine.rs exactly.
fn combine_val(a: f32, b: f32, method_id: u32, strength: f32) -> f32 {
    if method_id == 0u {
        return mix(a, b, strength); // Blend
    } else if method_id == 1u {
        return a + strength * b; // Add
    } else if method_id == 2u {
        return a - strength * b; // Subtract
    } else if method_id == 3u {
        return mix(a, a * b, strength); // Multiply
    } else if method_id == 4u {
        return mix(a, 1.0 - (1.0 - a) * (1.0 - b), strength); // Screen
    } else {
        return mix(a, abs(a - b), strength); // Difference
    }
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = params.method.y;
    if gid.x >= size || gid.y >= size {
        return;
    }

    let plane = size * size;
    let texel = gid.y * size + gid.x;
    for (var c = 0u; c < params.method.z; c = c + 1u) {
        let idx = c * plane + texel;
        output[idx] = combine_val(input_a[idx], input_b[idx], params.method.x, params.strength.x);
    }
}
