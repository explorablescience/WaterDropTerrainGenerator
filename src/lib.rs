//! Library crate for the terrain generator.

pub mod core;
pub mod nodes;
pub mod render;
pub mod ui;

pub use core::terrain_graph::{TerrainGraph, TerrainGraphHolder};

/// Whether WaterDropEngine's own built-in UI menu items (e.g. "Engine/*", "Camera/*", "PBR/*") are shown.
pub const DEBUG_MODE: bool = false;
