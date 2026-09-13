use std::sync::Arc;

use rayon::prelude::*;

use crate::core::*;

const ICON: NodeIcon = NodeIcon {
    id: "node-satmap",
    png_bytes: include_bytes!("../../../assets/icons/node_satmap.png")
};

/// A gradient stop: `t` in `[0, 1]` and its RGB color, also in `[0, 1]`. Stops within a preset
/// must be sorted ascending by `t`.
type Stop = (f32, [f32; 3]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
enum SatMapPreset {
    #[default]
    Grayscale,
    Desert,
    Alpine,
    Tundra,
    Volcanic,
    TropicalIslands,
    Arctic
}
impl SatMapPreset {
    fn to_str(self) -> &'static str {
        match self {
            SatMapPreset::Grayscale => "Grayscale",
            SatMapPreset::Desert => "Desert",
            SatMapPreset::Alpine => "Alpine",
            SatMapPreset::Tundra => "Tundra",
            SatMapPreset::Volcanic => "Volcanic",
            SatMapPreset::TropicalIslands => "Tropical Islands",
            SatMapPreset::Arctic => "Arctic"
        }
    }
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "Grayscale" => Some(SatMapPreset::Grayscale),
            "Desert" => Some(SatMapPreset::Desert),
            "Alpine" => Some(SatMapPreset::Alpine),
            "Tundra" => Some(SatMapPreset::Tundra),
            "Volcanic" => Some(SatMapPreset::Volcanic),
            "Tropical Islands" => Some(SatMapPreset::TropicalIslands),
            "Arctic" => Some(SatMapPreset::Arctic),
            _ => None
        }
    }
    fn all_options() -> Vec<&'static str> {
        vec![
            "Grayscale",
            "Desert",
            "Alpine",
            "Tundra",
            "Volcanic",
            "Tropical Islands",
            "Arctic",
        ]
    }

    fn stops(self) -> &'static [Stop] {
        match self {
            SatMapPreset::Grayscale => &[(0.00, [0.00, 0.00, 0.00]), (1.00, [1.00, 1.00, 1.00])],
            SatMapPreset::Desert => &[
                (0.00, [0.10, 0.07, 0.05]),
                (0.15, [0.35, 0.20, 0.10]),
                (0.35, [0.65, 0.45, 0.25]),
                (0.55, [0.80, 0.65, 0.40]),
                (0.75, [0.55, 0.40, 0.30]),
                (1.00, [0.95, 0.92, 0.85])
            ],
            SatMapPreset::Alpine => &[
                (0.00, [0.05, 0.15, 0.05]),
                (0.25, [0.15, 0.35, 0.12]),
                (0.45, [0.35, 0.45, 0.20]),
                (0.65, [0.45, 0.40, 0.35]),
                (0.80, [0.55, 0.55, 0.55]),
                (0.92, [0.85, 0.85, 0.88]),
                (1.00, [1.00, 1.00, 1.00])
            ],
            SatMapPreset::Tundra => &[
                (0.00, [0.25, 0.30, 0.28]),
                (0.30, [0.40, 0.42, 0.35]),
                (0.55, [0.55, 0.50, 0.45]),
                (0.75, [0.70, 0.70, 0.68]),
                (0.90, [0.88, 0.90, 0.90]),
                (1.00, [1.00, 1.00, 1.00])
            ],
            SatMapPreset::Volcanic => &[
                (0.00, [0.02, 0.02, 0.02]),
                (0.20, [0.10, 0.05, 0.05]),
                (0.40, [0.30, 0.08, 0.05]),
                (0.55, [0.55, 0.15, 0.05]),
                (0.70, [0.80, 0.35, 0.05]),
                (0.85, [0.95, 0.60, 0.10]),
                (1.00, [1.00, 0.90, 0.40])
            ],
            SatMapPreset::TropicalIslands => &[
                (0.00, [0.02, 0.15, 0.35]),
                (0.15, [0.05, 0.35, 0.55]),
                (0.25, [0.15, 0.55, 0.65]),
                (0.30, [0.90, 0.85, 0.65]),
                (0.40, [0.55, 0.70, 0.30]),
                (0.60, [0.20, 0.50, 0.20]),
                (0.80, [0.15, 0.35, 0.15]),
                (1.00, [0.45, 0.40, 0.35])
            ],
            SatMapPreset::Arctic => &[
                (0.00, [0.05, 0.10, 0.20]),
                (0.20, [0.30, 0.45, 0.55]),
                (0.40, [0.70, 0.80, 0.85]),
                (0.60, [0.85, 0.90, 0.93]),
                (0.80, [0.95, 0.96, 0.98]),
                (1.00, [1.00, 1.00, 1.00])
            ]
        }
    }
}

/// Piecewise-linear sample of a sorted gradient at `t` (clamped to the gradient's own range).
fn sample_gradient(stops: &[Stop], t: f32) -> [f32; 3] {
    let (first, last) = (stops[0], stops[stops.len() - 1]);
    if t <= first.0 {
        return first.1;
    }
    if t >= last.0 {
        return last.1;
    }
    for pair in stops.windows(2) {
        let (t0, c0) = pair[0];
        let (t1, c1) = pair[1];
        if t <= t1 {
            let f = (t - t0) / (t1 - t0);
            return [
                c0[0] + (c1[0] - c0[0]) * f,
                c0[1] + (c1[1] - c0[1]) * f,
                c0[2] + (c1[2] - c0[2]) * f
            ];
        }
    }
    last.1
}

/// Colorizes a heightmap into RGB using a chosen preset gradient (à la Gaea's SatMap), remapping
/// `[min_height, max_height]` onto the gradient's `[0, 1]` before sampling.
#[derive(Debug, Clone)]
pub struct SatMap {
    preset: SatMapPreset,
    min_height: f32,
    max_height: f32
}
impl Default for SatMap {
    fn default() -> Self {
        Self {
            preset: SatMapPreset::default(),
            min_height: 0.0,
            max_height: 1.0
        }
    }
}
impl SatMap {
    fn desc_params() -> &'static [NParamDesc] {
        static PARAMS: std::sync::OnceLock<Vec<NParamDesc>> = std::sync::OnceLock::new();
        PARAMS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "preset",
                    label: "Colormap",
                    category: "Coloring",
                    default: NParamValue::Enum(SatMapPreset::default().to_str().to_string()),
                    constraints: Some(NParamConstraints::EnumOneOf {
                        options: SatMapPreset::all_options()
                    })
                },
                NParamDesc {
                    key: "min_height",
                    label: "Min Height",
                    category: "Coloring",
                    default: NParamValue::Float(0.0),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: -50.0,
                        max: 50.0
                    })
                },
                NParamDesc {
                    key: "max_height",
                    label: "Max Height",
                    category: "Coloring",
                    default: NParamValue::Float(1.0),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: -50.0,
                        max: 50.0
                    })
                },
            ]
        })
    }

    fn process_tile(&self, pool: &Arc<TilePool>, inputs: &[TileHandle]) -> TileHandle {
        let mut color = pool.allocate_channels(3);
        let height = &inputs[0];
        let stops = self.preset.stops();
        let range = (self.max_height - self.min_height).max(f32::EPSILON);
        let min_height = self.min_height;

        for c in 0..3 {
            color
                .plane_mut(c)
                .par_iter_mut()
                .enumerate()
                .for_each(|(idx, texel)| {
                    let t = ((height[idx] - min_height) / range).clamp(0.0, 1.0);
                    *texel = sample_gradient(stops, t)[c];
                });
        }

        Arc::new(color)
    }
}
impl Node for SatMap {
    fn label(&self) -> &str {
        "SatMap"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::Texturing
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
            name: "Color",
            dtype: SocketDtype::Fixed(NodePortType::Color),
            required: true
        }]
    }

    fn desc_params(&self) -> &'static [NParamDesc] {
        Self::desc_params()
    }
    fn get_param(&self, key: &str) -> Option<NParamValue> {
        match key {
            "preset" => Some(NParamValue::Enum(self.preset.to_str().to_string())),
            "min_height" => Some(NParamValue::Float(self.min_height)),
            "max_height" => Some(NParamValue::Float(self.max_height)),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("preset", NParamValue::Enum(v)) => {
                self.preset =
                    SatMapPreset::from_str(&v).ok_or_else(|| format!("Invalid preset: {}", v))?;
            }
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
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![self.process_tile(pool, inputs)])
    }

    fn clone_boxed(&self) -> Box<dyn Node> {
        Box::new(self.clone())
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "SatMap",
        category: NodeCategory::Texturing,
        subcategory: "Color Maps",
        icon: ICON,
        factory: || Box::new(SatMap::default())
    }
}
