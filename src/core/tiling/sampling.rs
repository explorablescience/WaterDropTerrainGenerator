//! Generic per-texel math over flat `&[f32]` grids, independent of [`TilePool`](super::TilePool).

/// Clamps to the grid's edge when `x, y` fall outside `[0, size - 1]`.
pub fn bilinear_sample(data: &[f32], size: usize, x: f32, y: f32) -> f32 {
    let px = |x: usize, y: usize| data[y * size + x];

    let fx = x.clamp(0.0, (size - 1) as f32);
    let fy = y.clamp(0.0, (size - 1) as f32);
    let x0 = fx as usize;
    let y0 = fy as usize;
    let x1 = (x0 + 1).min(size - 1);
    let y1 = (y0 + 1).min(size - 1);
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;

    let top = px(x0, y0) * (1.0 - tx) + px(x1, y0) * tx;
    let bottom = px(x0, y1) * (1.0 - tx) + px(x1, y1) * tx;
    top * (1.0 - ty) + bottom * ty
}

/// Crops off the kernel-padding margin around a node's requested output.
pub fn crop_padding(data: &[f32], full_size: usize, target_size: usize) -> Vec<f32> {
    if full_size == target_size {
        return data.to_vec();
    }
    let padding = (full_size - target_size) / 2;
    let mut cropped = Vec::with_capacity(target_size * target_size);
    for z in 0..target_size {
        let row_start = (z + padding) * full_size + padding;
        cropped.extend_from_slice(&data[row_start..row_start + target_size]);
    }
    cropped
}
