use std::sync::{Arc, OnceLock};

use rfd::FileDialog;

use crate::core::node::{
    NParamConstraints, NParamDesc, NParamValue, Node, NodeCategory, NodeDescriptor, NodeError,
    NodeIcon, NodeLocality, NodePortType, NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-save",
    png_bytes: include_bytes!("../../../assets/icons/node_save.png")
};

/// Splits its input into a `chunks` x `chunks` grid of `resolution`-sided tiles - independent of
/// the terrain's own chunk grid - and writes each as a 16-bit grayscale PNG into `folder_path`,
/// named `{file_prefix}_{x}_{y}.png`. `[min_height, max_height]` maps to the PNG's full `0..65535`
/// range; [`LoadFile`](crate::nodes::generation::LoadFile) reverses the same mapping on import.
#[derive(Debug)]
pub struct ExportFile {
    folder_path: String,
    file_prefix: String,
    chunks: u32,
    resolution: u32,
    min_height: f32,
    max_height: f32
}
impl Default for ExportFile {
    fn default() -> Self {
        Self {
            folder_path: String::new(),
            file_prefix: "heightmap".to_string(),
            chunks: 4,
            resolution: 512,
            min_height: 0.0,
            max_height: 10.0
        }
    }
}
impl ExportFile {
    fn params() -> &'static [NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        SPECS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "folder_path",
                    label: "Folder Path",
                    category: "Export",
                    default: NParamValue::String(String::new()),
                    constraints: None
                },
                NParamDesc {
                    key: "browse",
                    label: "Browse Folder...",
                    category: "Export",
                    default: NParamValue::Action {
                        show_success_message: false
                    },
                    constraints: None
                },
                NParamDesc {
                    key: "file_prefix",
                    label: "File Prefix",
                    category: "Export",
                    default: NParamValue::String("heightmap".to_string()),
                    constraints: None
                },
                NParamDesc {
                    key: "chunks",
                    label: "Chunks",
                    category: "Tiling",
                    default: NParamValue::Int(4),
                    constraints: Some(NParamConstraints::IntRange { min: 1, max: 16 })
                },
                NParamDesc {
                    key: "resolution",
                    label: "Resolution",
                    category: "Tiling",
                    default: NParamValue::Int(512),
                    constraints: Some(NParamConstraints::IntRange { min: 16, max: 1024 })
                },
                NParamDesc {
                    key: "min_height",
                    label: "Min Height",
                    category: "Range",
                    default: NParamValue::Float(0.0),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: -1000.0,
                        max: 1000.0
                    })
                },
                NParamDesc {
                    key: "max_height",
                    label: "Max Height",
                    category: "Range",
                    default: NParamValue::Float(10.0),
                    constraints: Some(NParamConstraints::FloatRange {
                        min: -1000.0,
                        max: 1000.0
                    })
                },
                NParamDesc {
                    key: "export",
                    label: "Export Terrain",
                    category: "Export",
                    default: NParamValue::Action {
                        show_success_message: true
                    },
                    constraints: None
                },
            ]
        })
    }
}
impl Node for ExportFile {
    fn label(&self) -> &str {
        "Export"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::Export
    }
    fn icon(&self) -> NodeIcon {
        ICON
    }

    fn locality(&self) -> NodeLocality {
        NodeLocality::Global {
            native_resolution: (self.chunks * self.resolution) as usize
        }
    }
    fn inputs(&self) -> &[NodeSocket] {
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
            "folder_path" => Some(NParamValue::String(self.folder_path.clone())),
            "file_prefix" => Some(NParamValue::String(self.file_prefix.clone())),
            "chunks" => Some(NParamValue::Int(self.chunks as i32)),
            "resolution" => Some(NParamValue::Int(self.resolution as i32)),
            "min_height" => Some(NParamValue::Float(self.min_height)),
            "max_height" => Some(NParamValue::Float(self.max_height)),
            "browse" => Some(NParamValue::Action {
                show_success_message: false
            }),
            "export" => Some(NParamValue::Action {
                show_success_message: true
            }),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("folder_path", NParamValue::String(v)) => self.folder_path = v,
            ("file_prefix", NParamValue::String(v)) => self.file_prefix = v,
            ("chunks", NParamValue::Int(v)) => self.chunks = v as u32,
            ("resolution", NParamValue::Int(v)) => self.resolution = v as u32,
            ("min_height", NParamValue::Float(v)) => self.min_height = v,
            ("max_height", NParamValue::Float(v)) => self.max_height = v,
            (k, v) => return Err(format!("Unknown parameter {} with value {:?}", k, v).into())
        }
        Ok(())
    }

    /// A `Global` node's own pass only needs to hand the already-resampled input tile back to
    /// [`Self::on_action`] (via the `output` it's given) - the actual disk write is a side effect
    /// triggered by the "Export Terrain" button, not part of the (cached, pure) evaluation.
    fn process(
        &self,
        _pool: &Arc<TilePool>,
        inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![inputs[0].clone()])
    }

    fn on_action(
        &mut self,
        key: &str,
        output: &[TileHandle],
        _output_size: usize
    ) -> Result<(), NodeError> {
        match key {
            "browse" => {
                let mut dialog = FileDialog::new();
                if !self.folder_path.is_empty() {
                    dialog = dialog.set_directory(&self.folder_path);
                }
                if let Some(dir) = dialog.pick_folder() {
                    self.folder_path = dir.display().to_string();
                }
                Ok(())
            }
            "export" => self.export(output),
            _ => Err(format!("Unknown action '{}'", key).into())
        }
    }
}
impl ExportFile {
    fn export(&self, output: &[TileHandle]) -> Result<(), NodeError> {
        if self.folder_path.is_empty() {
            return Err("No folder selected".into());
        }
        let Some(tile) = output.first() else {
            return Err("No data to export".into());
        };

        let side = self.chunks as usize * self.resolution as usize;
        if tile.size() != side {
            return Err(format!(
                "Expected a {0}x{0} tile but got {1}x{1} - try running Export again",
                side,
                tile.size()
            )
            .into());
        }

        let folder = std::path::Path::new(&self.folder_path);
        std::fs::create_dir_all(folder)
            .map_err(|e| format!("Failed to create folder {}: {}", self.folder_path, e))?;

        let range = (self.max_height - self.min_height).max(f32::EPSILON);
        let resolution = self.resolution as usize;
        for cy in 0..self.chunks as usize {
            for cx in 0..self.chunks as usize {
                let mut pixels = Vec::with_capacity(resolution * resolution);
                for y in 0..resolution {
                    let row = (cy * resolution + y) * side + cx * resolution;
                    for x in 0..resolution {
                        let height = tile[row + x];
                        let t = ((height - self.min_height) / range).clamp(0.0, 1.0);
                        pixels.push((t * u16::MAX as f32).round() as u16);
                    }
                }

                let image = image::ImageBuffer::<image::Luma<u16>, _>::from_raw(
                    resolution as u32,
                    resolution as u32,
                    pixels
                )
                .expect("pixel buffer is exactly resolution x resolution");
                let path = folder.join(format!("{}_{}_{}.png", self.file_prefix, cx, cy));
                image
                    .save(&path)
                    .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
            }
        }

        Ok(())
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "Export",
        category: NodeCategory::Export,
        subcategory: "Production Export",
        icon: ICON,
        factory: || Box::new(ExportFile::default())
    }
}
