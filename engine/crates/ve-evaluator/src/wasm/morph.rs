//! Shape-key buffers transferred separately from the part's small JSON metadata.
use wasm_bindgen::prelude::*;

/// Number of shape keys on one evaluated part.
#[wasm_bindgen]
#[must_use]
pub fn eval_morph_count(part: u32) -> u32 {
    super::HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|result| result.parts.get(part as usize))
            .map_or(0, |part| {
                u32::try_from(part.morph_targets.len()).unwrap_or(u32::MAX)
            })
    })
}

/// A shape key's name, or an empty string when the index does not exist.
#[wasm_bindgen]
#[must_use]
pub fn eval_morph_name(part: u32, target: u32) -> String {
    super::HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|result| result.parts.get(part as usize))
            .and_then(|part| part.morph_targets.get(target as usize))
            .map_or_else(String::new, |target| target.name.clone())
    })
}

/// A shape key's initial glTF weight, or zero when the index does not exist.
#[wasm_bindgen]
#[must_use]
pub fn eval_morph_weight(part: u32, target: u32) -> f32 {
    super::HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|result| result.parts.get(part as usize))
            .and_then(|part| part.morph_targets.get(target as usize))
            .map_or(0.0, |target| target.weight)
    })
}

/// Per-vertex POSITION deltas for a shape key.
#[wasm_bindgen]
#[must_use]
pub fn eval_morph_positions(part: u32, target: u32) -> Vec<f32> {
    super::HOST.with(|cell| {
        cell.borrow()
            .result
            .as_ref()
            .and_then(|result| result.parts.get(part as usize))
            .and_then(|part| part.morph_targets.get(target as usize))
            .map_or_else(Vec::new, |target| {
                target.positions.iter().flatten().copied().collect()
            })
    })
}
