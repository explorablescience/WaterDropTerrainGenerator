use std::sync::{Arc, OnceLock};

use crate::core::gpu;
use crate::core::node::{
    NParamConstraints, NParamDesc, NParamValue, Node, NodeCategory, NodeDescriptor, NodeError,
    NodeIcon, NodePortType, NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-mountain",
    png_bytes: include_bytes!("../../../../assets/icons/node_mountain.png")
};

const SHADER: &str = include_str!("mountain.comp.wgsl");
const WORKGROUP_SIZE: u32 = 8;

/// Layout must match `mountain.comp.wgsl`'s `Params` struct exactly (vec4-aligned fields).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MountainParams {
    height_radius_pos: [f32; 4],
    origin_step: [f32; 4],
    tile_size: [u32; 4]
}

/// A basic `Local` primitive: one smooth dome, pointwise in world space (no neighbor reads, no
/// whole-domain statistic), so it needs no padding - `position` and `radius` are plain world units.
#[derive(Debug)]
pub struct Mountain {
    pub height: f32,
    pub radius: f32,
    pub position: (f32, f32)
}
impl Default for Mountain {
    fn default() -> Self {
        Self {
            height: 2.0,
            radius: 2.5,
            position: (0.0, 0.0)
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
                    default: NParamValue::Float(2.5),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: 0.1,
                        max: 20.0
                    })
                },
                NParamDesc {
                    key: "position",
                    label: "Position",
                    category: "Placement",
                    default: NParamValue::Vector2(0.0, 0.0),
                    constraints: Some(NParamConstraints::Vector2Range {
                        min: (-50.0, -50.0),
                        max: (50.0, 50.0)
                    })
                },
            ]
        })
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
            "position" => Some(NParamValue::Vector2(self.position.0, self.position.1)),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("height", NParamValue::Float(v)) => self.height = v,
            ("radius", NParamValue::Float(v)) => self.radius = v,
            ("position", NParamValue::Vector2(x, y)) => self.position = (x, y),
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
        let size = output.size() as u32;
        let params = MountainParams {
            height_radius_pos: [self.height, self.radius, self.position.0, self.position.1],
            origin_step: [
                ctx.world_origin.0,
                ctx.world_origin.1,
                ctx.world_step.0,
                ctx.world_step.1,
            ],
            tile_size: [size, 0, 0, 0]
        };
        let workgroups = size.div_ceil(WORKGROUP_SIZE);
        let result = gpu::dispatch_f32(
            "mountain",
            SHADER,
            &params,
            &[],
            (size * size) as usize,
            (workgroups, workgroups, 1)
        )?;
        output.copy_from_slice(&result);
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
