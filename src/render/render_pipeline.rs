use bevy::{
    ecs::system::{
        SystemParamItem,
        lifetimeless::{SRes, SResMut}
    },
    prelude::*
};
use wde::prelude::*;

use crate::render::chunk_array::{TerrainPreviewArrayBg, TerrainPreviewInstancesBinding};

#[derive(Default, Asset, Clone, TypePath, Debug)]
pub(crate) struct TerrainPreviewRenderPipeline(pub CachedPipelineIndex);
impl RenderAsset for TerrainPreviewRenderPipeline {
    type SourceAsset = RenderPipelineAsset<TerrainPreviewRenderPipeline>;
    type Params = (
        SRes<AssetServer>,
        SResMut<PipelineManager>,
        SBinding<CameraBinding>,
        SBinding<PbrMaterial>,
        SBinding<TerrainPreviewInstancesBinding>,
        SBinding<TerrainPreviewArrayBg>
    );

    fn prepare(
        _id: AssetId<Self::SourceAsset>,
        asset: Self::SourceAsset,
        (assets_server, pipeline_manager, camera, pbr_materials, instances, heightmap_array): &mut SystemParamItem<
            Self::Params
        >
    ) -> Result<Self, PrepareAssetError<Self::SourceAsset>> {
        Ok(TerrainPreviewRenderPipeline(
            pipeline_manager.create_render_pipeline(
                RenderPipelineDescriptor {
                    label: "terrain-preview",
                    vert: Some(assets_server.load("terrain_preview/terrain_preview.vert.wgsl")),
                    frag: Some(assets_server.load("terrain_preview/terrain_preview.frag.wgsl")),
                    bind_group_layouts: vec![
                        camera.iter().next().map(|(_, c)| c.layout.clone()),
                        pbr_materials.iter().next().map(|(_, m)| m.layout.clone()),
                        instances.iter().next().map(|(_, i)| i.layout.clone()),
                        heightmap_array.iter().next().map(|(_, a)| a.layout.clone()),
                    ],
                    depth: DepthDescriptor {
                        enabled: true,
                        write: true,
                        ..Default::default()
                    },
                    render_targets: Some(vec![
                        // Same order as the PbrDeferredTextures
                        PbrTextureFormat::DEPTH,  // Depth
                        PbrTextureFormat::ALBEDO, // Albedo
                        PbrTextureFormat::NORMAL, // Normal
                        PbrTextureFormat::AO,     // AO
                    ]),
                    sample_count: MSAA_SAMPLE_COUNT,
                    // One shared mesh (real bound vertex/index buffers, not the bindless SSBO
                    // scheme), instanced once per chunk - see chunk_array.rs.
                    ..default()
                },
                asset
            )?
        ))
    }
}
