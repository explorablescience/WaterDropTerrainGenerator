use std::sync::{Arc, OnceLock};

use crate::core::node::{Node, NodePortType, NodeSocket};
use crate::core::node_error::NodeError;
use crate::core::node_parameters::{NParamConstraints, NParamDesc, NParamValue};
use crate::core::tile_allocator::{TileHandle, TilePool};

/// A minimal thermal-erosion node
#[derive(Debug)]
pub struct NodeErosion {
    /// How strongly each texel is pulled towards its neighbours' average height, in `[0, 1]`.
    strength: f32
}
impl Default for NodeErosion {
    fn default() -> Self {
        Self { strength: 0.5 }
    }
}
impl NodeErosion {
    fn params() -> &'static [NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        SPECS.get_or_init(|| {
            vec![NParamDesc {
                key: "strength",
                label: "Strength",
                default: NParamValue::Float(0.5),
                constraints: Some(NParamConstraints::FloatRange { min: 0.0, max: 1.0 })
            }]
        })
    }

    fn process_tile(&self, pool: &Arc<TilePool>, input: &TileHandle) -> TileHandle {
        let mut output = pool.allocate();
        let s = output.size();
        for y in 0..s {
            for x in 0..s {
                let mut sum = 0.0;
                let mut count = 0;
                for (dx, dy) in [(-1isize, 0isize), (1, 0), (0, -1), (0, 1)] {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    if nx >= 0 && nx < s as isize && ny >= 0 && ny < s as isize {
                        sum += input[ny as usize * s + nx as usize];
                        count += 1;
                    }
                }
                let average = sum / count as f32;
                let current = input[y * s + x];
                output[y * s + x] = current + self.strength * (average - current);
            }
        }
        Arc::new(output)
    }
}
impl Node for NodeErosion {
    fn label(&self) -> &str {
        "Erosion"
    }

    fn size(&self) -> usize {
        3 // 3x3 kernel
    }
    fn inputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "height",
            dtype: NodePortType::Height
        }]
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
            "strength" => Some(NParamValue::Float(self.strength)),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), String> {
        match (key, value) {
            ("strength", NParamValue::Float(v)) => self.strength = v,
            (k, v) => return Err(format!("Unknown parameter {} with value {:?}", k, v))
        }
        Ok(())
    }

    fn process(
        &self,
        pool: &Arc<TilePool>,
        inputs: &[TileHandle]
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![self.process_tile(pool, &inputs[0])])
    }
}
