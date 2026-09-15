use std::sync::{Arc, OnceLock};

use rayon::prelude::*;

use crate::core::*;

const ICON: NodeIcon = NodeIcon {
    id: "node-clip",
    png_bytes: include_bytes!("../../../assets/icons/node_clip.png")
};

/// Unlike `Clamp`, which pins out-of-range texels to the range's boundary, `Clip` discards them
/// entirely, replacing any height outside `[min_height, max_height]` with `clip_value` - useful for
/// isolating a height band or flattening extremes to a fixed level (e.g. sea level). All three are
/// fractions of the terrain's overall height (see `TileContext::terrain_height`), not world units.
#[derive(Debug, Clone)]
pub struct Clip {
    min_height: f32,
    max_height: f32,
    clip_value: f32
}
impl Default for Clip {
    fn default() -> Self {
        Self {
            min_height: 0.0,
            max_height: 1.0,
            clip_value: 0.0
        }
    }
}
impl Clip {
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
                NParamDesc {
                    key: "clip_value",
                    label: "Clip Value",
                    category: "Range",
                    default: NParamValue::Float(0.0),
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
        let min_h =
            (self.min_height * ctx.terrain_height).min(self.max_height * ctx.terrain_height);
        let max_h =
            (self.max_height * ctx.terrain_height).max(self.min_height * ctx.terrain_height);
        let clip_h = self.clip_value * ctx.terrain_height;
        output.par_iter_mut().enumerate().for_each(|(idx, texel)| {
            let h = height[idx];
            *texel = if h < min_h || h > max_h { clip_h } else { h };
        });
        Arc::new(output)
    }
}
impl Node for Clip {
    fn label(&self) -> &str {
        "Clip"
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
            "clip_value" => Some(NParamValue::Float(self.clip_value)),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("min_height", NParamValue::Float(v)) => self.min_height = v,
            ("max_height", NParamValue::Float(v)) => self.max_height = v,
            ("clip_value", NParamValue::Float(v)) => self.clip_value = v,
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
        label: "Clip",
        category: NodeCategory::Modification,
        subcategory: "Height Adjustments",
        icon: ICON,
        factory: || Box::new(Clip::default())
    }
}
