use std::sync::{Arc, OnceLock};

use rayon::prelude::*;

use crate::core::node::{
    NParamConstraints, NParamDesc, NParamValue, Node, NodeCategory, NodeDescriptor, NodeError,
    NodeIcon, NodePortType, NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-combine",
    png_bytes: include_bytes!("../../../assets/icons/node_combine.png")
};

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

    fn process_tile(&self, pool: &Arc<TilePool>, inputs: &[TileHandle]) -> TileHandle {
        let mut output = pool.allocate();
        let s = output.size();
        output.par_chunks_mut(s).enumerate().for_each(|(y, row)| {
            for x in 0..s {
                let mut sum = 0.0;
                let mut count = 0;
                for input in inputs {
                    let val = input[y * s + x];
                    if val.is_finite() {
                        sum += val;
                        count += 1;
                    }
                }
                row[x] = if count > 0 {
                    sum / count as f32
                } else {
                    f32::NAN
                };
            }
        });
        Arc::new(output)
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
        Ok(vec![self.process_tile(pool, inputs)])
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
