use std::fmt::Debug;
use std::hash::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::core::node_error::NodeError;
use crate::core::{
    node_parameters::{NParamDesc, NParamValue},
    tile_allocator::{TileHandle, TilePool}
};

/// Represents a node in the node graph, which can have input and output sockets for connecting to other nodes.
/// It is responsible for processing data and producing output based on its inputs.
pub trait Node: Debug + Send + Sync {
    fn label(&self) -> &str;

    /// The category this node belongs to. Drives the color the graph editor uses for the node's
    /// outline, title, pins and icon.
    fn category(&self) -> NodeCategory;
    /// The logo shown next to this node's title, hinting at what the node does: a stable id
    /// (used as the egui image cache key) paired with the node's PNG icon bytes.
    fn icon(&self) -> NodeIcon;

    /// Size of the kernel (in texels) that this node operates on. Used to determine padding.
    fn size(&self) -> usize {
        0
    }
    fn inputs(&self) -> &[NodeSocket] {
        &[]
    }
    fn outputs(&self) -> &[NodeSocket] {
        &[]
    }

    /// Returns a slice of parameter descriptions for this node.
    fn desc_params(&self) -> &[NParamDesc] {
        &[]
    }
    /// Gets the value of a parameter by its key. Returns `None` if the parameter does not exist.
    fn get_param(&self, _key: &str) -> Option<NParamValue> {
        None
    }
    /// Sets the value of a parameter by its key. Returns an error if the parameter does not exist or if the value is invalid.
    fn set_param(&mut self, _key: &str, _value: NParamValue) -> Result<(), NodeError> {
        Err("Parameter not found".into())
    }

    /// Called when a button is pressed in the node's UI (that is an `NParamValue::Action`). The `key` identifies which button was pressed.
    /// The `output` slice contains the node's output tiles, and `output_size` is the size of the output tiles. Returns an error if the action fails.
    fn on_action(
        &mut self,
        _key: &str,
        _output: &[TileHandle],
        _output_size: usize
    ) -> Result<(), NodeError> {
        Err("Action not supported".into())
    }

    /// Processes the node's inputs and produces its outputs, allocating any new tiles from `pool`.
    fn process(
        &self,
        _pool: &Arc<TilePool>,
        _inputs: &[TileHandle]
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![])
    }

    /// Returns a hash representing the current state of the node's parameters, used for caching.
    fn params_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        for param in self.desc_params() {
            param.hash(&mut hasher);
            if let Some(value) = self.get_param(param.key) {
                value.hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}

/// NodeSocket represents an input or output socket of a node, which can be connected to other nodes.
pub struct NodeSocket {
    pub name: &'static str,
    pub dtype: NodePortType,
    pub required: bool
}
/// PortType represents the type of data that can be passed through a node's input or output socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodePortType {
    Height, // Scalar heightfield (f32 per texel)
    Mask,   // Scalar mask (f32 per texel) - Same as Height, but used for masks
    Color,  // RGBA texture
    Vector, // Vector field (f32x3 per texel)
    Scalar  // Scalar value (f32) - Used for parameters, not textures
}

/// High-level grouping of nodes, used to color-code and organize nodes in the graph editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeCategory {
    Generator,
    Simulation,
    Io
}
impl NodeCategory {
    /// Every category, in the order they should be listed in the "Add Node" menu.
    pub const ALL: [NodeCategory; 3] =
        [NodeCategory::Generator, NodeCategory::Simulation, NodeCategory::Io];

    pub fn display_name(&self) -> &'static str {
        match self {
            NodeCategory::Generator => "Generator",
            NodeCategory::Simulation => "Simulation",
            NodeCategory::Io => "I/O"
        }
    }
}

/// Identifies the PNG logo drawn next to a node's title in the graph editor: a stable id used as
/// the egui image cache key, paired with the embedded PNG bytes. The image is expected to be a
/// white glyph on a transparent background, so the UI layer can tint it with the node's category
/// color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeIcon {
    pub id: &'static str,
    pub png_bytes: &'static [u8]
}
