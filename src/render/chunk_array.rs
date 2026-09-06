use bevy::{ecs::system::SystemParamItem, prelude::*};
use wde::prelude::*;

/// Max number of chunks allowed in the terrain preview at once (max size of arrays).
pub(crate) const MAX_PREVIEW_CHUNKS: usize = 4096;

/// Per-chunk draw descriptor read by the vertex shader via `@builtin(instance_index)`.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ChunkInstance {
    /// World-space offset of this chunk's `(0, 0)` texel, in world units.
    pub world_offset: [f32; 2],
    /// World-space size of each texel in this chunk, in world units.
    pub cell_size: f32,
    /// Layer of the heightmap texture array that holds this chunk's padded heightmap.
    pub layer: u32
}

/// GPU-side storage buffer holding every chunk's [`ChunkInstance`], indexed by draw instance.
#[derive(Asset, Clone, TypePath, Default)]
pub(crate) struct TerrainPreviewInstances;
impl TerrainPreviewInstances {
    pub const BUFFER_ID: u32 = 0;
}
impl RenderData for TerrainPreviewInstances {
    type Params = ();

    fn describe(_params: &mut SystemParamItem<Self::Params>, builder: &mut RenderDataBuilder) {
        builder.add_buffer(
            Self::BUFFER_ID,
            Buffer {
                label: "terrain-preview-instances-buffer".to_string(),
                size: std::mem::size_of::<ChunkInstance>() * MAX_PREVIEW_CHUNKS,
                usage: BufferUsage::STORAGE | BufferUsage::COPY_DST,
                content: None
            }
        );
    }
}

#[derive(Asset, Clone, TypePath, Default)]
pub(crate) struct TerrainPreviewInstancesBinding;
impl RenderBinding for TerrainPreviewInstancesBinding {
    type Params = SRenderData<TerrainPreviewInstances>;

    fn describe(
        &mut self,
        instances: &SystemParamItem<Self::Params>,
        builder: &mut RenderBindingBuilder
    ) {
        builder.add_buffer(instances, TerrainPreviewInstances::BUFFER_ID);
    }

    fn label(&self) -> &str {
        "terrain_preview_instances_binding"
    }
}

/// Bind group exposing the heightmap texture array to the vertex shader.
#[derive(Asset, Clone, TypePath, Default)]
pub(crate) struct TerrainPreviewArrayBg {
    pub heightmap_array: Option<Handle<Texture>>
}
impl RenderBinding for TerrainPreviewArrayBg {
    type Params = ();

    fn describe(
        &mut self,
        _params: &SystemParamItem<Self::Params>,
        builder: &mut RenderBindingBuilder
    ) {
        builder.add_texture_array_view_from_id(self.heightmap_array.as_ref().map(|h| h.id()));
    }

    fn label(&self) -> &str {
        "terrain-preview-chunk-array"
    }
}

/// Main-world snapshot of everything the render world needs to draw the terrain preview.
#[derive(Resource, Default, Clone, ExtractResource)]
pub(crate) struct TerrainPreviewSync {
    pub heightmap_array: Option<Handle<Texture>>,
    pub mesh: Option<Handle<Mesh>>,
    pub material: Option<Handle<PbrMaterial>>,
    pub instances: Vec<ChunkInstance>,
    /// `(layer, padded heightmap data)` pairs queued since the last sync
    pub pending_writes: Vec<(u32, Vec<f32>)>
}

/// Render-world-only state that persists across frames.
#[derive(Resource, Default)]
pub(crate) struct TerrainPreviewGpu {
    pub ready: bool,
    pub heightmap_array: Option<Handle<Texture>>,
    pub bind_group: Option<Handle<TerrainPreviewArrayBg>>,
    pub chunk_count: u32,
    /// Writes not yet applied to the GPU texture, retried each frame until it succeeds.
    pub pending_writes: Vec<(u32, Vec<f32>)>
}

/// Uploads dirty heightmap layers, rewrites the instance buffer, and (re)creates the array bind
/// group when the heightmap texture handle changes.
pub(crate) fn sync_terrain_preview_gpu(
    asset_server: Res<AssetServer>,
    sync: Res<TerrainPreviewSync>,
    mut gpu: ResMut<TerrainPreviewGpu>,
    textures: Res<RenderAssets<GpuTexture>>,
    instances: ResRenderData<TerrainPreviewInstances>,
    buffers: Res<RenderAssets<GpuBuffer>>,
    render_instance: Res<RenderInstance>
) {
    gpu.chunk_count = sync.instances.len().min(MAX_PREVIEW_CHUNKS) as u32;
    if sync.instances.len() > MAX_PREVIEW_CHUNKS {
        warn!(
            "Terrain preview has {} chunks, exceeding the {} instance buffer capacity; extra chunks won't be drawn.",
            sync.instances.len(),
            MAX_PREVIEW_CHUNKS
        );
    }

    // Recreate the bind group whenever the heightmap array's handle changed
    if sync.heightmap_array != gpu.heightmap_array {
        gpu.heightmap_array = sync.heightmap_array.clone();
        gpu.bind_group = None;
        gpu.ready = false;
        gpu.pending_writes.clear();
    }
    if gpu.bind_group.is_none()
        && let Some(heightmap_array) = &gpu.heightmap_array
    {
        gpu.bind_group = Some(asset_server.add(TerrainPreviewArrayBg {
            heightmap_array: Some(heightmap_array.clone())
        }));
    }
    gpu.ready = gpu.bind_group.is_some();

    // Queue this frame's new writes
    if sync.is_changed() && !sync.pending_writes.is_empty() {
        gpu.pending_writes
            .extend(sync.pending_writes.iter().cloned());
    }

    // Flush as much of the queue as the GPU texture will currently accept
    if !gpu.pending_writes.is_empty()
        && let Some(heightmap_array) = &gpu.heightmap_array
        && let Some(tex) = textures.get(heightmap_array)
    {
        let render_instance = render_instance.0.read().unwrap();
        for (layer, data) in gpu.pending_writes.drain(..) {
            tex.texture.copy_from_buffer_layered(
                &render_instance,
                tex.texture.format,
                layer,
                bytemuck::cast_slice(&data)
            );
        }
    }

    // Rewrite the instance buffer with this frame's chunk list.
    if gpu.chunk_count > 0
        && let Some((_, instances_data)) = instances.iter().next()
        && let Some(instance_buffer) = instances_data.get_buffer(TerrainPreviewInstances::BUFFER_ID)
        && let Some(buf) = buffers.get(&instance_buffer)
    {
        let render_instance = render_instance.0.read().unwrap();
        buf.buffer.write(
            &render_instance,
            bytemuck::cast_slice(&sync.instances[..gpu.chunk_count as usize]),
            0
        );
    }
}
