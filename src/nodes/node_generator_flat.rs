use std::sync::Arc;

use crate::core::node::{
    Node, NodeCategory, NodeDescriptor, NodeError, NodeIcon, NodePortType, NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-flat",
    png_bytes: include_bytes!("../../assets/icons/node_flat.png")
};

#[derive(Debug, Default)]
pub struct NodeGeneratorFlat;
impl Node for NodeGeneratorFlat {
    fn label(&self) -> &str {
        "Flat Generator"
    }

    fn category(&self) -> NodeCategory {
        NodeCategory::Generator
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

    fn process(
        &self,
        pool: &Arc<TilePool>,
        _inputs: &[TileHandle],
        _ctx: &TileContext
    ) -> Result<Vec<TileHandle>, NodeError> {
        Ok(vec![Arc::new(pool.allocate())])
    }
}

inventory::submit! {
    NodeDescriptor {
        label: "Flat Generator",
        category: NodeCategory::Generator,
        icon: ICON,
        factory: || Box::new(NodeGeneratorFlat)
    }
}
