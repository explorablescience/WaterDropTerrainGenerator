//! Assembles a chunked graph's per-chunk output into one full-terrain buffer - the stitching step
//! behind whole-terrain export (and any future full-terrain preview), as opposed to previewing or
//! saving just the currently selected chunk.

use crate::core::{
    graph::{GraphNodeId, NodeGraph, NodeGraphProcessResult},
    node_error::NodeError,
    tile_allocator::crop_center
};

/// Evaluates `node_id` for every chunk of `graph`'s chunk grid and blits each chunk's cropped core
/// region into one full-terrain buffer, in row-major order.
///
/// Returns the assembled data along with its width and height in texels
/// (`chunks_x * tile_size` by `chunks_y * tile_size`).
pub fn assemble_terrain(
    graph: &mut NodeGraph,
    node_id: GraphNodeId
) -> Result<(Vec<f32>, usize, usize), NodeError> {
    let chunk_grid = *graph.chunk_grid();
    let tile_size = chunk_grid.tile_size();
    let width = chunk_grid.chunks_x() as usize * tile_size;
    let height = chunk_grid.chunks_y() as usize * tile_size;
    let mut assembled = vec![0.0f32; width * height];

    for chunk in chunk_grid.coords() {
        let tiles = match graph.process_chunk(node_id, chunk)? {
            NodeGraphProcessResult::Processed(_, tiles) => tiles,
            // Evaluation is synchronous and always resolves within one call; a `Processing`
            // result here would mean the graph was left mid-cycle, which is an internal bug.
            NodeGraphProcessResult::Processing => return Err(NodeError::NodeNotEvaluated(node_id))
        };
        let Some(heightmap) = tiles.first() else {
            return Err(NodeError::OutputNotAvailable {
                node: graph.node(node_id)?.label().to_string()
            });
        };

        let internal_size = heightmap.size();
        let cropped = crop_center(heightmap, internal_size, tile_size);

        let origin_x = chunk.0 as usize * tile_size;
        let origin_y = chunk.1 as usize * tile_size;
        for y in 0..tile_size {
            let row_start = (origin_y + y) * width + origin_x;
            assembled[row_start..row_start + tile_size]
                .copy_from_slice(&cropped[y * tile_size..(y + 1) * tile_size]);
        }
    }

    Ok((assembled, width, height))
}
