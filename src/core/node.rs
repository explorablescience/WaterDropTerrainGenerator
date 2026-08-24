use std::fmt;
use std::fmt::Debug;
use std::sync::Arc;

use crate::core::{
    graph::GraphNodeId,
    node_parameters::{NParamDesc, NParamValue},
    tile_allocator::{TileHandle, TilePool}
};

/// Represents a node in the node graph, which can have input and output sockets for connecting to other nodes.
/// It is responsible for processing data and producing output based on its inputs.
pub trait Node: Debug + Send + Sync {
    fn label(&self) -> &str;

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
    fn set_param(&mut self, _key: &str, _value: NParamValue) -> Result<(), String> {
        Err("Parameter not found".into())
    }

    /// Processes the node's inputs and produces its outputs, allocating any new tiles from `pool`.
    fn process(
        &self,
        pool: &Arc<TilePool>,
        inputs: &[TileHandle]
    ) -> Result<Vec<TileHandle>, NodeError>;
}

/// NodeSocket represents an input or output socket of a node, which can be connected to other nodes.
pub struct NodeSocket {
    pub name: &'static str,
    pub dtype: NodePortType
}
/// PortType represents the type of data that can be passed through a node's input or output socket.
pub enum NodePortType {
    Height, // Scalar heightfield (f32 per texel)
    Mask,   // Scalar mask (f32 per texel) - Same as Height, but used for masks
    Color,  // RGBA texture
    Vector, // Vector field (f32x3 per texel)
    Scalar  // Scalar value (f32) - Used for parameters, not textures
}

/// An error produced while working with a [`Node`] or the [`NodeGraph`](crate::core::graph::NodeGraph) it belongs to.
#[derive(Debug)]
pub enum NodeError {
    /// No node exists with this id (it was never added, or has since been removed).
    NodeNotFound(GraphNodeId),
    /// The node has no output socket at this index.
    OutputSocketNotFound { node: String, socket: usize },
    /// The node has no input socket at this index.
    InputSocketNotFound { node: String, socket: usize },
    /// The output and input sockets being connected carry different data types.
    SocketTypeMismatch {
        from_node: String,
        from_socket: usize,
        to_node: String,
        to_socket: usize
    },
    /// No connection exists between the given output and input sockets.
    NotConnected {
        from_node: GraphNodeId,
        from_socket: usize,
        to_node: GraphNodeId,
        to_socket: usize
    },
    /// An input socket of the node has no incoming connection.
    InputNotConnected { node: String, socket: usize },
    /// The dependency graph contains a cycle, so it cannot be topologically sorted.
    CyclicGraph,
    /// A connected input socket's source node has not produced its output yet (internal
    /// scheduling bug: the topological order should guarantee this never happens).
    OutputNotAvailable { node: String },
    /// The requested node was never evaluated, so it has no output (internal scheduling bug).
    NodeNotEvaluated(GraphNodeId),
    /// A node failed while processing its inputs, with a node-specific message.
    ProcessingFailed(String)
}
impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NodeNotFound(id) => write!(f, "Unknown node id {:?}", id),
            Self::OutputSocketNotFound { node, socket } => {
                write!(f, "{} has no output socket {}", node, socket)
            }
            Self::InputSocketNotFound { node, socket } => {
                write!(f, "{} has no input socket {}", node, socket)
            }
            Self::SocketTypeMismatch {
                from_node,
                from_socket,
                to_node,
                to_socket
            } => write!(
                f,
                "Cannot connect {}:{} to {}:{}, socket types differ",
                from_node, from_socket, to_node, to_socket
            ),
            Self::NotConnected {
                from_node,
                from_socket,
                to_node,
                to_socket
            } => write!(
                f,
                "No connection from {:?}:{} to {:?}:{}",
                from_node, from_socket, to_node, to_socket
            ),
            Self::InputNotConnected { node, socket } => {
                write!(f, "{} input {} is not connected", node, socket)
            }
            Self::CyclicGraph => write!(f, "Graph contains a cycle"),
            Self::OutputNotAvailable { node } => {
                write!(f, "No output available to feed {}", node)
            }
            Self::NodeNotEvaluated(id) => write!(f, "{:?} was not evaluated", id),
            Self::ProcessingFailed(msg) => write!(f, "{}", msg)
        }
    }
}
impl std::error::Error for NodeError {}
impl From<&str> for NodeError {
    fn from(msg: &str) -> Self {
        Self::ProcessingFailed(msg.to_string())
    }
}
impl From<String> for NodeError {
    fn from(msg: String) -> Self {
        Self::ProcessingFailed(msg)
    }
}
