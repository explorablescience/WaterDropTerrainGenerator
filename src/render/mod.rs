use bevy::prelude::*;
use wde::prelude::*;

use crate::{
    core::parallelism::ChunkJobs,
    render::{
        render_pipeline::TerrainPreviewRenderPipeline,
        render_subpass::{SubRenderPassTerrainPreview, TerrainPreviewMeshes},
        generate_chunks::{TerrainPreview, create_material, update_render_chunks},
    },
};

mod utils;
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
            .init_resource::<ChunkJobs>()
            .add_systems(Startup, create_material)
            .add_systems(Update, update_render_chunks);

        // Add the render pipeline and subpass for rendering the chunks. The rendered meshes should be stored in the `TerrainPreviewMeshes`.
        app.init_resource::<TerrainPreviewMeshes>()
            .add_plugins((
                RenderPipelineRegisterPlugin::<TerrainPreviewRenderPipeline>::default(),
                ExtractResourcePlugin::<TerrainPreviewMeshes>::default(),
            ))
            .get_sub_app_mut(RenderApp)
            .unwrap()
            .world_mut()
            .get_resource_mut::<RenderGraph>()
            .unwrap()
            .add_sub_pass::<SubRenderPassTerrainPreview, RenderPassDeferredGBuffer>();
    }
}
