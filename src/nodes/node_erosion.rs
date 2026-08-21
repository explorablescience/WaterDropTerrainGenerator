use std::sync::{Arc, OnceLock};

use crate::core::tile_allocator::{TileHandle, TilePool};
use crate::core::node::{Node, NodeError, NodePortType, NodeSocket};
use crate::core::node_parameters::{NParamConstraints, NParamDesc, NParamValue};

/// A minimal thermal-erosion node
#[derive(Debug)]
pub struct NodeErosion {
    /// Width (and height) in texels of the square tile this node operates on.
    width: usize,
    /// How strongly each texel is pulled towards its neighbours' average height, in `[0, 1]`.
    strength: f32,
}
impl Default for NodeErosion {
    fn default() -> Self {
        Self {
            width: 3,
            strength: 0.5,
        }
    }
}
impl NodeErosion {
    fn params() -> &'static [NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        SPECS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "strength",
                    label: "Strength",
                    default: NParamValue::Float(0.5),
                    constraints: Some(NParamConstraints::FloatRange { min: 0.0, max: 1.0 }),
                },
                NParamDesc {
                    key: "width",
                    label: "Width",
                    default: NParamValue::Int(3),
                    constraints: Some(NParamConstraints::IntRange { min: 1, max: 1024 }),
                },
            ]
        })
    }

    fn process_tile(&self, pool: &Arc<TilePool>, input: &TileHandle) -> TileHandle {
        let mut output = pool.allocate();
        for y in 0..self.width {
            for x in 0..self.width {
                let idx = y * self.width + x;
                let mut sum = 0.0;
                let mut count = 0.0;
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx >= 0 && nx < self.width as i32 && ny >= 0 && ny < self.width as i32 {
                        sum += input[ny as usize * self.width + nx as usize];
                        count += 1.0;
                    }
                }
                let avg = if count > 0.0 { sum / count } else { input[idx] };
                output[idx] = input[idx] + (avg - input[idx]) * self.strength;
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
        self.width
    }
    fn inputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "height",
            dtype: NodePortType::Height,
        }]
    }
    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "height",
            dtype: NodePortType::Height,
        }]
    }

    fn desc_params(&self) -> &'static [NParamDesc] {
        Self::params()
    }
    fn get_param(&self, key: &str) -> Option<NParamValue> {
        match key {
            "strength" => Some(NParamValue::Float(self.strength)),
            "width" => Some(NParamValue::Int(self.width as i32)),
            _ => None,
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), String> {
        match (key, value) {
            ("strength", NParamValue::Float(v)) => self.strength = v,
            ("width", NParamValue::Int(v)) => self.width = v as usize,
            (k, v) => return Err(format!("Unknown parameter {} with value {:?}", k, v)),
        }
        Ok(())
    }

    fn process(
        &self,
        pool: &Arc<TilePool>,
        inputs: &[TileHandle],
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![self.process_tile(pool, &inputs[0])])
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_erosion() {
        let pool = TilePool::new(9); // 3x3 tile
        let mut input = pool.allocate();
        input.copy_from_slice(&[0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]);

        let erosion = NodeErosion::default();
        let output = erosion
            .process(&pool, &[Arc::new(input)])
            .expect("erosion should succeed on a matching tile");

        // The centre texel should have moved halfway towards its neighbours' average (0.0).
        assert_eq!(output.len(), 1);
        assert!((output[0][4] - 0.5).abs() < 1e-6);
        // A corner texel (no raised neighbour) should stay untouched.
        assert_eq!(output[0][0], 0.0);
    }
}
