use std::sync::{Arc, OnceLock};

use crate::core::node::{Node, NodeError, NodePortType, NodeSocket};
use crate::core::node_parameters::{NParamConstraints, NParamDesc, NParamValue};
use crate::core::tile_allocator::{TileHandle, TilePool};

/// A node that generates a perlin noise terrain tile.
#[derive(Debug)]
pub struct NodeGeneratorPerlin {
    pub frequency: f32,
    pub amplitude: f32,
    pub octaves: u32
}
impl Default for NodeGeneratorPerlin {
    fn default() -> Self {
        Self {
            frequency: 1.0,
            amplitude: 1.0,
            octaves: 4
        }
    }
}
impl NodeGeneratorPerlin {
    fn params() -> &'static [crate::core::node_parameters::NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = std::sync::OnceLock::new();
        SPECS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "frequency",
                    label: "Frequency",
                    default: NParamValue::Float(1.0),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: 0.0,
                        max: 10.0
                    })
                },
                NParamDesc {
                    key: "amplitude",
                    label: "Amplitude",
                    default: NParamValue::Float(1.0),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: 0.0,
                        max: 10.0
                    })
                },
                NParamDesc {
                    key: "octaves",
                    label: "Octaves",
                    default: NParamValue::Int(4),
                    constraints: Some(NParamConstraints::IntRange { min: 1, max: 10 })
                },
            ]
        })
    }

    fn process_tile(&self, pool: &Arc<TilePool>) -> TileHandle {
        let mut output = pool.allocate();
        let s = output.size();
        for y in 0..s {
            for x in 0..s {
                let mut noise_value = 0.0;
                let mut frequency = self.frequency;
                let mut amplitude = self.amplitude;
                for _ in 0..self.octaves {
                    let nx = x as f32 / s as f32;
                    let ny = y as f32 / s as f32;
                    noise_value += self.perlin_noise(nx * frequency, ny * frequency) * amplitude;
                    frequency *= 2.0;
                    amplitude *= 0.5;
                }
                output[y * s + x] = noise_value;
            }
        }
        Arc::new(output)
    }

    fn perlin_noise(&self, x: f32, y: f32) -> f32 {
        // For now, simple sin
        (x * 2.0 * std::f32::consts::PI).sin()
            * (y * 2.0 * std::f32::consts::PI).sin()
            * self.amplitude
    }
}
impl Node for NodeGeneratorPerlin {
    fn label(&self) -> &str {
        "Generator Perlin Noise"
    }

    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "height",
            dtype: NodePortType::Height
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
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), String> {
        match (key, value) {
            ("frequency", NParamValue::Float(v)) => self.frequency = v,
            ("amplitude", NParamValue::Float(v)) => self.amplitude = v,
            ("octaves", NParamValue::Int(v)) => self.octaves = v as u32,
            (k, v) => return Err(format!("Unknown parameter {} with value {:?}", k, v))
        }
        Ok(())
    }

    fn process(
        &self,
        pool: &Arc<TilePool>,
        _inputs: &[TileHandle]
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![self.process_tile(pool)])
    }
}
