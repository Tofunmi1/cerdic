//! Wasm export for the model-facing `measure` tool, keeping the root entry point small.

use wasm_bindgen::prelude::wasm_bindgen;

/// Measures a mesh given as flat arrays and returns a plain-text summary (`detail` other than
/// `"full"`) or the complete JSON report. Malformed input comes back as
/// `{"error": ..., "suggestion": ...}`; it never traps.
#[wasm_bindgen]
#[must_use]
pub fn measure_mesh(positions: &[f32], indices: &[u32], uvs: &[f32], detail: &str) -> String {
    ve_measure::measure_mesh(positions, indices, uvs, detail)
}
