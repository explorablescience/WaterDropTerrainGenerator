use std::sync::{Arc, OnceLock};

use crate::core::gpu;
use crate::core::node::{
    NParamConstraints, NParamDesc, NParamValue, Node, NodeCategory, NodeDescriptor, NodeError,
    NodeIcon, NodePortType, NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-combine",
    png_bytes: include_bytes!("../../../assets/icons/node_combine.png")
};

const SHADER: &str = include_str!("combine.comp.wgsl");
const WORKGROUP_SIZE: u32 = 8;

/// Layout must match `combine.comp.wgsl`'s `Params` struct exactly.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CombineParams {
    tile_size: [u32; 4]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum CombineMethod {
    #[default]
    Average
}
impl CombineMethod {
    fn to_str(self) -> &'static str {
        match self {
            CombineMethod::Average => "Average"
        }
    }
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "Average" => Some(CombineMethod::Average),
            _ => None
        }
    }
    fn all_options() -> Vec<&'static str> {
        vec!["Average"]
    }
}

/// Combine multiple heightmaps using multiple compositing methods.
#[derive(Debug, Default)]
pub struct Combine {
    method: CombineMethod
}
impl Combine {
    fn params() -> &'static [NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        SPECS.get_or_init(|| {
            vec![NParamDesc {
                key: "method",
                label: "Method",
                category: "Combining",
                default: NParamValue::Enum(CombineMethod::Average.to_str().to_string()),
                constraints: Some(NParamConstraints::EnumOneOf {
                    options: CombineMethod::all_options()
                })
            }]
        })
    }

}
impl Node for Combine {
    fn label(&self) -> &str {
        "Combine"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::Utility
    }
    fn icon(&self) -> NodeIcon {
        ICON
    }

    fn inputs(&self) -> &[NodeSocket] {
        &[
            NodeSocket {
                name: "Height A",
                dtype: NodePortType::Height,
                required: true
            },
            NodeSocket {
                name: "Height B",
                dtype: NodePortType::Height,
                required: true
            }
        ]
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
            "method" => Some(NParamValue::Enum(self.method.to_str().to_string())),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("method", NParamValue::Enum(v)) => {
                self.method =
                    CombineMethod::from_str(&v).ok_or_else(|| format!("Invalid method: {}", v))?
            }
            (k, v) => return Err(format!("Unknown parameter {} with value {:?}", k, v).into())
        }
        Ok(())
    }

    fn process(
        &self,
        pool: &Arc<TilePool>,
        inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        let mut output = pool.allocate();
        let size = output.size() as u32;
        let params = CombineParams {
            tile_size: [size, 0, 0, 0]
        };
        let input_slices: Vec<&[f32]> = inputs.iter().map(|t| &t[..]).collect();
        let workgroups = size.div_ceil(WORKGROUP_SIZE);
        let result = gpu::dispatch_f32(
            "combine",
            SHADER,
            &params,
            &input_slices,
            (size * size) as usize,
            (workgroups, workgroups, 1)
        )?;
        output.copy_from_slice(&result);
        Ok(vec![Arc::new(output)])
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "Combine",
        category: NodeCategory::Utility,
        subcategory: "Compositing",
        icon: ICON,
        factory: || Box::new(Combine::default())
    }
}
