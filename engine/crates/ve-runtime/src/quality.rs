//! Runtime quality control: the page asks for a tier by name and the app applies it on its next frame.

use std::cell::Cell;

use bevy_ecs::system::ResMut;
use ve_render::{QualityTier, RenderQuality};

thread_local! {
    static REQUESTED: Cell<Option<QualityTier>> = const { Cell::new(None) };
    static CURRENT: Cell<QualityTier> = const { Cell::new(QualityTier::High) };
}

/// Asks for a tier; the app switches to it on its next frame.
pub(crate) fn request(tier: QualityTier) {
    REQUESTED.with(|slot| slot.set(Some(tier)));
}

/// The render-resolution scale of the tier the app is on (or about to switch to), for sizing the
/// headset's framebuffer when a session starts.
pub(crate) fn resolution_scale() -> f32 {
    let tier = REQUESTED
        .with(Cell::get)
        .unwrap_or_else(|| CURRENT.with(Cell::get));
    RenderQuality::for_tier(tier).resolution_scale
}

/// Switches [`RenderQuality`] to the requested tier, if one is waiting.
pub(crate) fn apply_requested(mut quality: ResMut<RenderQuality>) {
    if let Some(tier) = REQUESTED.with(Cell::take) {
        CURRENT.with(|slot| slot.set(tier));
        if quality.tier != tier {
            *quality = RenderQuality::for_tier(tier);
        }
    }
}
