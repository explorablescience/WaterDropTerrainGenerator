use std::sync::Arc;

use crate::core::node::{Node, NodeCategory, NodeIcon, NodePortType, NodeSocket};
use crate::core::node_error::NodeError;
use crate::core::node_registry::NodeDescriptor;
use crate::core::tile_allocator::{TileHandle, TilePool};

/// A node that generates a flat terrain tile.
#[derive(Debug, Default)]
pub struct NodeGeneratorFlat;
impl Node for NodeGeneratorFlat {
    fn label(&self) -> &str {
        "Generator Flat Terrain"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::Generator
    }
    fn icon(&self) -> NodeIcon {
        NodeIcon::Plane
    }

    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "Height",
            dtype: NodePortType::Height
        }]
    }

    fn process(
        &self,
        pool: &Arc<TilePool>,
        _inputs: &[TileHandle]
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![Arc::new(pool.allocate())])
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "Flat Generator",
        category: NodeCategory::Generator,
        icon: NodeIcon::Plane,
        factory: || Box::new(NodeGeneratorFlat)
    }
}
