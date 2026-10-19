//! Runtime viewer grading control: the page picks a preset or explicit controls and the app applies
//! them on its next frame. The requested values are clamped to the documented ranges, so the shader
//! only ever sees bounded numbers.

use std::cell::Cell;
use std::str::FromStr;

use bevy_ecs::system::ResMut;
use ve_render::{GradePreset, Grading};
use wasm_bindgen::prelude::*;

thread_local! {
    static REQUESTED: Cell<Option<Grading>> = const { Cell::new(None) };
    static CURRENT: Cell<Grading> = const { Cell::new(Grading::neutral()) };
}

/// Asks for a grading; the app applies it on its next frame. Values are clamped first.
pub(crate) fn request(grading: Grading) {
    REQUESTED.with(|slot| slot.set(Some(grading.clamped())));
}

/// Switches [`Grading`] to the requested values, if any are waiting.
pub(crate) fn apply_requested(mut grading: ResMut<Grading>) {
    if let Some(next) = REQUESTED.with(Cell::take) {
        CURRENT.with(|slot| slot.set(next));
        if *grading != next {
            *grading = next;
        }
    }
}

/// The active grading as JSON, for the page to confirm selection reached the engine.
pub(crate) fn snapshot() -> String {
    CURRENT.with(|slot| {
        let grading = slot.get();
        serde_json::json!({
            "exposure": grading.exposure,
            "contrast": grading.contrast,
            "saturation": grading.saturation,
            "tint": grading.tint,
        })
        .to_string()
    })
}

/// Selects a viewer grading preset (`"neutral"`, `"studio"`, `"warm"`, `"cool"` or `"mono"`).
///
/// Grading is a viewer-only setting: it changes the image on screen and never reaches the evaluator,
/// so it does not affect saved documents or native/WASI evaluation output. Takes effect next frame.
///
/// # Errors
/// Fails if the preset name is unknown.
#[wasm_bindgen]
pub fn set_grading_preset(preset: &str) -> Result<(), JsError> {
    let preset = GradePreset::from_str(preset)
        .map_err(|()| JsError::new(&format!("unknown grading preset {preset:?}")))?;
    request(Grading::for_preset(preset));
    Ok(())
}

/// Sets explicit viewer grading controls, clamped to their documented ranges (exposure 0.25 to 4,
/// contrast and saturation 0 to 2, each tint channel 0.5 to 1.5): exposure, contrast, saturation and
/// the three tint channels.
#[wasm_bindgen]
pub fn set_grading(
    exposure: f32,
    contrast: f32,
    saturation: f32,
    tint_r: f32,
    tint_g: f32,
    tint_b: f32,
) {
    request(Grading {
        exposure,
        contrast,
        saturation,
        tint: [tint_r, tint_g, tint_b],
    });
}

/// The active viewer grading as JSON: `{ "exposure", "contrast", "saturation", "tint" }`.
#[wasm_bindgen]
#[must_use]
pub fn grading_status() -> String {
    snapshot()
}
