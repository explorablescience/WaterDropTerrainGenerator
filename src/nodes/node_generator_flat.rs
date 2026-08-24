use std::sync::Arc;

use crate::core::node::{Node, NodeError, NodePortType, NodeSocket};
use crate::core::tile_allocator::{TileHandle, TilePool};

/// A node that generates a flat terrain tile.
#[derive(Debug, Default)]
pub struct NodeGeneratorFlat;
impl Node for NodeGeneratorFlat {
    fn label(&self) -> &str {
        "Generator Flat Terrain"
    }

    fn outputs(&self) -> &[NodeSocket] {
        &[NodeSocket {
            name: "height",
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
