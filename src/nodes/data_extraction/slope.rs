use std::sync::Arc;

use rayon::prelude::*;

use crate::core::*;

const ICON: NodeIcon = NodeIcon {
    id: "node-slope",
    png_bytes: include_bytes!("../../../assets/icons/node_slope.png")
};

/// Extracts terrain steepness as a Mask: 0 where flat, 1 where vertical, from the central-difference
/// gradient of the height input normalized by its incline angle (`atan(gradient) / (pi/2)`).
#[derive(Debug, Default, Clone)]
pub struct Slope;
impl Slope {
    fn process_tile(
        &self,
        pool: &Arc<TilePool>,
        inputs: &[TileHandle],
        ctx: &TileContext
    ) -> TileHandle {
        let mut output = pool.allocate();
        let s = output.size();
        let height = &inputs[0];
        let (step_x, step_z) = ctx.world_step;

        output.par_chunks_mut(s).enumerate().for_each(|(y, row)| {
            let y0 = y.saturating_sub(1);
            let y1 = (y + 1).min(s - 1);
            for (x, texel) in row.iter_mut().enumerate() {
                let x0 = x.saturating_sub(1);
                let x1 = (x + 1).min(s - 1);
                let dhdx =
                    (height[y * s + x1] - height[y * s + x0]) / (step_x * (x1 - x0).max(1) as f32);
                let dhdz =
                    (height[y1 * s + x] - height[y0 * s + x]) / (step_z * (y1 - y0).max(1) as f32);
                let gradient = (dhdx * dhdx + dhdz * dhdz).sqrt();
                *texel = gradient.atan() / std::f32::consts::FRAC_PI_2;
            }
        });

        Arc::new(output)
    }
}
impl Node for Slope {
    fn label(&self) -> &str {
        "Slope"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::DataExtraction
    }
    fn icon(&self) -> NodeIcon {
        ICON
    }

    /// Reads one neighbor texel on each side for the central-difference gradient.
    fn size(&self) -> usize {
        1
    }

    fn inputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: SocketDtype::Fixed(NodePortType::Height),
            required: true
        }]
    }
    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Slope",
            dtype: SocketDtype::Fixed(NodePortType::Mask),
            required: true
        }]
    }

    fn process(
        &self,
        pool: &Arc<TilePool>,
        inputs: &[TileHandle],
        ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![self.process_tile(pool, inputs, ctx)])
    }

    fn clone_boxed(&self) -> Box<dyn Node> {
        Box::new(self.clone())
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "Slope",
        category: NodeCategory::DataExtraction,
        subcategory: "Topographic Analysis",
        icon: ICON,
        factory: || Box::new(Slope)
    }
}
