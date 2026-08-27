use std::sync::{Arc, OnceLock};

use crate::core::node::{
    NParamConstraints, NParamDesc, NParamValue, Node, NodeCategory, NodeDescriptor, NodeError,
    NodeIcon, NodeLocality, NodePortType, NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-mountain",
    png_bytes: include_bytes!("../../../../assets/icons/node_mountain.png")
};

/// A very basic `Global` primitive: one smooth dome.
#[derive(Debug)]
pub struct Mountain {
    pub height: f32,
    pub radius: f32,
    pub native_resolution: u32
}
impl Default for Mountain {
    fn default() -> Self {
        Self {
            height: 2.0,
            radius: 0.1,
            native_resolution: 256
        }
    }
}
impl Mountain {
    fn params() -> &'static [NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        SPECS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "height",
                    label: "Height",
                    category: "Shape",
                    default: NParamValue::Float(1.0),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: 0.0,
                        max: 10.0
                    })
                },
                NParamDesc {
                    key: "radius",
                    label: "Radius",
                    category: "Shape",
                    default: NParamValue::Float(0.3),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: 0.01,
                        max: 2.0
                    })
                },
                NParamDesc {
                    key: "native_resolution",
                    label: "Native Resolution",
                    category: "Shape",
                    default: NParamValue::Int(256),
                    constraints: Some(NParamConstraints::IntRange { min: 16, max: 4096 })
                },
            ]
        })
    }

    /// Smooth radial falloff from `center`: `self.height` at the center, `0` at `self.radius` and beyond.
    fn dome(&self, local: (f32, f32), center: (f32, f32)) -> f32 {
        let dx = local.0 - center.0;
        let dy = local.1 - center.1;
        let dist = (dx * dx + dy * dy).sqrt();
        let t = (1.0 - dist / self.radius).clamp(0.0, 1.0);
        let falloff = t * t * (3.0 - 2.0 * t); // smoothstep
        falloff * self.height
    }
}
impl Node for Mountain {
    fn label(&self) -> &str {
        "Mountain"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::Generation
    }
    fn icon(&self) -> NodeIcon {
        ICON
    }

    fn locality(&self) -> NodeLocality {
        NodeLocality::Global {
            native_resolution: self.native_resolution as usize
        }
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
            "height" => Some(NParamValue::Float(self.height)),
            "radius" => Some(NParamValue::Float(self.radius)),
            "native_resolution" => Some(NParamValue::Int(self.native_resolution as i32)),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("height", NParamValue::Float(v)) => self.height = v,
            ("radius", NParamValue::Float(v)) => self.radius = v,
            ("native_resolution", NParamValue::Int(v)) => self.native_resolution = v as u32,
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
        let s = output.size();
        for y in 0..s {
            for x in 0..s {
                let local = ctx.local_pos(x, y);
                output[y * s + x] = self.dome(local, (0.0, 0.0));
            }
        }
        Ok(vec![Arc::new(output)])
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "Mountain",
        category: NodeCategory::Generation,
        subcategory: "Primitives",
        icon: ICON,
        factory: || Box::new(Mountain::default())
    }
}
