use std::sync::{Arc, OnceLock};

use crate::core::node::{
    NParamConstraints, NParamDesc, NParamValue, Node, NodeCategory, NodeDescriptor, NodeError,
    NodeIcon, NodeLocality, NodePortType, NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-hydraulic-erosion",
    // No dedicated icon asset yet; reusing `Erosion`'s since both are erosion nodes.
    png_bytes: include_bytes!("../../../assets/icons/node_erosion.png")
};

/// Simple hydraulic erosion simulation
#[derive(Debug)]
pub struct HydraulicErosion {
    pub droplets: u32,
    pub erosion_rate: f32,
    pub deposit_rate: f32,
    pub seed: u32,
    pub native_resolution: u32
}
impl Default for HydraulicErosion {
    fn default() -> Self {
        Self {
            droplets: 20_000,
            erosion_rate: 0.3,
            deposit_rate: 0.3,
            seed: 1,
            native_resolution: 256
        }
    }
}
impl HydraulicErosion {
    fn params() -> &'static [NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        SPECS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "droplets",
                    label: "Droplets",
                    category: "Simulation",
                    default: NParamValue::Int(20_000),
                    constraints: Some(NParamConstraints::IntRange {
                        min: 0,
                        max: 200_000
                    })
                },
                NParamDesc {
                    key: "erosion_rate",
                    label: "Erosion Rate",
                    category: "Simulation",
                    default: NParamValue::Float(0.3),
                    constraints: Some(NParamConstraints::FloatRange { min: 0.0, max: 1.0 })
                },
                NParamDesc {
                    key: "deposit_rate",
                    label: "Deposit Rate",
                    category: "Simulation",
                    default: NParamValue::Float(0.3),
                    constraints: Some(NParamConstraints::FloatRange { min: 0.0, max: 1.0 })
                },
                NParamDesc {
                    key: "seed",
                    label: "Seed",
                    category: "Simulation",
                    default: NParamValue::Int(1),
                    constraints: Some(NParamConstraints::IntRange {
                        min: 0,
                        max: i32::MAX
                    })
                },
                NParamDesc {
                    key: "native_resolution",
                    label: "Native Resolution",
                    category: "Shape",
                    default: NParamValue::Int(256),
                    constraints: Some(NParamConstraints::IntRange { min: 16, max: 1024 })
                },
            ]
        })
    }

    /// Deterministic splitmix64 step, so re-running with the same `seed` reproduces the same droplets.
    fn next_unit(state: &mut u64) -> f32 {
        *state = state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^= z >> 31;
        (z >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Walks one droplet downhill via steepest descent, eroding as it goes, until it reaches a
    /// local minimum or the domain edge. Serial and mutates `heights` in place, since concurrent
    /// droplets would race on shared cells - this is a demo, not a throughput-tuned pass.
    fn simulate_droplet(&self, heights: &mut [f32], s: usize, rng_state: &mut u64) {
        let mut ix = (Self::next_unit(rng_state) * (s - 1) as f32) as usize;
        let mut iy = (Self::next_unit(rng_state) * (s - 1) as f32) as usize;
        let mut sediment = 0.0f32;

        loop {
            let here = heights[iy * s + ix];
            let mut step = None;
            for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (ix as i32 + dx, iy as i32 + dy);
                if nx < 0 || ny < 0 || nx as usize >= s || ny as usize >= s {
                    continue; // domain edge: only a whole-terrain pass even knows where this is
                }
                let nh = heights[ny as usize * s + nx as usize];
                if step.is_none_or(|(_, _, best)| nh < best) {
                    step = Some((nx as usize, ny as usize, nh));
                }
            }

            let Some((nix, niy, next_h)) = step else {
                break;
            };
            if next_h >= here {
                heights[iy * s + ix] += sediment; // local minimum: drop the rest here
                break;
            }

            let carved = self.erosion_rate * (here - next_h);
            heights[iy * s + ix] -= carved;
            sediment += carved;

            let deposited = sediment * self.deposit_rate;
            sediment -= deposited;
            heights[niy * s + nix] += deposited;

            ix = nix;
            iy = niy;
        }
    }
}
impl Node for HydraulicErosion {
    fn label(&self) -> &str {
        "Hydraulic Erosion (Demo)"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::Simulation
    }
    fn icon(&self) -> NodeIcon {
        ICON
    }

    fn locality(&self) -> NodeLocality {
        NodeLocality::Global {
            native_resolution: self.native_resolution as usize
        }
    }
    fn inputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: NodePortType::Height,
            required: true
        }]
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
            "droplets" => Some(NParamValue::Int(self.droplets as i32)),
            "erosion_rate" => Some(NParamValue::Float(self.erosion_rate)),
            "deposit_rate" => Some(NParamValue::Float(self.deposit_rate)),
            "seed" => Some(NParamValue::Int(self.seed as i32)),
            "native_resolution" => Some(NParamValue::Int(self.native_resolution as i32)),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("droplets", NParamValue::Int(v)) => self.droplets = v as u32,
            ("erosion_rate", NParamValue::Float(v)) => self.erosion_rate = v,
            ("deposit_rate", NParamValue::Float(v)) => self.deposit_rate = v,
            ("seed", NParamValue::Int(v)) => self.seed = v as u32,
            ("native_resolution", NParamValue::Int(v)) => self.native_resolution = v as u32,
            (k, v) => return Err(format!("Unknown parameter {} with value {:?}", k, v).into())
        }
        Ok(())
    }

    fn process(
        &self,
        pool: &Arc<TilePool>,
        inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        let input = &inputs[0];
        let s = input.size();
        let mut heights: Vec<f32> = input.to_vec();

        let mut rng_state = self.seed as u64 ^ 0x9E3779B97F4A7C15;
        for _ in 0..self.droplets {
            self.simulate_droplet(&mut heights, s, &mut rng_state);
        }

        let mut output = pool.allocate();
        output.copy_from_slice(&heights);
        Ok(vec![Arc::new(output)])
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "Hydraulic Erosion (Demo)",
        category: NodeCategory::Simulation,
        subcategory: "Hydrology",
        icon: ICON,
        factory: || Box::new(HydraulicErosion::default())
    }
}
