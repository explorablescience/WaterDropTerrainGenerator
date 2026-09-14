//! What a node is: the `Node` trait itself, its socket/parameter/error/message types, and the
//! registry that lets the graph editor discover every node type without knowing about them by name.

use std::fmt::Debug;
use std::hash::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

mod error;
mod message;
mod parameters;
mod registry;

use crate::core::*;
pub use error::NodeError;
pub use message::{
    MessageLifetime, NodeMessage, NodeMessageLog, NodeMessageSeverity, TimedNodeMessage
};
pub use parameters::{NParamConstraints, NParamDesc, NParamValidator, NParamValue, ParamUnit};
pub use registry::{NodeDescriptor, registered_nodes};

/// A node is a single operation in the terrain graph, which can be connected to other nodes to form a directed graph (DAG) of terrain operations.
/// This is the core element of the terrain graph system, and is used to define the behavior of the graph editor and the terrain generation pipeline.
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

    /// If `Some`, an `Action` button with this `key` gets `on_action`'s `output` freshly evaluated
    /// from this node's socket-0 input at this resolution instead of the node's own current
    /// output - e.g. an export action that needs a resolution decoupled from how the node is
    /// normally (cheaply) previewed.
    fn action_resolution(&self, _key: &str) -> Option<usize> {
        None
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
    /// For GPU work, only dispatch (via [`crate::core::gpu::dispatch_f32`]) when `ctx.chunk.is_some() && ctx.compute_target == ComputeTarget::Gpu`; otherwise stay on CPU.
    fn process(
        &self,
        _pool: &Arc<TilePool>,
        _inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![])
    }

    /// An owned snapshot, for handing to a background chunk-processing task.
    fn clone_boxed(&self) -> Box<dyn Node>;

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

/// A node's input or output port, which can be connected to other nodes in the graph.
pub struct NodeSocket {
    pub name: &'static str,
    pub dtype: SocketDtype,
    pub required: bool
}

/// `Generic` resolves at connect-time from whatever's wired into the node's other `Generic`
/// sockets, which must all agree (see `Topology::connect`) - e.g. `Combine` works on any of
/// Height, Mask or Color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketDtype {
    Fixed(NodePortType),
    Generic
}

/// A node's input or output port type, which determines what kind of data can flow through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodePortType {
    Height, // Scalar heightfield (f32 per texel)
    Mask,   // Scalar mask (f32 per texel)
    Color   // RGB color (f32 per channel per texel)
}
impl NodePortType {
    pub fn channels(self) -> usize {
        match self {
            NodePortType::Height | NodePortType::Mask => 1,
            NodePortType::Color => 3
        }
    }
}

/// `Local` nodes can be computed independently for each chunk, while `Global` nodes need to be evaluated once over the terrain's whole real-world extent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeLocality {
    /// Given just that chunk's (padded) tile and a world-space coordinate frame to sample consistently across chunk borders.
    Local,
    /// Evaluated once over the terrain's whole real-world extent.
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
    Utility
}
impl NodeCategory {
    /// Every category, in the order they should be listed in the "Add Node" menu.
    pub const ALL: [NodeCategory; 7] = [
        NodeCategory::Generation,
        NodeCategory::Modification,
        NodeCategory::Surface,
        NodeCategory::Simulation,
        NodeCategory::DataExtraction,
        NodeCategory::Texturing,
        NodeCategory::Utility
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            NodeCategory::Generation => "Generation",
            NodeCategory::Modification => "Modification",
            NodeCategory::Surface => "Surface",
            NodeCategory::Simulation => "Simulation",
            NodeCategory::DataExtraction => "Data Extraction",
            NodeCategory::Texturing => "Texturing",
            NodeCategory::Utility => "Utility"
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
            NodeCategory::Utility => &["Compositing", "Data Operations", "Logic"]
        }
    }
}

/// The image is expected to be a white glyph on a transparent background so the UI layer can tint it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeIcon {
    pub id: &'static str,
    pub png_bytes: &'static [u8]
}
