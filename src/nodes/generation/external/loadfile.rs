use std::sync::{Arc, OnceLock};

use rfd::FileDialog;

use crate::core::node::{
    NParamConstraints, NParamDesc, NParamValue, Node, NodeCategory, NodeDescriptor, NodeError,
    NodeIcon, NodeLocality, NodePortType, NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-load",
    png_bytes: include_bytes!("../../../../assets/icons/node_load.png")
};

/// Reverses [`ExportFile`](crate::nodes::export::ExportFile): reads a `chunks` x `chunks` grid of
/// `resolution`-sided 16-bit grayscale PNGs named `{file_prefix}_{x}_{y}.png` from `folder_path`,
/// stitches them back into a single tile, and maps `0..65535` back to `[min_height, max_height]`.
#[derive(Debug)]
pub struct LoadFile {
    folder_path: String,
    file_prefix: String,
    chunks: u32,
    resolution: u32,
    min_height: f32,
    max_height: f32
}
impl Default for LoadFile {
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
impl LoadFile {
    fn params() -> &'static [NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        SPECS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "folder_path",
                    label: "Folder Path",
                    category: "Import",
                    default: NParamValue::String(String::new()),
                    constraints: None
                },
                NParamDesc {
                    key: "browse",
                    label: "Browse Folder...",
                    category: "Import",
                    default: NParamValue::Action {
                        show_success_message: false
                    },
                    constraints: None
                },
                NParamDesc {
                    key: "file_prefix",
                    label: "File Prefix",
                    category: "Import",
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
            ]
        })
    }
}
impl Node for LoadFile {
    fn label(&self) -> &str {
        "Load File"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::Generation
    }
    fn icon(&self) -> NodeIcon {
        ICON
    }

    fn locality(&self) -> NodeLocality {
        NodeLocality::Global {
            native_resolution: (self.chunks * self.resolution) as usize
        }
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
            "folder_path" => Some(NParamValue::String(self.folder_path.clone())),
            "file_prefix" => Some(NParamValue::String(self.file_prefix.clone())),
            "chunks" => Some(NParamValue::Int(self.chunks as i32)),
            "resolution" => Some(NParamValue::Int(self.resolution as i32)),
            "min_height" => Some(NParamValue::Float(self.min_height)),
            "max_height" => Some(NParamValue::Float(self.max_height)),
            "browse" => Some(NParamValue::Action {
                show_success_message: false
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

    fn on_action(
        &mut self,
        key: &str,
        _output: &[TileHandle],
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
            _ => Err(format!("Unknown action '{}'", key).into())
        }
    }

    /// Unlike most nodes, the actual work (disk reads) happens here rather than behind an action
    /// button - `LoadFile` has no input to pass through, so its whole-terrain tile only exists
    /// once this reads it off disk. Cached like any other node: re-runs only when a param changes.
    fn process(
        &self,
        pool: &Arc<TilePool>,
        _inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        if self.folder_path.is_empty() {
            return Err("No folder selected".into());
        }

        let folder = std::path::Path::new(&self.folder_path);
        let resolution = self.resolution as usize;
        let side = self.chunks as usize * resolution;
        let range = self.max_height - self.min_height;

        let mut output = pool.allocate();
        for cy in 0..self.chunks as usize {
            for cx in 0..self.chunks as usize {
                let path = folder.join(format!("{}_{}_{}.png", self.file_prefix, cx, cy));
                let image = image::open(&path)
                    .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?
                    .into_luma16();
                if image.width() as usize != resolution || image.height() as usize != resolution {
                    let (width, height) = (image.width(), image.height());
                    return Err(format!(
                        "{} is {width}x{height}, expected {resolution}x{resolution}",
                        path.display()
                    )
                    .into());
                }

                for y in 0..resolution {
                    let row = (cy * resolution + y) * side + cx * resolution;
                    for x in 0..resolution {
                        let t = image.get_pixel(x as u32, y as u32).0[0] as f32 / u16::MAX as f32;
                        output[row + x] = self.min_height + t * range;
                    }
                }
            }
        }

        Ok(vec![Arc::new(output)])
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "Load File",
        category: NodeCategory::Generation,
        subcategory: "External",
        icon: ICON,
        factory: || Box::new(LoadFile::default())
    }
}
