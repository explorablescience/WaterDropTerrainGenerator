use bevy::prelude::*;
use wde::prelude::*;

use crate::{
    core::parallelism::ChunkJobs,
    render::{
        chunk_array::{
            TerrainPreviewArrayBg, TerrainPreviewGpu, TerrainPreviewInstances,
            TerrainPreviewInstancesBinding, TerrainPreviewSync, sync_terrain_preview_gpu
        },
        render_pipeline::TerrainPreviewRenderPipeline,
        render_subpass::SubRenderPassTerrainPreview,
        generate_chunks::{TerrainPreview, create_material, update_render_chunks},
    },
};

mod utils;
mod chunk_array;
mod render_pipeline;
mod render_subpass;
mod generate_chunks;
mod generate_chunks_global;
mod generate_chunks_local;

/// Handles the generation and rendering of the terrain preview mesh in the editor.
pub struct RenderPlugin;
impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainPreview>()
            .init_resource::<TerrainPreviewSync>()
            .init_resource::<ChunkJobs>()
            .add_systems(Startup, create_material)
            .add_systems(Update, update_render_chunks);

        // Chunk-instance storage buffer and heightmap texture array bind groups
        app.add_plugins((
            RenderDataRegisterPlugin::<TerrainPreviewInstances>::default(),
            RenderBindingRegisterPlugin::<TerrainPreviewInstancesBinding>::default(),
            RenderBindingRegisterPlugin::<TerrainPreviewArrayBg>::without_init(),
            ExtractResourcePlugin::<TerrainPreviewSync>::default(),
        ));

        // Render pipeline and subpass for rendering the chunks
        app.add_plugins(RenderPipelineRegisterPlugin::<TerrainPreviewRenderPipeline>::default());

        let render_app = app.get_sub_app_mut(RenderApp).unwrap();
        render_app
            .init_resource::<TerrainPreviewGpu>()
            .add_systems(Render, sync_terrain_preview_gpu.in_set(RenderSet::Prepare));
        render_app
            .world_mut()
            .get_resource_mut::<RenderGraph>()
            .unwrap()
            .add_sub_pass::<SubRenderPassTerrainPreview, RenderPassDeferredGBuffer>();
    }
}
