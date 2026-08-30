struct Params {
    tile_size: vec4<u32>,
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = params.tile_size.x;
    if gid.x >= size || gid.y >= size {
        return;
    }

    output[gid.y * size + gid.x] = 0.0;
}
