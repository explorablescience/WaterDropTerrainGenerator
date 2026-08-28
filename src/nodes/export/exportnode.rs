use std::sync::OnceLock;

use rfd::FileDialog;

use crate::core::node::{
    NParamDesc, NParamValue, Node, NodeCategory, NodeDescriptor, NodeError, NodeIcon, NodePortType,
    NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-save",
    png_bytes: include_bytes!("../../../assets/icons/node_save.png")
};

/// Exports the whole, stitched-together terrain to disk as a PNG heightmap.
#[derive(Debug, Default)]
pub struct ExportFile {
    file_path: String
}
impl ExportFile {
    fn params() -> &'static [NParamDesc] {
        static SPECS: OnceLock<Vec<NParamDesc>> = OnceLock::new();
        SPECS.get_or_init(|| {
            vec![
                NParamDesc {
                    key: "file_path",
                    label: "File Path",
                    category: "Export",
                    default: NParamValue::String(String::new()),
                    constraints: None
                },
                NParamDesc {
                    key: "browse",
                    label: "Browse File...",
                    category: "Export",
                    default: NParamValue::Action {
                        show_success_message: false
                    },
                    constraints: None
                },
                NParamDesc {
                    key: "save",
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
            "file_path" => Some(NParamValue::String(self.file_path.clone())),
            "browse" => Some(NParamValue::Action {
                show_success_message: false
            }),
            "save" => Some(NParamValue::Action {
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

    fn process(
        &self,
        _pool: &std::sync::Arc<TilePool>,
        _inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        todo!()
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
                if let Some(file) = dialog.save_file() {
                    self.file_path = file.display().to_string();
                }
                Ok(())
            }
            _ => unreachable!("Unknown action key {}", key)
        }
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
