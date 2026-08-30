//! GPU compute entry point for `Node::process` implementations, which run from background
//! `AsyncComputeTaskPool` tasks and have no render-world access of their own. Forwards to
//! `wde_renderer`'s `ComputeDispatcher`, which caches pipelines by label and is safe to call from
//! any thread.

use std::sync::OnceLock;

use wde::wde_renderer::compute::ComputeDispatcher;

use crate::core::node::NodeError;

static DISPATCHER: OnceLock<ComputeDispatcher> = OnceLock::new();

pub fn init(dispatcher: ComputeDispatcher) {
    let _ = DISPATCHER.set(dispatcher);
}

pub fn dispatch_f32<P: bytemuck::NoUninit>(
    label: &'static str,
    shader: &str,
    params: &P,
    inputs: &[&[f32]],
    output_len: usize,
    workgroups: (u32, u32, u32)
) -> Result<Vec<f32>, NodeError> {
    let dispatcher = DISPATCHER
        .get()
        .ok_or("GPU compute requested before the render world initialized it")?;
    dispatcher
        .dispatch_f32(label, shader, params, inputs, output_len, workgroups)
        .map_err(NodeError::from)
}
