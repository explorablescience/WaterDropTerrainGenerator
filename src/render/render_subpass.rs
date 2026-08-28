use bevy::{
    ecs::system::{SystemParamItem, lifetimeless::SRes},
    prelude::*
};
use wde::prelude::*;

use crate::render::render_pipeline::TerrainPreviewRenderPipeline;

/// Stores the meshes and material to be rendered.
#[derive(Resource, Default, Clone, ExtractResource)]
pub(crate) struct TerrainPreviewMeshes {
    pub meshes: Vec<Handle<Mesh>>,
    pub material: Option<Handle<PbrMaterial>>
}

pub(crate) struct SubRenderPassTerrainPreview;
impl RenderSubPass for SubRenderPassTerrainPreview {
    type Params = (
        SRes<TerrainPreviewMeshes>,
        SRes<RenderAssets<GpuMesh>>,
        SRes<RenderAssets<TerrainPreviewRenderPipeline>>,
        SBinding<CameraBinding>,
        SBinding<PbrMaterial>
    );

    fn describe(
        (chunk_meshes, meshes, pipeline, camera, materials): &SystemParamItem<Self::Params>
    ) -> RenderSubPassDesc {
        let Some(pipeline_index) = pipeline.iter().next().map(|(_, p)| p.0) else {
            return RenderSubPassDesc::default();
        };
        let Some(material_bind_group) = chunk_meshes
            .material
            .as_ref()
            .and_then(|handle| materials.get(handle.id()))
            .map(|m| m.bind_group.clone())
        else {
            return RenderSubPassDesc::default();
        };

        let mut commands = vec![
            SubPassCommand::Pipeline(Some(pipeline_index)),
            SubPassCommand::BindGroup(0, camera.iter().next().map(|(_, c)| c.bind_group.clone())),
            SubPassCommand::BindGroup(1, Some(material_bind_group)),
        ];

        // One mesh command + draw per chunk
        for handle in &chunk_meshes.meshes {
            let Some(mesh) = meshes.get(handle.id()) else {
                continue;
            };
            commands.push(SubPassCommand::Mesh(Some(handle.id())));
            commands.push(SubPassCommand::DrawBatches(vec![DrawCommandsBatch {
                bind_group: None,
                index_range: 0..mesh.index_count,
                instance_range: 0..1
            }]));
        }

        RenderSubPassDesc(commands)
    }

    fn label() -> &'static str {
        "terrain-preview"
    }
}
