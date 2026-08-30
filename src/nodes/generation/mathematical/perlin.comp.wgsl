struct Params {
    origin_step: vec4<f32>,             // world_origin.x, world_origin.y, world_step.x, world_step.y
    amp_freq_hurst_warpamp: vec4<f32>,  // amplitude, frequency, hurst_exponent, warp_amplitude
    warpfreq_seed: vec4<f32>,           // warp_frequency, seed, unused, unused
    counts: vec4<u32>,                  // octaves, warp_octaves, tile_size, unused
}

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;

// Precision-adjusted variations of https://www.shadertoy.com/view/4djSRW
fn frac(x: f32) -> f32 {
    return x - floor(x);
}
fn frac3(v: vec3<f32>) -> vec3<f32> {
    return v - floor(v);
}
fn hash1(p_in: f32) -> f32 {
    var p = frac(p_in * 0.011);
    p = p * (p + 7.5);
    p = p * (p + p);
    return frac(p);
}
fn hash2(p_in: vec2<f32>) -> f32 {
    var p3 = frac3(vec3<f32>(p_in.x, p_in.y, p_in.x) * 0.13);
    let d = dot(p3, p3.yzx + vec3<f32>(3.333, 3.333, 3.333));
    p3 = p3 + vec3<f32>(d, d, d);
    return frac((p3.x + p3.y) * p3.z);
}

// 2D Perlin noise function (https://www.shadertoy.com/view/4dS3Wd)
fn noise2(pos: vec2<f32>) -> f32 {
    let i = floor(pos);
    let f = pos - i;

    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));

    let u = f * f * (vec2<f32>(3.0, 3.0) - vec2<f32>(2.0, 2.0) * f);
    return mix(a, b, u.x) + (c - a) * u.y * (1.0 - u.x) + (d - b) * u.x * u.y;
}

// Fractal Brownian Motion (fBm) using Perlin noise; rotates each octave to reduce axial bias.
fn fbm(pos: vec2<f32>, frequency: f32, amplitude: f32, hurst_exponent: f32, octaves: u32, seed_offset: vec2<f32>) -> f32 {
    var x = pos * frequency + seed_offset;
    var v = 0.0;
    var a = amplitude;
    let g = exp(-hurst_exponent);
    let shift = vec2<f32>(100.0, 100.0);
    let s = sin(0.5);
    let c = cos(0.5);
    for (var i: u32 = 0u; i < octaves; i = i + 1u) {
        v = v + a * noise2(x);
        x = vec2<f32>(c * x.x - s * x.y, s * x.x + c * x.y) * 2.0 + shift;
        a = a * g;
    }
    return v;
}

// fbm with warping effects (https://iquilezles.org/articles/warp/)
fn fbm_with_warp(
    pos: vec2<f32>, frequency: f32, amplitude: f32, hurst_exponent: f32, octaves: u32,
    warp_amplitude: f32, warp_frequency: f32, warp_octaves: u32, seed_offset: vec2<f32>
) -> f32 {
    var offset = seed_offset + vec2<f32>(121484.0, 121484.0);
    for (var i: u32 = 0u; i < warp_octaves; i = i + 1u) {
        let warp_pos = pos * warp_frequency + offset;
        let q = vec2<f32>(
            fbm(warp_pos, frequency, amplitude, hurst_exponent, octaves, seed_offset),
            fbm(warp_pos + vec2<f32>(5.2, 1.3), frequency, amplitude, hurst_exponent, octaves, seed_offset)
        );
        offset = warp_amplitude * q;
    }
    return fbm(pos + offset, frequency, amplitude, hurst_exponent, octaves, seed_offset);
}

fn seed_offset_of(seed: f32) -> vec2<f32> {
    return vec2<f32>(hash1(seed) * 1000.0, hash1(seed + 91.7) * 1000.0);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = params.counts.z;
    if gid.x >= size || gid.y >= size {
        return;
    }

    let world_origin = params.origin_step.xy;
    let world_step = params.origin_step.zw;
    let amplitude = params.amp_freq_hurst_warpamp.x;
    let frequency = params.amp_freq_hurst_warpamp.y;
    let hurst_exponent = params.amp_freq_hurst_warpamp.z;
    let warp_amplitude = params.amp_freq_hurst_warpamp.w;
    let warp_frequency = params.warpfreq_seed.x;
    let seed = params.warpfreq_seed.y;
    let octaves = params.counts.x;
    let warp_octaves = params.counts.y;

    let world = world_origin + vec2<f32>(f32(gid.x), f32(gid.y)) * world_step;
    let offset = seed_offset_of(seed);

    var value: f32;
    if warp_amplitude > 0.0 {
        value = fbm_with_warp(
            world, frequency, amplitude, hurst_exponent, octaves,
            warp_amplitude, warp_frequency, warp_octaves, offset
        );
    } else {
        value = fbm(world, frequency, amplitude, hurst_exponent, octaves, offset);
    }

    output[gid.y * size + gid.x] = value;
}
