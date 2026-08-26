use std::sync::{Arc, OnceLock};

use crate::core::node::{Node, NodeCategory, NodeIcon, NodeLocality, NodePortType, NodeSocket};
use crate::core::node_error::NodeError;
use crate::core::node_parameters::{NParamConstraints, NParamDesc, NParamValue};
use crate::core::node_registry::NodeDescriptor;
use crate::core::tile_allocator::{TileHandle, TilePool};
use crate::core::tile_context::TileContext;

const ICON: NodeIcon = NodeIcon {
    id: "node-mountain",
    png_bytes: include_bytes!("../../assets/icons/node_mountain.png")
};

/// Resolution (texels per axis) of the whole-terrain buffer this node computes its shape into,
/// independent of the chunk grid - see [`NodeLocality::Global`].
const NATIVE_RESOLUTION: usize = 256;

/// A very basic `Global` primitive: one smooth dome centered on the terrain. A mountain like this
/// doesn't tile - it has to be computed once over the whole extent rather than per chunk, so each
/// chunk instead gets its own cropped, resampled slice of the single shared shape.
#[derive(Debug)]
pub struct NodeMountain {
    pub center: (f32, f32),
    pub height: f32,
    pub radius: f32
}
impl Default for NodeMountain {
    fn default() -> Self {
        Self { center: (0.5, 0.5), height: 1.0, radius: 2.0 }
    }
}
impl NodeMountain {
    fn params() -> &'static [NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        SPECS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "center",
                    label: "Center",
                    category: "Shape",
                    default: NParamValue::Vector2(0.5, 0.5),
                    constraints: Some(NParamConstraints::Vector2Range {
                        min: (0.0, 0.0),
                        max: (1.0, 1.0)
                    })
                },
                NParamDesc {
                    key: "height",
                    label: "Height",
                    category: "Shape",
                    default: NParamValue::Float(1.0),
                    constraints: Some(NParamConstraints::FloatRange { min: 0.0, max: 10.0 })
                },
                NParamDesc {
                    key: "radius",
                    label: "Radius",
                    category: "Shape",
                    default: NParamValue::Float(2.0),
                    constraints: Some(NParamConstraints::FloatRange { min: 0.1, max: 50.0 })
                },
            ]
        })
    }

    /// Smooth radial falloff from `center`: `self.height` at the center, `0` at `self.radius` and
    /// beyond.
    fn dome(&self, world: (f32, f32), center: (f32, f32)) -> f32 {
        let dx = world.0 - center.0;
        let dy = world.1 - center.1;
        let dist = (dx * dx + dy * dy).sqrt();
        let t = (1.0 - dist / self.radius).clamp(0.0, 1.0);
        let falloff = t * t * (3.0 - 2.0 * t); // smoothstep
        falloff * self.height
    }
}
impl Node for NodeMountain {
    fn label(&self) -> &str {
        "Mountain"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::Generator
    }
    fn icon(&self) -> NodeIcon {
        ICON
    }

    fn locality(&self) -> NodeLocality {
        NodeLocality::Global { native_resolution: NATIVE_RESOLUTION }
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
            "center" => Some(NParamValue::Vector2(self.center.0, self.center.1)),
            "height" => Some(NParamValue::Float(self.height)),
            "radius" => Some(NParamValue::Float(self.radius)),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("center", NParamValue::Vector2(x, y)) => self.center = (x, y),
            ("height", NParamValue::Float(v)) => self.height = v,
            ("radius", NParamValue::Float(v)) => self.radius = v,
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
        let center = (self.center.0 * ctx.world_size().0, self.center.1 * ctx.world_size().1);
        for y in 0..s {
            for x in 0..s {
                let world_pos = ctx.world_pos(x, y);
                output[y * s + x] = self.dome(world_pos, center);
            }
        }
        Ok(vec![Arc::new(output)])
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "Mountain",
        category: NodeCategory::Generator,
        icon: ICON,
        factory: || Box::new(NodeMountain::default())
    }
}
