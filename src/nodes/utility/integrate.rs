use std::sync::{Arc, OnceLock};

use crate::core::node::{
    NParamConstraints, NParamDesc, NParamValue, Node, NodeCategory, NodeDescriptor, NodeError,
    NodeIcon, NodePortType, NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool, bilinear_sample};

const ICON: NodeIcon = NodeIcon {
    id: "node-integrate",
    png_bytes: include_bytes!("../../../assets/icons/node_integrate.png")
};

/// Maps a `Global` node's bare, self-centered result (see `TileContext::for_global`) onto the actual chunked terrain.
#[derive(Debug)]
pub struct Integrate {
    /// World-space location the input's own local origin `(0, 0)` should land on.
    pub position: (f32, f32),
    /// World units the input's full local domain (`[-0.5, 0.5)` on each axis) should span.
    pub scale: f32
}
impl Default for Integrate {
    fn default() -> Self {
        Self {
            position: (0.0, 0.0),
            scale: 1.0
        }
    }
}
impl Integrate {
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
                    constraints: Some(NParamConstraints::FloatRange {
                        min: 0.001,
                        max: 10.0
                    })
                },
            ]
        })
    }
}
impl Node for Integrate {
    fn label(&self) -> &str {
        "Integrate"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::Utility
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
        // The physical footprint is `scale` alone - `input_size` only picks how many texels sample it, not how large it is.
        let input_ctx = TileContext::for_global(input_size);

        let mut output = pool.allocate();
        let s = output.size();
        for y in 0..s {
            for x in 0..s {
                let world = ctx.world_pos(x, y);
                let local = TileContext::to_local(world, self.position, self.scale);
                let (sx, sy) = input_ctx.to_texel(local);
                output[y * s + x] = bilinear_sample(input, input_size, sx, sy);
            }
        }
        Ok(vec![Arc::new(output)])
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "Integrate",
        category: NodeCategory::Utility,
        icon: ICON,
        factory: || Box::new(Integrate::default())
    }
}
