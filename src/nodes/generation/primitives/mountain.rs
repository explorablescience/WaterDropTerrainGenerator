use std::sync::{Arc, OnceLock};

use rayon::prelude::*;

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
    pub native_resolution: u32,
    /// World units this node's own local domain spans when previewed standalone; an `Integrate` downstream multiplies its own `scale` by this.
    pub world_size: f32
}
impl Default for Mountain {
    fn default() -> Self {
        Self {
            height: 2.0,
            radius: 0.1,
            native_resolution: 256,
            world_size: 25.6
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
                NParamDesc {
                    key: "world_size",
                    label: "World Size",
                    category: "Placement",
                    default: NParamValue::Float(25.6),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: 0.1,
                        max: 500.0
                    })
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
            native_resolution: self.native_resolution as usize,
            world_size: self.world_size
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
            "world_size" => Some(NParamValue::Float(self.world_size)),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("height", NParamValue::Float(v)) => self.height = v,
            ("radius", NParamValue::Float(v)) => self.radius = v,
            ("native_resolution", NParamValue::Int(v)) => self.native_resolution = v as u32,
            ("world_size", NParamValue::Float(v)) => self.world_size = v,
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
        output.par_chunks_mut(s).enumerate().for_each(|(y, row)| {
            for (x, texel) in row.iter_mut().enumerate() {
                let local = ctx.local_pos(x, y);
                *texel = self.dome(local, (0.0, 0.0));
            }
        });
        output.world_size = self.world_size;
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
