use bevy::prelude::*;
use wde::prelude::*;

use crate::{
    core::session::ChunkJobs,
    render::{
        terrain_preview::{TerrainPreview, create_material, update_terrain_preview},
        terrain_preview_pipeline::TerrainPreviewRenderPipeline,
        terrain_preview_subpass::{SubRenderPassTerrainPreview, TerrainPreviewMeshes}
    }
};

mod chunk_stitching;
mod mesh_generation;
mod terrain_preview;
mod terrain_preview_global;
mod terrain_preview_local;
mod terrain_preview_pipeline;
mod terrain_preview_subpass;

pub struct RenderPlugin;
impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainPreview>()
            .init_resource::<ChunkJobs>()
            .add_systems(Startup, create_material)
            .add_systems(Update, update_terrain_preview);

        app.add_plugins(RenderPipelineRegisterPlugin::<TerrainPreviewRenderPipeline>::default());

        // Extract the terrain-preview meshes from the main world into the render world every frame
        app.init_resource::<TerrainPreviewMeshes>();
        app.get_sub_app_mut(RenderApp)
            .unwrap()
            .init_resource::<TerrainPreviewMeshes>()
            .add_systems(Extract, SubRenderPassTerrainPreview::extract);

        app.get_sub_app_mut(RenderApp)
            .unwrap()
            .world_mut()
            .get_resource_mut::<RenderGraph>()
            .unwrap()
            .add_sub_pass::<SubRenderPassTerrainPreview, RenderPassDeferredGBuffer>();
    }
}
