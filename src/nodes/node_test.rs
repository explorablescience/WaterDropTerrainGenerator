use std::sync::{Arc, OnceLock};

use crate::core::node::{Node, NodeCategory, NodeIcon, NodePortType, NodeSocket};
use crate::core::node_error::NodeError;
use crate::core::node_parameters::{NParamConstraints, NParamDesc, NParamValue};
use crate::core::node_registry::NodeDescriptor;
use crate::core::tile_allocator::{TileHandle, TilePool};

/// A node that generates a flat terrain tile.
#[derive(Debug)]
pub struct NodeTest {
    param1: f32,
    param2: i32,
    param3: bool,
    param4: String,
    param5: String,
}
impl Default for NodeTest {
    fn default() -> Self {
        Self {
            param1: 0.5,
            param2: 10,
            param3: true,
            param4: "default".to_string(),
            param5: "option1".to_string(),
        }
    }
}
impl Node for NodeTest {
    fn label(&self) -> &str {
        "Test Node"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::Generator
    }
    fn icon(&self) -> NodeIcon {
        NodeIcon::Plane
    }

    fn inputs(&self) -> &[NodeSocket] {
        &[
            NodeSocket {
                name: "Color",
                dtype: NodePortType::Color,
            },
            NodeSocket {
                name: "Mask",
                dtype: NodePortType::Mask,
            },
            NodeSocket {
                name: "Height",
                dtype: NodePortType::Height,
            },
            NodeSocket {
                name: "Scalar",
                dtype: NodePortType::Scalar,
            },
            NodeSocket {
                name: "Vector",
                dtype: NodePortType::Vector,
            },
        ]
    }

    fn outputs(&self) -> &[NodeSocket] {
        &[
            NodeSocket {
                name: "Height",
                dtype: NodePortType::Height,
            },
            NodeSocket {
                name: "Mask",
                dtype: NodePortType::Mask,
            },
            NodeSocket {
                name: "Color",
                dtype: NodePortType::Color,
            },
            NodeSocket {
                name: "Vector",
                dtype: NodePortType::Vector,
            },
            NodeSocket {
                name: "Scalar",
                dtype: NodePortType::Scalar,
            },
        ]
    }

    fn desc_params(&self) -> &[NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = std::sync::OnceLock::new();
        SPECS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "param1",
                    label: "Parameter 1",
                    category: "General",
                    default: NParamValue::Float(0.5),
                    constraints: Some(NParamConstraints::FloatRange { min: 0.0, max: 1.0 }),
                },
                NParamDesc {
                    key: "param2",
                    label: "Parameter 2",
                    category: "General",
                    default: NParamValue::Int(10),
                    constraints: Some(NParamConstraints::IntRange { min: 0, max: 100 }),
                },
                NParamDesc {
                    key: "param3",
                    label: "Parameter 3",
                    category: "Advanced",
                    default: NParamValue::Bool(true),
                    constraints: None,
                },
                NParamDesc {
                    key: "param4",
                    label: "Parameter 4",
                    category: "Advanced",
                    default: NParamValue::String("default".to_string()),
                    constraints: Some(NParamConstraints::StringMaxLength { max_length: 10 }),
                },
                NParamDesc {
                    key: "param5",
                    label: "Parameter 5",
                    category: "Advanced",
                    default: NParamValue::Enum("option1".to_string()),
                    constraints: Some(NParamConstraints::EnumOneOf {
                        options: vec!["option1", "option2", "option3"],
                    }),
                },
            ]
        })
    }

    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), String> {
        match key {
            "param1" => {
                if let NParamValue::Float(v) = value {
                    self.param1 = v;
                    Ok(())
                } else {
                    Err("Invalid value for param1".into())
                }
            }
            "param2" => {
                if let NParamValue::Int(v) = value {
                    self.param2 = v;
                    Ok(())
                } else {
                    Err("Invalid value for param2".into())
                }
            }
            "param3" => {
                if let NParamValue::Bool(v) = value {
                    self.param3 = v;
                    Ok(())
                } else {
                    Err("Invalid value for param3".into())
                }
            }
            "param4" => {
                if let NParamValue::String(v) = value {
                    self.param4 = v;
                    Ok(())
                } else {
                    Err("Invalid value for param4".into())
                }
            }
            "param5" => {
                if let NParamValue::Enum(v) = value {
                    self.param5 = v;
                    Ok(())
                } else {
                    Err("Invalid value for param5".into())
                }
            }
            _ => Err(format!("Unknown parameter: {}", key)),
        }
    }

    fn get_param(&self, key: &str) -> Option<NParamValue> {
        match key {
            "param1" => Some(NParamValue::Float(self.param1)),
            "param2" => Some(NParamValue::Int(self.param2)),
            "param3" => Some(NParamValue::Bool(self.param3)),
            "param4" => Some(NParamValue::String(self.param4.clone())),
            "param5" => Some(NParamValue::Enum(self.param5.clone())),
            _ => None,
        }
    }

    fn process(
        &self,
        pool: &Arc<TilePool>,
        _inputs: &[TileHandle],
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![Arc::new(pool.allocate())])
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "Test Node",
        category: NodeCategory::Generator,
        icon: NodeIcon::Plane,
        factory: || Box::new(NodeTest::default())
    }
}
