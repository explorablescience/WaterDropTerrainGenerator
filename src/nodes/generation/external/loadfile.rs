use std::sync::{Arc, OnceLock};

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

/// Loads a heightmap from disk as a PNG file and outputs it as a single tile
#[derive(Debug, Default)]
pub struct LoadFile {
    file_path: String
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
            "load" => todo!(),
            _ => Err(format!("Unknown action '{}'", key).into())
        }
    }

    fn process(
        &self,
        _pool: &Arc<TilePool>,
        _inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        todo!()
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
