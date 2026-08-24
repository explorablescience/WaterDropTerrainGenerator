use std::hash::{Hash, Hasher};
use std::{hash::DefaultHasher};
use std::fmt::Debug;
use std::sync::Arc;

use crate::core::node_error::NodeError;
use crate::core::{
    node_parameters::{NParamDesc, NParamValue}, tile_allocator::{TileHandle, TilePool}
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
    pub dtype: NodePortType
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
