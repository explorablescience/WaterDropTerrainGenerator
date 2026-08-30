//! Minimal GPU compute path for `Node::process` implementations. `Node::process` runs
//! synchronously from background `AsyncComputeTaskPool` tasks, not render-world systems, so there's
//! no render graph or bind-group registry available there - this borrows the `wgpu::Device`/`Queue`
//! the render world already owns (via [`init`]) and drives a compute dispatch + blocking readback
//! directly, bypassing that machinery entirely.

use std::sync::{Arc, OnceLock, RwLock};

use wde_wgpu::bind_group::{BindGroupBuilder, BindGroupLayout};
use wde_wgpu::buffer::{Buffer, BufferBindingType, BufferUsage};
use wde_wgpu::command_buffer::CommandBuffer;
use wde_wgpu::compute_pipeline::ComputePipeline;
use wde_wgpu::instance::RenderInstanceData;
use wde_wgpu::render_pipeline::ShaderStages;

use crate::core::node::NodeError;

static INSTANCE: OnceLock<Arc<RwLock<RenderInstanceData<'static>>>> = OnceLock::new();

/// Publishes the render world's shared GPU instance so [`dispatch_f32`] can reach it from
/// background chunk-eval tasks. Called once, from the app's render setup.
pub fn init(instance: Arc<RwLock<RenderInstanceData<'static>>>) {
    let _ = INSTANCE.set(instance);
}

/// Runs a WGSL compute shader over an `output_len`-element `f32` output buffer (binding 1),
/// uploading `params` as a uniform buffer (binding 0) and each of `inputs` as a read-only `f32`
/// storage buffer at bindings `2, 3, ...` in order. Blocks the calling thread until the result is
/// read back - fine from a background chunk-eval task, never from the render thread.
pub fn dispatch_f32<P: bytemuck::NoUninit>(
    label: &str,
    shader: &str,
    params: &P,
    inputs: &[&[f32]],
    output_len: usize,
    workgroups: (u32, u32, u32)
) -> Result<Vec<f32>, NodeError> {
    let instance = INSTANCE
        .get()
        .ok_or("GPU compute requested before the render world initialized it")?;
    let data = instance
        .read()
        .map_err(|_| "GPU render instance lock poisoned")?;

    let params_buf = Buffer::new(
        &data,
        &format!("{label}-params"),
        std::mem::size_of::<P>(),
        BufferUsage::UNIFORM | BufferUsage::COPY_DST,
        Some(bytemuck::bytes_of(params))
    );
    let output_size = output_len * std::mem::size_of::<f32>();
    let output_buf = Buffer::new(
        &data,
        &format!("{label}-output"),
        output_size,
        BufferUsage::STORAGE | BufferUsage::COPY_SRC,
        None
    );
    let input_bufs: Vec<Buffer> = inputs
        .iter()
        .enumerate()
        .map(|(i, slice)| {
            Buffer::new(
                &data,
                &format!("{label}-input{i}"),
                std::mem::size_of_val(*slice),
                BufferUsage::STORAGE | BufferUsage::COPY_DST,
                Some(bytemuck::cast_slice(slice))
            )
        })
        .collect();

    let layout = BindGroupLayout::new(label, |b| {
        b.add_buffer(0, ShaderStages::COMPUTE, BufferBindingType::Uniform);
        b.add_buffer(
            1,
            ShaderStages::COMPUTE,
            BufferBindingType::Storage { read_only: false }
        );
        for i in 0..inputs.len() {
            b.add_buffer(
                2 + i as u32,
                ShaderStages::COMPUTE,
                BufferBindingType::Storage { read_only: true }
            );
        }
    });
    let wgpu_layout = layout
        .build(&data)
        .map_err(|e| format!("{label}: {e:?}"))?;
    let mut entries = vec![
        BindGroupBuilder::buffer(0, &params_buf),
        BindGroupBuilder::buffer(1, &output_buf),
    ];
    for (i, buf) in input_bufs.iter().enumerate() {
        entries.push(BindGroupBuilder::buffer(2 + i as u32, buf));
    }
    let bind_group = BindGroupBuilder::build(label, &data, &wgpu_layout, &entries)
        .map_err(|e| format!("{label}: {e:?}"))?;

    let mut pipeline = ComputePipeline::new(label);
    pipeline.set_shader(shader).set_bind_groups(vec![wgpu_layout]);
    pipeline.init(&data).map_err(|e| format!("{label}: {e:?}"))?;

    let mut cmd = CommandBuffer::new(&data, label);
    {
        let mut pass = cmd.create_compute_pass(label);
        pass.set_pipeline(&pipeline)
            .map_err(|e| format!("{label}: {e:?}"))?
            .set_bind_group(0, &bind_group);
        pass.dispatch(workgroups.0, workgroups.1, workgroups.2)
            .map_err(|e| format!("{label}: {e:?}"))?;
    }
    cmd.submit(&data);

    let staging = Buffer::new(
        &data,
        &format!("{label}-staging"),
        output_size,
        BufferUsage::MAP_READ | BufferUsage::COPY_DST,
        None
    );
    staging.copy_from_buffer(&data, &output_buf);

    let mut result = vec![0.0f32; output_len];
    staging.map_read(&data, |view| {
        result.copy_from_slice(bytemuck::cast_slice(&view));
    });
    Ok(result)
}
