//! Instance projection exports, keeping the root entry point small.
use crate::projection;
use wasm_bindgen::prelude::*;

/// Appends at most 512 checked instance placements to one prototype batch.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_instances(
    token: &str,
    group: u32,
    node: &str,
    matrices: &[f32],
    indices: &[u32],
) -> bool {
    projection::instances(token, group, node, matrices, indices)
}

/// Adds one prototype mesh chunk belonging to an instance batch.
#[wasm_bindgen]
#[must_use]
pub fn eval_projection_chunk_group(
    token: &str,
    group: u32,
    node: &str,
    name: &str,
    vertices: &[f32],
) -> bool {
    projection::chunk_group(token, group, node, name, vertices)
}
