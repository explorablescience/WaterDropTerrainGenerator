use std::sync::{Arc, OnceLock};

use rayon::prelude::*;
use rfd::FileDialog;

use crate::core::node::{
    NParamDesc, NParamValue, Node, NodeCategory, NodeDescriptor, NodeError, NodeIcon, NodePortType,
    NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-load",
    png_bytes: include_bytes!("../../../../assets/icons/node_load.png")
};

/// Kept in memory so `process` can resample it at whatever resolution the graph requests without touching the filesystem again.
#[derive(Debug, Clone)]
struct LoadedImage {
    data: Vec<f32>,
    width: u32,
    height: u32
}

/// Loads a heightmap from disk as a PNG file and outputs it as a single tile
#[derive(Debug, Default)]
pub struct LoadFile {
    file_path: String,
    loaded: Option<LoadedImage>
}
impl LoadFile {
    fn params() -> &'static [NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        SPECS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "file_path",
                    label: "File Path",
                    category: "Import",
                    default: NParamValue::String(String::new()),
                    constraints: None
                },
                NParamDesc {
                    key: "browse",
                    label: "Browse File...",
                    category: "Import",
                    default: NParamValue::Action {
                        show_success_message: false
                    },
                    constraints: None
                },
                NParamDesc {
                    key: "load",
                    label: "Load Heightmap",
                    category: "Import",
                    default: NParamValue::Action {
                        show_success_message: true
                    },
                    constraints: None
                },
            ]
        })
    }

    fn load_from_disk(&mut self) -> Result<(), NodeError> {
        if self.file_path.trim().is_empty() {
            return Err("File path is empty".into());
        }

        let image = image::open(&self.file_path)
            .map_err(|e| format!("Failed to load '{}': {}", self.file_path, e))?
            .into_luma8();
        let (width, height) = image.dimensions();
        let data = image.into_raw().iter().map(|&v| v as f32 / 255.0).collect();

        self.loaded = Some(LoadedImage {
            data,
            width,
            height
        });
        Ok(())
    }

    /// Bilinearly samples the loaded image at normalized coordinates `u, v` in `[0, 1]`.
    fn sample(image: &LoadedImage, u: f32, v: f32) -> f32 {
        let px = |x: u32, y: u32| image.data[(y * image.width + x) as usize];

        let fx = (u * image.width as f32 - 0.5).clamp(0.0, (image.width - 1) as f32);
        let fy = (v * image.height as f32 - 0.5).clamp(0.0, (image.height - 1) as f32);
        let x0 = fx as u32;
        let y0 = fy as u32;
        let x1 = (x0 + 1).min(image.width - 1);
        let y1 = (y0 + 1).min(image.height - 1);
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;

        let top = px(x0, y0) * (1.0 - tx) + px(x1, y0) * tx;
        let bottom = px(x0, y1) * (1.0 - tx) + px(x1, y1) * tx;
        top * (1.0 - ty) + bottom * ty
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
            "file_path" => Some(NParamValue::String(self.file_path.clone())),
            "browse" => Some(NParamValue::Action {
                show_success_message: false
            }),
            "load" => Some(NParamValue::Action {
                show_success_message: true
            }),
            _ => None
        }
    }
    fn set_param(&mut self, key: &str, value: NParamValue) -> Result<(), NodeError> {
        match (key, value) {
            ("file_path", NParamValue::String(v)) => self.file_path = v,
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
                let mut dialog = FileDialog::new().add_filter("PNG heightmap", &["png"]);
                if let Some(dir) = std::path::Path::new(&self.file_path).parent()
                    && !dir.as_os_str().is_empty()
                {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(file) = dialog.pick_file() {
                    self.file_path = file.display().to_string();
                }
                Ok(())
            }
            "load" => self.load_from_disk(),
            _ => Err(format!("Unknown action '{}'", key).into())
        }
    }

    fn process(
        &self,
        pool: &Arc<TilePool>,
        _inputs: &[TileHandle],
        ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        let Some(image) = &self.loaded else {
            return Err(NodeError::ProcessingFailed(
                "No heightmap loaded - browse to a PNG file and click 'Load Heightmap'".to_string()
            ));
        };

        // The loaded image covers the whole terrain, so each chunk samples its own footprint of it.
        let mut output = pool.allocate();
        let s = output.size();
        output.par_chunks_mut(s).enumerate().for_each(|(y, row)| {
            for (x, texel) in row.iter_mut().enumerate() {
                let (u, v) = ctx.normalize(ctx.world_pos(x, y));
                *texel = Self::sample(image, u, v);
            }
        });
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
