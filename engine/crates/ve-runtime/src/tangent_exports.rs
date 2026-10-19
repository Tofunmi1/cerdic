//! Browser ABI for the kernel's MikkTSpace tangent generator.

use wasm_bindgen::prelude::*;

/// Generates tangents in triangle-corner order for normal-mapped GLB export.
/// Inputs are flattened per-vertex positions/normals/UVs and triangle indices.
#[wasm_bindgen]
pub fn eval_tangents(
    positions: Vec<f32>,
    normals: Vec<f32>,
    uvs: Vec<f32>,
    indices: Vec<u32>,
) -> Result<Vec<f32>, JsError> {
    if positions.len() % 3 != 0
        || normals.len() != positions.len()
        || uvs.len() != positions.len() / 3 * 2
    {
        return Err(JsError::new(
            "tangent inputs must have matching per-vertex position, normal and UV lengths",
        ));
    }
    let positions: Vec<[f32; 3]> = positions
        .chunks_exact(3)
        .map(|v| [v[0], v[1], v[2]])
        .collect();
    let normals: Vec<[f32; 3]> = normals
        .chunks_exact(3)
        .map(|v| [v[0], v[1], v[2]])
        .collect();
    let uvs: Vec<[f32; 2]> = uvs.chunks_exact(2).map(|v| [v[0], v[1]]).collect();
    let tangents = ve_kernel::tangents(&positions, &normals, &uvs, &indices)
        .map_err(|error| JsError::new(&error.to_string()))?;
    Ok(tangents.into_iter().flatten().collect())
}
