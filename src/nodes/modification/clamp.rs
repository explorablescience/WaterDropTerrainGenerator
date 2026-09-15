use std::sync::{Arc, OnceLock};

use rayon::prelude::*;

use crate::core::*;

const ICON: NodeIcon = NodeIcon {
    id: "node-clamp",
    png_bytes: include_bytes!("../../../assets/icons/node_clamp.png")
};

/// Pins height values outside `[min_height, max_height]` to that range's boundary. Both are
/// fractions of the terrain's overall height (see `TileContext::terrain_height`), not world units.
#[derive(Debug, Clone)]
pub struct Clamp {
    min_height: f32,
    max_height: f32
}
impl Default for Clamp {
    fn default() -> Self {
        Self {
            min_height: 0.0,
            max_height: 1.0
        }
    }
}
impl Clamp {
    fn desc_params() -> &'static [NParamDesc] {
        static PARAMS: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        PARAMS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "min_height",
                    label: "Min Height",
                    category: "Range",
                    default: NParamValue::Float(0.0),
                    constraints: Some(NParamConstraints::FloatRange { min: 0.0, max: 1.0 }),
                    unit: ParamUnit::Percent
                },
                NParamDesc {
                    key: "max_height",
                    label: "Max Height",
                    category: "Range",
                    default: NParamValue::Float(1.0),
                    constraints: Some(NParamConstraints::FloatRange { min: 0.0, max: 1.0 }),
                    unit: ParamUnit::Percent
                },
            ]
        })
    }

    fn process_tile(
        &self,
        pool: &Arc<TilePool>,
        inputs: &[TileHandle],
        ctx: &TileContext
    ) -> TileHandle {
        let mut output = pool.allocate();
        let height = &inputs[0];
        let min_h = self.min_height * ctx.terrain_height;
        let max_h = self.max_height * ctx.terrain_height;
        output.par_iter_mut().enumerate().for_each(|(idx, texel)| {
            *texel = height[idx].clamp(min_h.min(max_h), max_h.max(min_h))
        });
        Arc::new(output)
    }
}
impl Node for Clamp {
    fn label(&self) -> &str {
        "Clamp"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::Modification
    }
    fn icon(&self) -> NodeIcon {
        ICON
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
            name: "Height",
            dtype: SocketDtype::Fixed(NodePortType::Height),
            required: true
        }]
    }

    fn desc_params(&self) -> &[NParamDesc] {
        Self::desc_params()
    }
    fn get_param(&self, key: &str) -> Option<NParamValue> {
        match key {
            "min_height" => Some(NParamValue::Float(self.min_height)),
            "max_height" => Some(NParamValue::Float(self.max_height)),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("min_height", NParamValue::Float(v)) => self.min_height = v,
            ("max_height", NParamValue::Float(v)) => self.max_height = v,
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
        Ok(vec![self.process_tile(pool, inputs, ctx)])
    }

    fn clone_boxed(&self) -> Box<dyn Node> {
        Box::new(self.clone())
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "Clamp",
        category: NodeCategory::Modification,
        subcategory: "Height Adjustments",
        icon: ICON,
        factory: || Box::new(Clamp::default())
    }
}
