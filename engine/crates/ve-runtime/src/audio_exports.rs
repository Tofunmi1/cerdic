//! Procedural sound generation exports for the browser audio layer.
use ve_audio::{ALL_SFX, Mood, MusicSpec, Scale, music, sfx};
use wasm_bindgen::prelude::*;

/// Request a new explosion in the FX scene.
#[wasm_bindgen]
pub fn fx_explode() {
    crate::fx::request_explosion();
}

/// Return the number of particles currently rendered by the FX scene.
#[wasm_bindgen]
#[must_use]
pub fn fx_live_particles() -> usize {
    crate::fx::live_count()
}

/// Generate mono samples for a named sound effect. Unknown names return an empty vector.
#[wasm_bindgen]
#[must_use]
pub fn audio_sfx(kind: &str, seed: u32, intensity: f32) -> Vec<f32> {
    let Some(kind) = ALL_SFX
        .iter()
        .copied()
        .find(|candidate| format!("{candidate:?}").to_lowercase() == kind)
    else {
        return Vec::new();
    };
    sfx(kind, u64::from(seed), intensity).samples
}

/// Generate mono samples for a named music mood. Unknown moods return an empty vector.
#[wasm_bindgen]
#[must_use]
pub fn audio_music(mood: &str, seed: u32, bars: u32) -> Vec<f32> {
    let Ok(mood) = serde_json::from_str::<Mood>(&format!("\"{mood}\"")) else {
        return Vec::new();
    };
    let spec = MusicSpec {
        mood,
        seed: u64::from(seed),
        bars,
        tempo_bpm: 96.0,
        scale: Scale::Pentatonic,
    };
    music(spec).samples
}
