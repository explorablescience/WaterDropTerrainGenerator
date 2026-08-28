//! What a node is: the `Node` trait itself, its socket/parameter/error/message types, and the
//! registry that lets the graph editor discover every node type without knowing about them by name.

use std::fmt::Debug;
use std::hash::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::core::tiling::{TileContext, TileHandle, TilePool};

mod error;
mod message;
mod parameters;
mod registry;

pub use error::NodeError;
pub use message::{
    MessageLifetime, NodeMessage, NodeMessageLog, NodeMessageSeverity, TimedNodeMessage
};
pub use parameters::{NParamConstraints, NParamDesc, NParamValidator, NParamValue};
pub use registry::{NodeDescriptor, registered_nodes};

pub trait Node: Debug + Send + Sync {
    fn label(&self) -> &str;

    /// Drives the color the graph editor uses for the node's outline, title, pins and icon.
    fn category(&self) -> NodeCategory;
    /// A stable id (used as the egui image cache key) paired with the node's PNG icon bytes.
    fn icon(&self) -> NodeIcon;

    /// Kernel size in texels; used to determine padding.
    fn size(&self) -> usize {
        0
    }

    /// Defaults to `Local`, which covers most nodes: they can be computed independently for each chunk.
    fn locality(&self) -> NodeLocality {
        NodeLocality::Local
    }
    fn inputs(&self) -> &[NodeSocket] {
        &[]
    }
    fn outputs(&self) -> &[NodeSocket] {
        &[]
    }

    fn desc_params(&self) -> &[NParamDesc] {
        &[]
    }
    fn get_param(&self, _key: &str) -> Option<NParamValue> {
        None
    }
    fn set_param(&mut self, _key: &str, _value: NParamValue) -> Result<(), NodeError> {
        Err("Parameter not found".into())
    }

    /// Called when an `NParamValue::Action` button is pressed in the node's UI; `key` identifies which one.
    fn on_action(
        &mut self,
        _key: &str,
        _output: &[TileHandle],
        _output_size: usize
    ) -> Result<(), NodeError> {
        Err("Action not supported".into())
    }

    /// `ctx` describes where in the terrain this call is computing - only position-aware nodes need to use it.
    fn process(
        &self,
        _pool: &Arc<TilePool>,
        _inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![])
    }

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

pub struct NodeSocket {
    pub name: &'static str,
    pub dtype: NodePortType,
    pub required: bool
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodePortType {
    Height, // Scalar heightfield (f32 per texel)
    Mask,   // Same as Height, but used for masks
    Color,  // RGBA texture
    Vector, // Vector field (f32x3 per texel)
    Scalar  // Scalar value (f32) - used for parameters, not textures
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeLocality {
    /// Given just that chunk's (padded) tile and a world-space coordinate frame to sample consistently across chunk borders.
    Local,
    /// Evaluated once over the terrain's whole real-world extent (see
    /// [`ChunkGrid::world_extent`](crate::core::tiling::ChunkGrid::world_extent)) at
    /// `native_resolution` texels per axis, instead of per chunk - for operations whose effect
    /// isn't bounded by any fixed kernel radius (e.g. erosion transport), so no per-chunk padding
    /// could express it. Any ancestor or descendant at a different resolution (or `Local` scope)
    /// is bilinearly resampled into/out of this frame automatically by the graph engine.
    Global { native_resolution: usize }
}

/// High-level grouping of nodes, used to color-code and organize nodes in the graph editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeCategory {
    Generation,
    Modification,
    Surface,
    Simulation,
    DataExtraction,
    Texturing,
    Utility,
    Export
}
impl NodeCategory {
    /// Every category, in the order they should be listed in the "Add Node" menu.
    pub const ALL: [NodeCategory; 8] = [
        NodeCategory::Generation,
        NodeCategory::Modification,
        NodeCategory::Surface,
        NodeCategory::Simulation,
        NodeCategory::DataExtraction,
        NodeCategory::Texturing,
        NodeCategory::Utility,
        NodeCategory::Export
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            NodeCategory::Generation => "Generation",
            NodeCategory::Modification => "Modification",
            NodeCategory::Surface => "Surface",
            NodeCategory::Simulation => "Simulation",
            NodeCategory::DataExtraction => "Data Extraction",
            NodeCategory::Texturing => "Texturing",
            NodeCategory::Utility => "Utility",
            NodeCategory::Export => "Export"
        }
    }

    /// Every subcategory within this category.
    pub fn subcategories(&self) -> &'static [&'static str] {
        match self {
            NodeCategory::Generation => &[
                "External",
                "Mathematical",
                "Geometric",
                "Primitives",
                "Landscape"
            ],
            NodeCategory::Modification => &[
                "Transforms",
                "Morphing",
                "Height Adjustments",
                "Remapping",
                "Smoothing",
                "Stylized FX"
            ],
            NodeCategory::Surface => &[
                "Rock Formations",
                "Terracing",
                "Micro Structures",
                "Surface Texture",
                "Instance Scatter"
            ],
            NodeCategory::Simulation => {
                &["Erosion", "Hydrology", "Snow & Ice", "Ecological Scatter"]
            }
            NodeCategory::DataExtraction => &["Topographic Analysis", "Texture Masks"],
            NodeCategory::Texturing => &["Color Maps", "Color Blends"],
            NodeCategory::Utility => &["Compositing", "Data Operations", "Logic"],
            NodeCategory::Export => &["Production Export"]
        }
    }
}

/// The image is expected to be a white glyph on a transparent background so the UI layer can tint it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeIcon {
    pub id: &'static str,
    pub png_bytes: &'static [u8]
}
