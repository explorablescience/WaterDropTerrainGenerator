//! Heightmap <-> PNG file I/O, shared by whole-terrain export and any node that saves a heightmap
//! directly (e.g. `NodeSaveHeightmap`).

use std::path::Path;

/// Clamps `data` to `[0, 1]`, scales it to 8-bit grayscale, and saves it as a PNG at `path`,
/// creating parent directories as needed.
pub fn save_heightmap_png(
    data: &[f32],
    width: usize,
    height: usize,
    path: &Path
) -> Result<(), String> {
    let mut img = image::GrayImage::new(width as u32, height as u32);
    for (pixel, &value) in img.pixels_mut().zip(data.iter()) {
        pixel.0 = [(value.clamp(0.0, 1.0) * 255.0).round() as u8];
    }

    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("Failed to create directory '{}': {}", dir.display(), e))?;
    }
    img.save(path)
        .map_err(|e| format!("Failed to save '{}': {}", path.display(), e))
}
