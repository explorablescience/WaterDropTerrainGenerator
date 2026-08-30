use std::sync::Arc;

use crate::core::node::{
    Node, NodeCategory, NodeDescriptor, NodeError, NodeIcon, NodePortType, NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-flat",
    png_bytes: include_bytes!("../../../../assets/icons/node_flat.png")
};

#[derive(Debug, Default)]
pub struct Flat;
impl Node for Flat {
    fn label(&self) -> &str {
        "Flat"
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
        label: "Flat",
        category: NodeCategory::Generation,
        subcategory: "Mathematical",
        icon: ICON,
        factory: || Box::new(Flat)
    }
}
