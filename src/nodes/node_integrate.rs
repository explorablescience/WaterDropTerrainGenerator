use std::sync::{Arc, OnceLock};

use crate::core::node::{Node, NodeCategory, NodeIcon, NodePortType, NodeSocket};
use crate::core::node_error::NodeError;
use crate::core::node_parameters::{NParamConstraints, NParamDesc, NParamValue};
use crate::core::node_registry::NodeDescriptor;
use crate::core::tile_allocator::{TileHandle, TilePool, bilinear_sample};
use crate::core::tile_context::TileContext;

const ICON: NodeIcon = NodeIcon {
    id: "node-integrate",
    png_bytes: include_bytes!("../../assets/icons/node_integrate.png")
};

/// Maps a `Global` node's bare, self-centered result (see `TileContext::for_global`) onto the actual chunked terrain.
#[derive(Debug)]
pub struct NodeIntegrate {
    /// World-space location the input's own local origin `(0, 0)` should land on.
    pub position: (f32, f32),
    /// World units the input's full local domain (`[-0.5, 0.5)` on each axis) should span.
    pub scale: f32
}
impl Default for NodeIntegrate {
    fn default() -> Self {
        Self { position: (0.0, 0.0), scale: 1.0 }
    }
}
impl NodeIntegrate {
    fn params() -> &'static [NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        SPECS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "position",
                    label: "Position",
                    category: "Placement",
                    default: NParamValue::Vector2(0.0, 0.0),
                    constraints: Some(NParamConstraints::Vector2Range {
                        min: (-10.0, -10.0),
                        max: (10.0, 10.0)
                    })
                },
                NParamDesc {
                    key: "scale",
                    label: "Scale",
                    category: "Placement",
                    default: NParamValue::Float(1.0),
                    constraints: Some(NParamConstraints::FloatRange { min: 0.001, max: 10.0 })
                },
            ]
        })
    }
}
impl Node for NodeIntegrate {
    fn label(&self) -> &str {
        "Integrate"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::Simulation
    }
    fn icon(&self) -> NodeIcon {
        ICON
    }

    fn inputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Global",
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
            "position" => Some(NParamValue::Vector2(self.position.0, self.position.1)),
            "scale" => Some(NParamValue::Float(self.scale)),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("position", NParamValue::Vector2(x, y)) => self.position = (x, y),
            ("scale", NParamValue::Float(v)) => self.scale = v,
            (k, v) => return Err(format!("Unknown parameter {} with value {:?}", k, v).into())
        }
        Ok(())
    }

    fn process(
        &self,
        pool: &Arc<TilePool>,
        inputs: &[TileHandle],
        ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        let input = &inputs[0];
        let input_size = input.size();
        // The physical footprint is `scale` alone - not `input_size` (native_resolution), which
        // only picks how many texels sample that same footprint, not how large it is.
        let world_footprint = self.scale;

        let mut output = pool.allocate();
        let s = output.size();
        for y in 0..s {
            for x in 0..s {
                let world = ctx.world_pos(x, y);
                // World position -> the input's own local `[-0.5, 0.5)` domain, via this node's
                // scale/position -> the input tile's own texel coordinates.
                let local = (
                    (world.0 - self.position.0) / world_footprint,
                    (world.1 - self.position.1) / world_footprint
                );
                let sx = (local.0 + 0.5) * input_size as f32;
                let sy = (local.1 + 0.5) * input_size as f32;
                output[y * s + x] = bilinear_sample(input, input_size, sx, sy);
            }
        }
        Ok(vec![Arc::new(output)])
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "Integrate",
        category: NodeCategory::Simulation,
        icon: ICON,
        factory: || Box::new(NodeIntegrate::default())
    }
}
