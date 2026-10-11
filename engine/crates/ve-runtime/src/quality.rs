//! Runtime quality control: the page asks for a tier by name and the app applies it on its next frame.

use std::cell::Cell;

use bevy_ecs::system::{Res, ResMut};
use ve_render::{QualityTier, RenderQuality};

thread_local! {
    static REQUESTED: Cell<Option<QualityTier>> = const { Cell::new(None) };
    static CURRENT: Cell<QualityTier> = const { Cell::new(QualityTier::High) };
}

/// Asks for a tier; the app switches to it on its next frame.
pub(crate) fn request(tier: QualityTier) {
    REQUESTED.with(|slot| slot.set(Some(tier)));
}

/// The resolution scale the headset's layer is allocated at: the largest any tier asks for. A
/// layer cannot be resized while it is bound, so it is made big enough for the best tier once, and
/// lower tiers render into a smaller part of it (see [`sync_xr_scale`]).
pub(crate) fn layer_scale() -> f32 {
    QualityTier::ALL
        .iter()
        .map(|tier| RenderQuality::for_tier(*tier).resolution_scale)
        .fold(f32::MIN_POSITIVE, f32::max)
}

/// Asks the headset to render each view at the current tier's share of the allocated layer. A
/// smaller viewport inside the same allocation cuts the pixels shaded, not the memory held; whether
/// the runtime grants it shows in the viewports each frame reports.
pub(crate) fn sync_xr_scale(quality: Res<RenderQuality>) {
    ve_xr_web::set_viewport_scale(quality.resolution_scale / layer_scale());
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
