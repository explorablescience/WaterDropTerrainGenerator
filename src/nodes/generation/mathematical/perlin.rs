use std::sync::{Arc, OnceLock};

use crate::core::gpu;
use crate::core::node::{
    NParamConstraints, NParamDesc, NParamValue, Node, NodeCategory, NodeDescriptor, NodeError,
    NodeIcon, NodePortType, NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-perlin",
    png_bytes: include_bytes!("../../../../assets/icons/node_perlin.png")
};

const SHADER: &str = include_str!("perlin.comp.wgsl");
const WORKGROUP_SIZE: u32 = 8;

/// Layout must match `perlin.comp.wgsl`'s `Params` struct exactly (vec4-aligned fields).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PerlinParams {
    origin_step: [f32; 4],
    amp_freq_hurst_warpamp: [f32; 4],
    warpfreq_seed: [f32; 4],
    counts: [u32; 4]
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
                        max: 4.0
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
                        max: 1.0
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
        let mut output = pool.allocate();
        let size = output.size() as u32;
        let params = PerlinParams {
            origin_step: [
                ctx.world_origin.0,
                ctx.world_origin.1,
                ctx.world_step.0,
                ctx.world_step.1,
            ],
            amp_freq_hurst_warpamp: [
                self.amplitude,
                self.frequency,
                self.hurst_exponent,
                self.warp_amplitude,
            ],
            warpfreq_seed: [self.warp_frequency, self.seed as f32, 0.0, 0.0],
            counts: [self.octaves, self.warp_octaves, size, 0]
        };
        let workgroups = size.div_ceil(WORKGROUP_SIZE);
        let result = gpu::dispatch_f32(
            "perlin",
            SHADER,
            &params,
            &[],
            (size * size) as usize,
            (workgroups, workgroups, 1)
        )?;
        output.copy_from_slice(&result);
        Ok(vec![Arc::new(output)])
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
