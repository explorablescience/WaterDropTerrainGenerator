use std::fmt;
use std::sync::Arc;

use crate::core::allocator::{TileHandle, TilePool};

/// Represents a node in the node graph, which can have input and output sockets for connecting to other nodes.
/// It is responsible for processing data and producing output based on its inputs.
pub trait Node: Send + Sync {
    fn name(&self) -> &str;
    fn inputs(&self) -> &[NodeSocket];
    fn outputs(&self) -> &[NodeSocket];

    /// Processes the node's inputs and produces its outputs, allocating any new tiles from `pool`.
    fn process(&self, pool: &Arc<TilePool>, inputs: &[TileHandle]) -> Result<Vec<TileHandle>, NodeError>;
}

/// NodeSocket represents an input or output socket of a node, which can be connected to other nodes.
pub struct NodeSocket {
    pub name: &'static str,
    pub dtype: PortType,
}
/// PortType represents the type of data that can be passed through a node's input or output socket.
pub enum PortType {
    Height, // Scalar heightfield (f32 per texel)
    Mask,   // Scalar mask (f32 per texel) - Same as Height, but used for masks
    Color,  // RGBA texture
    Vector, // Vector field (f32x3 per texel)
    Scalar, // Scalar value (f32) - Used for parameters, not textures
}

/// An error produced while processing a [`Node`].
#[derive(Debug)]
pub struct NodeError(pub String);
impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for NodeError {}
impl From<&str> for NodeError {
    fn from(msg: &str) -> Self {
        Self(msg.to_string())
    }
}
impl From<String> for NodeError {
    fn from(msg: String) -> Self {
        Self(msg)
    }
}
