use std::sync::{Arc, OnceLock};

use rayon::prelude::*;

use crate::core::*;

const ICON: NodeIcon = NodeIcon {
    id: "node-combine",
    png_bytes: include_bytes!("../../../assets/icons/node_combine.png")
};

const SHADER: &str = include_str!("combine.comp.wgsl");
const WORKGROUP_SIZE: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum CombineMethod {
    #[default]
    Blend,
    Add,
    Subtract,
    Multiply,
    Screen,
    Difference
}
impl CombineMethod {
    fn to_str(self) -> &'static str {
        match self {
            CombineMethod::Blend => "Blend",
            CombineMethod::Add => "Add",
            CombineMethod::Subtract => "Subtract",
            CombineMethod::Multiply => "Multiply",
            CombineMethod::Screen => "Screen",
            CombineMethod::Difference => "Difference"
        }
    }
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "Blend" => Some(CombineMethod::Blend),
            "Add" => Some(CombineMethod::Add),
            "Subtract" => Some(CombineMethod::Subtract),
            "Multiply" => Some(CombineMethod::Multiply),
            "Screen" => Some(CombineMethod::Screen),
            "Difference" => Some(CombineMethod::Difference),
            _ => None
        }
    }
    fn all_options() -> Vec<&'static str> {
        vec![
            "Blend",
            "Add",
            "Subtract",
            "Multiply",
            "Screen",
            "Difference",
        ]
    }

    /// Must match `combine.comp.wgsl`'s `combine_val` dispatch exactly.
    fn gpu_id(self) -> u32 {
        match self {
            CombineMethod::Blend => 0,
            CombineMethod::Add => 1,
            CombineMethod::Subtract => 2,
            CombineMethod::Multiply => 3,
            CombineMethod::Screen => 4,
            CombineMethod::Difference => 5
        }
    }

    /// `strength`'s label/default/max, tailored to what it means for this method.
    fn strength_desc(self) -> (&'static str, f32, f32) {
        match self {
            CombineMethod::Blend => ("Blend Factor", 0.5, 1.0),
            CombineMethod::Add | CombineMethod::Subtract => ("Strength", 1.0, 2.0),
            CombineMethod::Multiply | CombineMethod::Screen | CombineMethod::Difference => {
                ("Amount", 1.0, 1.0)
            }
        }
    }

    fn combine(self, a: f32, b: f32, strength: f32) -> f32 {
        match self {
            CombineMethod::Blend => lerp(a, b, strength),
            CombineMethod::Add => a + strength * b,
            CombineMethod::Subtract => a - strength * b,
            CombineMethod::Multiply => lerp(a, a * b, strength),
            CombineMethod::Screen => lerp(a, 1.0 - (1.0 - a) * (1.0 - b), strength),
            CombineMethod::Difference => lerp(a, (a - b).abs(), strength)
        }
    }
}
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Layout must match `combine.comp.wgsl`'s `Params` struct exactly (vec4-aligned fields).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CombineParams {
    strength: [f32; 4],
    /// `[method_id, tile_size, unused, unused]`.
    method: [u32; 4]
}

/// Combines two heightmaps with a chosen blend method (Blend/Add/Subtract/Multiply/Screen/Difference).
#[derive(Debug, Clone)]
pub struct Combine {
    method: CombineMethod,
    strength: f32
}
impl Default for Combine {
    fn default() -> Self {
        let method = CombineMethod::default();
        Self {
            method,
            strength: method.strength_desc().1
        }
    }
}
impl Combine {
    fn method_desc() -> NParamDesc {
        NParamDesc {
            key: "method",
            label: "Method",
            category: "Combining",
            default: NParamValue::Enum(CombineMethod::default().to_str().to_string()),
            constraints: Some(NParamConstraints::EnumOneOf {
                options: CombineMethod::all_options()
            })
        }
    }

    fn strength_desc(method: CombineMethod) -> NParamDesc {
        let (label, default, max) = method.strength_desc();
        NParamDesc {
            key: "strength",
            label,
            category: "Combining",
            default: NParamValue::Float(default),
            constraints: Some(NParamConstraints::FloatRange { min: 0.0, max })
        }
    }

    /// One static param list per method, each pairing the shared "Method" selector with a
    /// `strength` descriptor tailored to that method - computed once per method, not per instance.
    fn params_for(method: CombineMethod) -> &'static [NParamDesc] {
        static BLEND: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        static ADD: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        static SUBTRACT: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        static MULTIPLY: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        static SCREEN: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        static DIFFERENCE: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        let cell = match method {
            CombineMethod::Blend => &BLEND,
            CombineMethod::Add => &ADD,
            CombineMethod::Subtract => &SUBTRACT,
            CombineMethod::Multiply => &MULTIPLY,
            CombineMethod::Screen => &SCREEN,
            CombineMethod::Difference => &DIFFERENCE
        };
        cell.get_or_init(|| vec![Self::method_desc(), Self::strength_desc(method)])
    }

    fn process_tile(&self, pool: &Arc<TilePool>, inputs: &[TileHandle]) -> TileHandle {
        let mut output = pool.allocate();
        let s = output.size();
        let (a, b) = (&inputs[0], &inputs[1]);
        let (method, strength) = (self.method, self.strength);
        output.par_chunks_mut(s).enumerate().for_each(|(y, row)| {
            for (x, texel) in row.iter_mut().enumerate() {
                let idx = y * s + x;
                *texel = method.combine(a[idx], b[idx], strength);
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
        Self::params_for(self.method)
    }
    fn get_param(&self, key: &str) -> Option<NParamValue> {
        match key {
            "method" => Some(NParamValue::Enum(self.method.to_str().to_string())),
            "strength" => Some(NParamValue::Float(self.strength)),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("method", NParamValue::Enum(v)) => {
                let method =
                    CombineMethod::from_str(&v).ok_or_else(|| format!("Invalid method: {}", v))?;
                self.strength = method.strength_desc().1;
                self.method = method;
            }
            ("strength", NParamValue::Float(v)) => self.strength = v,
            (k, v) => return Err(format!("Unknown parameter {} with value {:?}", k, v).into())
        }
        Ok(())
    }

    /// GPU per chunk when the user allows it; CPU otherwise (see `Node::process`'s doc comment).
    fn process(
        &self,
        pool: &Arc<TilePool>,
        inputs: &[TileHandle],
        ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        if ctx.chunk.is_none() || ctx.compute_target == ComputeTarget::Cpu {
            return Ok(vec![self.process_tile(pool, inputs)]);
        }

        let mut output = pool.allocate();
        let size = output.size() as u32;
        let params = CombineParams {
            strength: [self.strength, 0.0, 0.0, 0.0],
            method: [self.method.gpu_id(), size, 0, 0]
        };
        let workgroups = size.div_ceil(WORKGROUP_SIZE);
        let result = gpu::dispatch_f32(
            "combine",
            SHADER,
            &params,
            &[&inputs[0][..], &inputs[1][..]],
            (size * size) as usize,
            (workgroups, workgroups, 1)
        )?;
        output.copy_from_slice(&result);
        Ok(vec![Arc::new(output)])
    }

    fn clone_boxed(&self) -> Box<dyn Node> {
        Box::new(self.clone())
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
