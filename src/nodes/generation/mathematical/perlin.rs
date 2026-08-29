use std::sync::{Arc, OnceLock};

use bevy::math::{Vec2, Vec3};
use rayon::prelude::*;

use crate::core::node::{
    NParamConstraints, NParamDesc, NParamValue, Node, NodeCategory, NodeDescriptor, NodeError,
    NodeIcon, NodePortType, NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-perlin",
    png_bytes: include_bytes!("../../../../assets/icons/node_perlin.png")
};

// Precision-adjusted variations of https://www.shadertoy.com/view/4djSRW
fn hash1(p: f32) -> f32 {
    let mut p = frac(p * 0.011);
    p *= p + 7.5;
    p *= p + p;
    frac(p)
}
fn hash2(p: Vec2) -> f32 {
    let mut p3 = frac3(Vec3::new(p.x, p.y, p.x) * 0.13);
    let d = p3.dot(Vec3::new(p3.y, p3.z, p3.x) + Vec3::splat(3.333));
    p3 += Vec3::splat(d);
    frac((p3.x + p3.y) * p3.z)
}
fn frac(x: f32) -> f32 {
    x - x.floor()
}
fn frac3(v: Vec3) -> Vec3 {
    v - v.floor()
}

// 2D Perlin noise function (https://www.shadertoy.com/view/4dS3Wd)
fn noise(pos: Vec2) -> f32 {
    let i = pos.floor();
    let f = pos - i;

    // Four corners in 2D of a tile
    let a = hash2(i);
    let b = hash2(i + Vec2::new(1.0, 0.0));
    let c = hash2(i + Vec2::new(0.0, 1.0));
    let d = hash2(i + Vec2::new(1.0, 1.0));

    // Simple 2D lerp using smoothstep envelope between the values
    let u = f * f * (Vec2::splat(3.0) - Vec2::splat(2.0) * f);
    mix(a, b, u.x) + (c - a) * u.y * (1.0 - u.x) + (d - b) * u.x * u.y
}

// Fractal Brownian Motion (fBm) using Perlin noise
// Rotate each octave to reduce axial bias
fn fbm(pos: Vec2, params: &Perlin, seed_offset: Vec2) -> f32 {
    let mut x = pos * params.frequency + seed_offset;
    let mut v = 0.0;
    let mut a = params.amplitude;
    let g = (-params.hurst_exponent).exp();
    let shift = Vec2::splat(100.0);
    let (s, c) = 0.5_f32.sin_cos();
    for _ in 0..params.octaves {
        v += a * noise(x);
        x = Vec2::new(c * x.x - s * x.y, s * x.x + c * x.y) * 2.0 + shift;
        a *= g;
    }
    v
}

// Generate fbm with warping effects (https://iquilezles.org/articles/warp/)
fn fbm_with_warp(pos: Vec2, params: &Perlin, seed_offset: Vec2) -> f32 {
    let mut offset = seed_offset + Vec2::splat(121484.0);
    for _ in 0..params.warp_octaves {
        let warp_pos = pos * params.warp_frequency + offset;

        // Using arbitrary offsets to decorrelate
        let q = Vec2::new(
            fbm(warp_pos, params, seed_offset),
            fbm(warp_pos + Vec2::new(5.2, 1.3), params, seed_offset)
        );
        offset = params.warp_amplitude * q;
    }
    fbm(pos + offset, params, seed_offset)
}

// Utility functions
fn seed_offset(seed: u32) -> Vec2 {
    Vec2::new(
        hash1(seed as f32) * 1000.0,
        hash1(seed as f32 + 91.7) * 1000.0
    )
}
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

#[derive(Debug, Clone, Copy)]
pub struct Perlin {
    pub amplitude: f32,
    pub seed: u32,

    pub frequency: f32,
    pub octaves: u32,
    pub hurst_exponent: f32,

    pub warp_amplitude: f32,
    pub warp_frequency: f32,
    pub warp_octaves: u32    
}
impl Default for Perlin {
    fn default() -> Self {
        Self {
            seed: 0,
            amplitude: 1.0,
            frequency: 0.05,
            octaves: 6,
            hurst_exponent: 0.7,
            warp_amplitude: 0.0,
            warp_frequency: 0.0,
            warp_octaves: 1
        }
    }
}
impl Perlin {
    fn params() -> &'static [NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = std::sync::OnceLock::new();
        SPECS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "amplitude",
                    label: "Scale",
                    category: "Noise",
                    default: NParamValue::Float(1.0),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: 0.0,
                        max: 10.0
                    })
                },
                NParamDesc {
                    key: "seed",
                    label: "Seed",
                    category: "Noise",
                    default: NParamValue::Int(0),
                    constraints: Some(NParamConstraints::IntRange { min: 0, max: 10000 })
                },
                NParamDesc {
                    key: "frequency",
                    label: "Frequency",
                    category: "Fractal Brownian Motion",
                    default: NParamValue::Float(0.05),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: 0.0,
                        max: 2.0
                    })
                },
                NParamDesc {
                    key: "octaves",
                    label: "Octaves",
                    category: "Fractal Brownian Motion",
                    default: NParamValue::Int(6),
                    constraints: Some(NParamConstraints::IntRange { min: 1, max: 10 })
                },
                NParamDesc {
                    key: "hurst_exponent",
                    label: "Hurst Exponent",
                    category: "Fractal Brownian Motion",
                    default: NParamValue::Float(0.7),
                    constraints: Some(NParamConstraints::FloatRange { min: 0.0, max: 1.0 })
                },

                NParamDesc {
                    key: "warp_amplitude",
                    label: "Warp Amplitude",
                    category: "Warping",
                    default: NParamValue::Float(0.0),
                    constraints: Some(NParamConstraints::FloatRange { min: 0.0, max: 10.0 })
                },
                NParamDesc {
                    key: "warp_frequency",
                    label: "Warp Frequency",
                    category: "Warping",
                    default: NParamValue::Float(0.0),
                    constraints: Some(NParamConstraints::FloatRange { min: 0.0, max: 10.0 })
                },
                NParamDesc {
                    key: "warp_octaves",
                    label: "Warp Octaves",
                    category: "Warping",
                    default: NParamValue::Int(1),
                    constraints: Some(NParamConstraints::IntRange { min: 1, max: 10 })
                }
            ]
        })
    }

    /// Samples at each texel's world-space position (not a `[0, 1]` local range), so adjacent chunks line up seamlessly at their shared border.
    fn process_tile(&self, pool: &Arc<TilePool>, ctx: &TileContext) -> TileHandle {
        let mut output = pool.allocate();
        let s = output.size();
        let offset = seed_offset(self.seed);
        output.par_chunks_mut(s).enumerate().for_each(|(y, row)| {
            for (x, texel) in row.iter_mut().enumerate() {
                let (nx, ny) = ctx.world_pos(x, y);
                if self.warp_amplitude > 0.0 {
                    *texel = fbm_with_warp([nx, ny].into(), self, offset);
                } else {
                    *texel = fbm([nx, ny].into(), self, offset);
                }
            }
        });
        Arc::new(output)
    }
}
impl Node for Perlin {
    fn label(&self) -> &str {
        "Perlin"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::Generation
    }
    fn icon(&self) -> NodeIcon {
        ICON
    }

    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: NodePortType::Height,
            required: true
        }]
    }

    fn desc_params(&self) -> &'static [NParamDesc] {
        Self::params()
    }
    fn get_param(&self, key: &str) -> Option<NParamValue> {
        match key {
            "amplitude" => Some(NParamValue::Float(self.amplitude)),
            "seed" => Some(NParamValue::Int(self.seed as i32)),
            "frequency" => Some(NParamValue::Float(self.frequency)),
            "octaves" => Some(NParamValue::Int(self.octaves as i32)),
            "hurst_exponent" => Some(NParamValue::Float(self.hurst_exponent)),
            "warp_amplitude" => Some(NParamValue::Float(self.warp_amplitude)),
            "warp_frequency" => Some(NParamValue::Float(self.warp_frequency)),
            "warp_octaves" => Some(NParamValue::Int(self.warp_octaves as i32)),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("amplitude", NParamValue::Float(v)) => self.amplitude = v,
            ("seed", NParamValue::Int(v)) => self.seed = v as u32,
            ("frequency", NParamValue::Float(v)) => self.frequency = v,
            ("octaves", NParamValue::Int(v)) => self.octaves = v as u32,
            ("hurst_exponent", NParamValue::Float(v)) => self.hurst_exponent = v,
            ("warp_amplitude", NParamValue::Float(v)) => self.warp_amplitude = v,
            ("warp_frequency", NParamValue::Float(v)) => self.warp_frequency = v,
            ("warp_octaves", NParamValue::Int(v)) => self.warp_octaves = v as u32,
            (k, v) => return Err(format!("Unknown parameter {} with value {:?}", k, v).into())
        }
        Ok(())
    }

    fn process(
        &self,
        pool: &Arc<TilePool>,
        _inputs: &[TileHandle],
        ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![self.process_tile(pool, ctx)])
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "Perlin",
        category: NodeCategory::Generation,
        subcategory: "Mathematical",
        icon: ICON,
        factory: || Box::new(Perlin::default())
    }
}
