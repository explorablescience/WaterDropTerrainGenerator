//! Integration tests for individual `Node` implementations under `src/nodes/`.
//! Only the CPU path is exercised - GPU dispatch needs a running render world's
//! `wde_renderer::compute::ComputeDispatcher`, which a plain `#[test]` doesn't have.

mod combine;
mod satmap;
