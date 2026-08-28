use bevy::{
    ecs::system::{SystemParamItem, lifetimeless::SRes},
    prelude::*,
};
use wde::prelude::*;

use crate::render::{
    chunk_array::{
        TerrainPreviewArrayBg, TerrainPreviewGpu, TerrainPreviewInstancesBinding,
        TerrainPreviewSync,
    },
    render_pipeline::TerrainPreviewRenderPipeline,
};

pub(crate) struct SubRenderPassTerrainPreview;
impl RenderSubPass for SubRenderPassTerrainPreview {
    type Params = (
        SRes<TerrainPreviewSync>,
        SRes<TerrainPreviewGpu>,
        SRes<RenderAssets<GpuMesh>>,
        SRes<RenderAssets<TerrainPreviewRenderPipeline>>,
        SBinding<CameraBinding>,
        SBinding<PbrMaterial>,
        SBinding<TerrainPreviewInstancesBinding>,
        SBinding<TerrainPreviewArrayBg>,
    );

    fn describe(
        (sync, gpu, meshes, pipeline, camera, materials, instances, heightmap_array): &SystemParamItem<
            Self::Params
        >,
    ) -> RenderSubPassDesc {
        if !gpu.ready || gpu.chunk_count == 0 {
            return RenderSubPassDesc::default();
        }

        RenderSubPassDesc(vec![
            SubPassCommand::Pipeline(pipeline.iter().next().map(|(_, p)| p.0)),
            SubPassCommand::Mesh(sync.mesh.as_ref().map(|m| m.id())),
            SubPassCommand::BindGroup(0, camera.iter().next().map(|(_, c)| c.bind_group.clone())),
            SubPassCommand::BindGroup(
                1,
                sync.material
                    .as_ref()
                    .and_then(|handle| materials.get(handle.id()))
                    .map(|m| m.bind_group.clone()),
            ),
            SubPassCommand::BindGroup(
                2,
                instances.iter().next().map(|(_, i)| i.bind_group.clone()),
            ),
            SubPassCommand::BindGroup(
                3,
                gpu.bind_group
                    .as_ref()
                    .and_then(|h| heightmap_array.get(h))
                    .map(|a| a.bind_group.clone()),
            ),
            SubPassCommand::DrawBatches(vec![DrawCommandsBatch {
                bind_group: None,
                index_range: 0..sync
                    .mesh
                    .as_ref()
                    .and_then(|m| meshes.get(m.id()))
                    .map(|m| m.index_count)
                    .unwrap_or(0),
                instance_range: 0..gpu.chunk_count,
            }]),
        ])
    }

    fn label() -> &'static str {
        "terrain-preview"
    }
}
