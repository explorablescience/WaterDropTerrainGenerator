//! Assembles a chunked graph's per-chunk output into one full-terrain buffer - the stitching step behind whole-terrain export.

use bevy::tasks::ComputeTaskPool;

use crate::core::{
    graph::{GraphNodeId, NodeGraph, NodeGraphProcessResult},
    node::NodeError,
    tiling::{ChunkCoord, crop_center}
};

/// Blits each chunk's cropped core region into one full-terrain buffer, in row-major order.
/// Returns the data plus its width/height in texels (`chunks_x * tile_size` by
/// `chunks_y * tile_size`). Chunks are evaluated concurrently on the compute task pool.
pub fn assemble_terrain(
    graph: &mut NodeGraph,
    node_id: GraphNodeId
) -> Result<(Vec<f32>, usize, usize), NodeError> {
    let chunk_grid = *graph.chunk_grid();
    let tile_size = chunk_grid.tile_size();
    let width = chunk_grid.chunks_x() as usize * tile_size;
    let height = chunk_grid.chunks_y() as usize * tile_size;
    let mut assembled = vec![0.0f32; width * height];

    graph.prepare_for_parallel_eval(node_id)?;
    let label = graph.node(node_id)?.label().to_string();
    let graph: &NodeGraph = graph; // shared reborrow: chunk tasks below only need `&self`

    let results: Vec<Result<(ChunkCoord, Vec<f32>), NodeError>> =
        ComputeTaskPool::get().scope(|scope| {
            for chunk in chunk_grid.coords() {
                let label = label.clone();
                scope.spawn(async move {
                    let tiles = match graph.process_chunk_shared(node_id, chunk)? {
                        NodeGraphProcessResult::Processed(_, tiles) => tiles,
                        NodeGraphProcessResult::Processing => {
                            return Err(NodeError::NodeNotEvaluated(node_id));
                        }
                    };
                    let Some(heightmap) = tiles.first() else {
                        return Err(NodeError::OutputNotAvailable { node: label });
                    };

                    let internal_size = heightmap.size();
                    Ok((chunk, crop_center(heightmap, internal_size, tile_size)))
                });
            }
        });

    for result in results {
        let (chunk, cropped) = result?;
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
