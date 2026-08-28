#include "core/render/pbr/pbr_functions.wgsl"

// Vertex shader for the terrain-preview: one shared, flat, low-cost grid mesh is drawn once per
// chunk via GPU instancing (@builtin(instance_index)). Each instance's world offset/cell size and
// heightmap-array layer come from `in_chunks`, and height/normal are displaced by reading that
// chunk's layer of `in_heightmap` directly with `textureLoad` - mesh vertices land exactly on
// texel centers (see build_shared_chunk_mesh), so no filtering/sampler is needed.

struct VertexInput {
    @location(0) position: vec3<f32>, // Local, unscaled: (x, 0, z) for integer x,z in [0, size]
    @location(1) uv:       vec2<f32>, // Unused (kept for vertex-layout parity)
    @location(2) normal:   vec3<f32>, // Unused - normal is recomputed below
    @location(3) tangent:  vec4<f32>  // Unused - tangent is recomputed below
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,  // Clip space position (after projection)
    @location(0) tex_coord:     vec2<f32>,        // Texture coordinates (UV)
    @location(1) normal_world:  vec3<f32>,        // Normal in world space
    @location(2) tangent_world: vec4<f32>,        // Tangent in world space
    @location(3) bitangent_world: vec3<f32>,      // Bitangent in world space
    @location(4) ndc_z: f32,                      // Linear view-space depth (-view_z, always positive)
};

@group(0) @binding(0) var<uniform> in_camera: Camera;

struct ChunkInstance {
    world_offset: vec2<f32>,
    cell_size:    f32,
    layer:        u32
};
@group(2) @binding(0) var<storage, read> in_chunks: array<ChunkInstance>;

@group(3) @binding(0) var in_heightmap: texture_2d_array<f32>;

@vertex
fn main(@builtin(instance_index) instance: u32, in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let chunk = in_chunks[instance];
    let layer = i32(chunk.layer);

    // `in_heightmap`'s layers are padded 1 texel per side, so (x, z) and its normal-sample
    // neighbors always land in-bounds at +1 (mirrors the CPU-side `padded_heightmap`).
    let texel = vec2<i32>(i32(in.position.x) + 1, i32(in.position.z) + 1);
    let h_c = textureLoad(in_heightmap, texel, layer, 0).r;
    let h_l = textureLoad(in_heightmap, texel + vec2<i32>(-1, 0), layer, 0).r;
    let h_r = textureLoad(in_heightmap, texel + vec2<i32>( 1, 0), layer, 0).r;
    let h_d = textureLoad(in_heightmap, texel + vec2<i32>(0, -1), layer, 0).r;
    let h_u = textureLoad(in_heightmap, texel + vec2<i32>(0,  1), layer, 0).r;

    let world_pos = vec3<f32>(
        chunk.world_offset.x + in.position.x * chunk.cell_size,
        h_c,
        chunk.world_offset.y + in.position.z * chunk.cell_size
    );

    let view_pos4 = in_camera.world_to_view * vec4<f32>(world_pos, 1.0);
    out.clip_position = in_camera.view_to_ndc * view_pos4;
    out.ndc_z = -view_pos4.z;

    out.tex_coord = in.uv;

    // Central-difference slope -> smooth per-vertex normal (same formula as the old CPU bake).
    let dx = h_r - h_l;
    let dz = h_u - h_d;
    out.normal_world = normalize(vec3<f32>(-dx, 2.0 * chunk.cell_size, -dz));
    out.tangent_world = vec4<f32>(1.0, 0.0, 0.0, 1.0);
    out.bitangent_world = cross(out.normal_world, out.tangent_world.xyz) * out.tangent_world.w;

    return out;
}
