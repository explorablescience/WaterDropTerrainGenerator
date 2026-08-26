#include "core/render/pbr/pbr_functions.wgsl"

// Vertex shader for procedurally generated terrain-preview chunk meshes: unlike the main gbuffer
// pass, vertex data comes from a real bound vertex buffer dedicated to each mesh (not the shared
// bindless SSBO mesh arena, which has too little capacity for a large, dynamically regenerated
// multi-chunk terrain), and positions/normals are already in world space - each chunk's own world
// offset is baked into its vertex data on the CPU side, so no per-instance model matrix is needed.

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv:       vec2<f32>,
    @location(2) normal:   vec3<f32>,
    @location(3) tangent:  vec4<f32>
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

@vertex
fn main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let view_pos4 = in_camera.world_to_view * vec4<f32>(in.position, 1.0);
    out.clip_position = in_camera.view_to_ndc * view_pos4;
    out.ndc_z = -view_pos4.z;

    out.tex_coord = in.uv;
    out.normal_world = normalize(in.normal);
    out.tangent_world = in.tangent;
    out.bitangent_world = cross(in.normal, in.tangent.xyz) * in.tangent.w;

    return out;
}
