#include "core/render/pbr/pbr_functions.wgsl"

// Fragment shader for terrain-preview chunk meshes - identical PBR/G-buffer output to
// core/render/pbr/gbuffer.frag.wgsl, just reusing the material bind group at group(1) instead of
// group(3) since terrain_preview.vert.wgsl's pipeline only has two bind groups (camera, material).

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord:     vec2<f32>,
    @location(1) normal_world:  vec3<f32>,
    @location(2) tangent_world: vec4<f32>,
    @location(3) bitangent_world: vec3<f32>,
    @location(4) ndc_z: f32,
};

struct Material3dUniform {
    flags: vec4<f32>,    // Flags indicating material textures (1.0 = present, 0.0 = absent) - albedo, metallic-roughness, normal, occlusion
    albedo: vec4<f32>,   // Albedo color of the material (r, g, b)
    metallic: f32,       // Metallic intensity of the material
    roughness: f32,      // Roughness intensity of the material
    _padding: vec2<f32>, // Unused padding to align to 16 bytes
};
@group(1) @binding(0) var<uniform> in_pbr_material: Material3dUniform;
@group(1) @binding(1) var in_albedo_texture: texture_2d<f32>;
@group(1) @binding(2) var in_albedo_sampler: sampler;
@group(1) @binding(3) var in_metallic_roughness_texture: texture_2d<f32>;
@group(1) @binding(4) var in_metallic_roughness_sampler: sampler;
@group(1) @binding(5) var in_normal_texture: texture_2d<f32>;
@group(1) @binding(6) var in_normal_sampler: sampler;
@group(1) @binding(7) var in_occlusion_texture: texture_2d<f32>;
@group(1) @binding(8) var in_occlusion_sampler: sampler;

@fragment
fn main(in: VertexOutput) -> FragOutput {
    var out: FragOutput;

    out.depth = write_depth(in.ndc_z);

    var albedo_color: vec3<f32> = in_pbr_material.albedo.rgb;
    var metallic_value: f32 = in_pbr_material.metallic;
    if (in_pbr_material.flags.x == 1.0) {
        albedo_color = textureSample(in_albedo_texture, in_albedo_sampler, in.tex_coord).rgb;
    }
    if (in_pbr_material.flags.y == 1.0) {
        metallic_value = textureSample(in_metallic_roughness_texture, in_metallic_roughness_sampler, in.tex_coord).b;
    }
    out.albedo_metallic = vec4<f32>(albedo_color, metallic_value);

    var normal_world: vec3<f32> = normalize(in.normal_world);
    if (in_pbr_material.flags.z == 1.0) {
        let normal_sample = textureSample(in_normal_texture, in_normal_sampler, in.tex_coord).rgb;
        normal_world = apply_normal_map(normal_sample, in.tangent_world.xyz, in.bitangent_world, normal_world);
    }

    var roughness_value: f32 = in_pbr_material.roughness;
    if (in_pbr_material.flags.y == 1.0) {
        roughness_value = textureSample(in_metallic_roughness_texture, in_metallic_roughness_sampler, in.tex_coord).g;
    }
    out.normal_roughness = vec4<f32>(normal_world, roughness_value);

    var ao: f32 = 1.0;
    if (in_pbr_material.flags.w == 1.0) {
        ao = textureSample(in_occlusion_texture, in_occlusion_sampler, in.tex_coord).r;
    }
    out.ao = ao;

    return out;
}
