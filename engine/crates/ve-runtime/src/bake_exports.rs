//! Wasm export for the model-facing bake tool.
use wasm_bindgen::prelude::wasm_bindgen;

/// Bakes requested maps from flat low and high mesh arrays, returning JSON with base64 RGBA images.
#[wasm_bindgen]
#[must_use]
pub fn bake_maps(
    low_positions: &[f32],
    low_indices: &[u32],
    low_corner_uvs: &[f32],
    high_positions: &[f32],
    high_indices: &[u32],
    size: u32,
    maps: &str,
    cage: f32,
    max_distance: f32,
    rays: u32,
    bias: f32,
) -> String {
    ve_measure::bake_maps_json(
        low_positions,
        low_indices,
        low_corner_uvs,
        high_positions,
        high_indices,
        size,
        maps,
        cage,
        max_distance,
        rays,
        bias,
    )
}
