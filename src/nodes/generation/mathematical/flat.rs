use std::sync::Arc;

use crate::core::gpu;
use crate::core::node::{
    Node, NodeCategory, NodeDescriptor, NodeError, NodeIcon, NodePortType, NodeSocket
};
use crate::core::tiling::{TileContext, TileHandle, TilePool};

const ICON: NodeIcon = NodeIcon {
    id: "node-flat",
    png_bytes: include_bytes!("../../../../assets/icons/node_flat.png")
};

const SHADER: &str = include_str!("flat.comp.wgsl");
const WORKGROUP_SIZE: u32 = 8;

/// Layout must match `flat.comp.wgsl`'s `Params` struct exactly.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FlatParams {
    tile_size: [u32; 4]
}

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
        let mut output = pool.allocate();
        let size = output.size() as u32;
        let params = FlatParams {
            tile_size: [size, 0, 0, 0]
        };
        let workgroups = size.div_ceil(WORKGROUP_SIZE);
        let result = gpu::dispatch_f32(
            "flat",
            SHADER,
            &params,
            &[],
            (size * size) as usize,
            (workgroups, workgroups, 1)
        )?;
        output.copy_from_slice(&result);
        Ok(vec![Arc::new(output)])
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
