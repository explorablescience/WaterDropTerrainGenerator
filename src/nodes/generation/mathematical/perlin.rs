use std::sync::{Arc, OnceLock};

use crate::core::node::{
    NParamConstraints, NParamDesc, NParamValue, Node, NodeCategory, NodeDescriptor, NodeError,
    NodeIcon, NodePortType, NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-perlin",
    png_bytes: include_bytes!("../../../../assets/icons/node_perlin.png")
};

/// Period (in lattice units) the hashed noise lattice wraps around after, so the noise repeats on a fixed, predictable period rather than an incidental one.
const NOISE_PERIOD: i32 = 1024;

/// Cheap integer hash of a lattice point, wrapped to [`NOISE_PERIOD`], mapped to `[-1, 1]`.
fn hash(ix: i32, iy: i32) -> f32 {
    let px = ix.rem_euclid(NOISE_PERIOD) as u32;
    let py = iy.rem_euclid(NOISE_PERIOD) as u32;
    let mut h = px.wrapping_mul(374761393) ^ py.wrapping_mul(668265263);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// This is value noise, not true gradient-based Perlin noise, but it's periodic (see [`NOISE_PERIOD`]) and cheap.
fn value_noise(x: f32, y: f32) -> f32 {
    let x0 = x.floor();
    let y0 = y.floor();
    let (ix0, iy0) = (x0 as i32, y0 as i32);
    let (fx, fy) = (x - x0, y - y0);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));

    let n00 = hash(ix0, iy0);
    let n10 = hash(ix0 + 1, iy0);
    let n01 = hash(ix0, iy0 + 1);
    let n11 = hash(ix0 + 1, iy0 + 1);

    let nx0 = n00 + sx * (n10 - n00);
    let nx1 = n01 + sx * (n11 - n01);
    nx0 + sy * (nx1 - nx0)
}

#[derive(Debug)]
pub struct Perlin {
    pub frequency: f32,
    pub amplitude: f32,
    pub octaves: u32
}
impl Default for Perlin {
    fn default() -> Self {
        Self {
            frequency: 1.0,
            amplitude: 1.0,
            octaves: 4
        }
    }
}
impl Perlin {
    fn params() -> &'static [NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = std::sync::OnceLock::new();
        SPECS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "frequency",
                    label: "Frequency",
                    category: "Noise",
                    default: NParamValue::Float(1.0),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: 0.0,
                        max: 10.0
                    })
                },
                NParamDesc {
                    key: "amplitude",
                    label: "Amplitude",
                    category: "Noise",
                    default: NParamValue::Float(1.0),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: 0.0,
                        max: 10.0
                    })
                },
                NParamDesc {
                    key: "octaves",
                    label: "Octaves",
                    category: "Noise",
                    default: NParamValue::Int(4),
                    constraints: Some(NParamConstraints::IntRange { min: 1, max: 10 })
                },
            ]
        })
    }

    /// Samples at each texel's world-space position (not a `[0, 1]` local range), so adjacent chunks line up seamlessly at their shared border.
    fn process_tile(&self, pool: &Arc<TilePool>, ctx: &TileContext) -> TileHandle {
        let mut output = pool.allocate();
        let s = output.size();
        for y in 0..s {
            for x in 0..s {
                let (nx, ny) = ctx.world_pos(x, y);
                let mut noise_value = 0.0;
                let mut frequency = self.frequency;
                let mut amplitude = self.amplitude;
                for _ in 0..self.octaves {
                    noise_value += value_noise(nx * frequency, ny * frequency) * amplitude;
                    frequency *= 2.0;
                    amplitude *= 0.5;
                }
                output[y * s + x] = noise_value;
            }
        }
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
            "frequency" => Some(NParamValue::Float(self.frequency)),
            "amplitude" => Some(NParamValue::Float(self.amplitude)),
            "octaves" => Some(NParamValue::Int(self.octaves as i32)),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("frequency", NParamValue::Float(v)) => self.frequency = v,
            ("amplitude", NParamValue::Float(v)) => self.amplitude = v,
            ("octaves", NParamValue::Int(v)) => self.octaves = v as u32,
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
        icon: ICON,
        factory: || Box::new(Perlin::default())
    }
}
