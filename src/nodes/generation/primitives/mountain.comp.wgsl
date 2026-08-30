struct Params {
    height_radius_pos: vec4<f32>,
    origin_step: vec4<f32>,
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

    let height = params.height_radius_pos.x;
    let radius = params.height_radius_pos.y;
    let center = params.height_radius_pos.zw;
    let world_origin = params.origin_step.xy;
    let world_step = params.origin_step.zw;

    let world = world_origin + vec2<f32>(f32(gid.x), f32(gid.y)) * world_step;
    let dist = length(world - center);
    let t = clamp(1.0 - dist / radius, 0.0, 1.0);
    let falloff = t * t * (3.0 - 2.0 * t);

    output[gid.y * size + gid.x] = falloff * height;
}
