//! A minimal erosion node, used as a template for the terrain-generation node graph.

use std::sync::Arc;

use crate::core::allocator::{TileHandle, TilePool};
use crate::core::node::{Node, NodeError, NodeSocket, PortType};

/// A minimal thermal-erosion node
pub struct NodeErosion {
    /// Width (and height) in texels of the square tile this node operates on.
    width: usize,
    /// How strongly each texel is pulled towards its neighbours' average height, in `[0, 1]`.
    strength: f32,
}

impl NodeErosion {
    pub fn new(width: usize, strength: f32) -> Self {
        Self { width, strength }
    }
}

impl Node for NodeErosion {
    fn name(&self) -> &str {
        "Erosion"
    }

    fn inputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "height",
            dtype: PortType::Height,
        }]
    }

    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "height",
            dtype: PortType::Height,
        }]
    }

    fn process(
        &self,
        pool: &Arc<TilePool>,
        inputs: &[TileHandle],
    ) -> Result<Vec<TileHandle>, NodeError> {
        assert_eq!(inputs.len(), 1, "Erosion node expects a single input tile");
        let in_height = &inputs[0];
        assert_eq!(
            in_height.len(),
            self.width * self.width,
            "Erosion node expects a {}x{} input tile",
            self.width,
            self.width
        );

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
                        sum += in_height[ny as usize * self.width + nx as usize];
                        count += 1.0;
                    }
                }
                let avg = if count > 0.0 {
                    sum / count
                } else {
                    in_height[idx]
                };
                output[idx] = in_height[idx] + (avg - in_height[idx]) * self.strength;
            }
        }

        Ok(vec![Arc::new(output)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smooths_a_single_spike_towards_its_neighbours() {
        let pool = TilePool::new(9); // 3x3 tile
        let mut input = pool.allocate();
        input.copy_from_slice(&[0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]);

        let erosion = NodeErosion::new(3, 0.5);
        let output = erosion
            .process(&pool, &[Arc::new(input)])
            .expect("erosion should succeed on a matching tile");

        // The centre texel should have moved halfway towards its neighbours' average (0.0).
        assert_eq!(output.len(), 1);
        assert!((output[0][4] - 0.5).abs() < 1e-6);
        // A corner texel (no raised neighbour) should stay untouched.
        assert_eq!(output[0][0], 0.0);
    }

    #[test]
    fn rejects_mismatched_tile_sizes() {
        let pool = TilePool::new(4);
        let input = pool.allocate();
        let erosion = NodeErosion::new(3, 0.5);
        assert!(erosion.process(&pool, &[Arc::new(input)]).is_err());
    }
}
