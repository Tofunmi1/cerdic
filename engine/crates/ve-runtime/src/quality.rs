//! Runtime quality control: the page asks for a tier by name and the app applies it on its next frame.

use std::cell::Cell;

use bevy_camera::Camera3d;
use bevy_ecs::system::{Query, Res, ResMut};
use bevy_render::view::Msaa;
use ve_render::{QualityTier, RenderQuality};
use wasm_bindgen::prelude::*;

thread_local! {
    static REQUESTED: Cell<Option<QualityTier>> = const { Cell::new(None) };
    static REQUESTED_SHADOWS: Cell<Option<bool>> = const { Cell::new(None) };
    static REQUESTED_SPECULAR_AA: Cell<Option<bool>> = const { Cell::new(None) };
    static REQUESTED_SSAO: Cell<Option<bool>> = const { Cell::new(None) };
    static REQUESTED_BLOOM: Cell<Option<bool>> = const { Cell::new(None) };
    static REQUESTED_AUTO_EXPOSURE: Cell<Option<bool>> = const { Cell::new(None) };
    static REQUESTED_SMAA: Cell<Option<bool>> = const { Cell::new(None) };
    static REQUESTED_SHADOW_SIZE: Cell<Option<u32>> = const { Cell::new(None) };
    static REQUESTED_EFFECTS: std::cell::RefCell<Vec<(String, bool)>> = const { std::cell::RefCell::new(Vec::new()) };
    static CURRENT: Cell<QualityTier> = const { Cell::new(QualityTier::High) };
}

/// Turns one surface effect on or off in the viewer, on the next frame: `"subsurface"`, `"clearcoat"`, `"translucency"`, `"iridescence"`, `"sheen"`,
/// `"relief"`, `"wetness"`, `"anisotropy"` or `"maps"` (the baked and procedural texture maps). The document is not touched: every surface keeps
/// what its author gave it, and the viewer simply stops drawing that layer.
///
/// # Errors
/// Fails if the name is not one of those.
#[wasm_bindgen]
pub fn set_effect(name: &str, enabled: bool) -> Result<(), JsError> {
    if !ve_render::Effects::NAMES.contains(&name) {
        return Err(JsError::new(&format!(
            "unknown effect {name:?}; the effects are {}",
            ve_render::Effects::NAMES.join(", ")
        )));
    }
    REQUESTED_EFFECTS.with(|list| list.borrow_mut().push((name.to_owned(), enabled)));
    Ok(())
}

/// Turns the post-process edge smoothing (SMAA) on or off on the next frame.
#[wasm_bindgen]
pub fn set_antialiasing(enabled: bool) {
    REQUESTED_SMAA.with(|slot| slot.set(Some(enabled)));
}

/// Sets the sun shadow map's side in texels (`0` turns shadows off; `512`, `1024`, `2048` and `4096` are the usual sizes).
///
/// # Errors
/// Fails if the size is not 0 or a power of two from 256 to 4096.
#[wasm_bindgen]
pub fn set_shadow_resolution(texels: u32) -> Result<(), JsError> {
    if texels != 0 && !(texels.is_power_of_two() && (256..=4096).contains(&texels)) {
        return Err(JsError::new(
            "the shadow resolution is 0 (off) or 256, 512, 1024, 2048 or 4096",
        ));
    }
    REQUESTED_SHADOW_SIZE.with(|slot| slot.set(Some(texels)));
    Ok(())
}

/// Asks for a tier; the app switches to it on its next frame.
pub(crate) fn request(tier: QualityTier) {
    REQUESTED.with(|slot| slot.set(Some(tier)));
}

/// Switches the render quality tier (`"low"`, `"medium"` or `"high"`). Takes effect on the next
/// frame, and may be called before `start`.
///
/// # Errors
/// Fails if the tier name is unknown.
#[wasm_bindgen]
pub fn set_quality(tier: &str) -> Result<(), JsError> {
    let tier = tier
        .parse()
        .map_err(|()| JsError::new(&format!("unknown quality tier {tier:?}")))?;
    request(tier);
    Ok(())
}

/// Enables or disables sun shadows without changing multisampling or render resolution.
#[wasm_bindgen]
pub fn set_shadows(enabled: bool) {
    REQUESTED_SHADOWS.with(|slot| slot.set(Some(enabled)));
}

/// Enables derivative-based geometric specular anti-aliasing on the next frame.
#[wasm_bindgen]
pub fn set_specular_aa(enabled: bool) {
    REQUESTED_SPECULAR_AA.with(|slot| slot.set(Some(enabled)));
}

/// Enables screen-space ambient occlusion on the next frame.
#[wasm_bindgen]
pub fn set_ssao(enabled: bool) {
    REQUESTED_SSAO.with(|slot| slot.set(Some(enabled)));
}

/// Enables the bright-region bloom post effect on the next frame.
#[wasm_bindgen]
pub fn set_bloom(enabled: bool) {
    REQUESTED_BLOOM.with(|slot| slot.set(Some(enabled)));
}

/// Enables automatic exposure (eye adaptation) on the next frame.
#[wasm_bindgen]
pub fn set_auto_exposure(enabled: bool) {
    REQUESTED_AUTO_EXPOSURE.with(|slot| slot.set(Some(enabled)));
}

/// Tells the renderer how long the last frame took, so exposure adapts at the same speed at any frame rate.
pub(crate) fn publish_frame_delta(
    time: Res<bevy_time::Time<bevy_time::Real>>,
    mut delta: ResMut<ve_render::FrameDelta>,
) {
    delta.0 = time.delta_secs();
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
pub(crate) fn apply_requested(
    mut quality: ResMut<RenderQuality>,
    mut cameras: Query<&mut Msaa, bevy_ecs::prelude::With<Camera3d>>,
) {
    if let Some(tier) = REQUESTED.with(Cell::take) {
        CURRENT.with(|slot| slot.set(tier));
        if quality.tier != tier {
            // Settings the person chose stay across a tier change.
            let (specular_aa_enabled, auto_exposure_enabled, smaa_enabled, effects) = (
                quality.specular_aa_enabled,
                quality.auto_exposure_enabled,
                quality.smaa_enabled,
                quality.effects,
            );
            *quality = RenderQuality::for_tier(tier);
            quality.specular_aa_enabled = specular_aa_enabled;
            quality.auto_exposure_enabled = auto_exposure_enabled;
            quality.smaa_enabled = smaa_enabled;
            quality.effects = effects;
        }
    }
    if let Some(enabled) = REQUESTED_SHADOWS.with(Cell::take) {
        quality.shadow_resolution = if enabled {
            match quality.tier {
                QualityTier::Low => 0,
                QualityTier::Quest | QualityTier::Medium => 1024,
                QualityTier::High => 2048,
            }
        } else {
            0
        };
        quality.shadow_pcf_radius = u32::from(enabled && quality.tier == QualityTier::High);
    }
    if let Some(texels) = REQUESTED_SHADOW_SIZE.with(Cell::take) {
        quality.shadow_resolution = texels;
        quality.shadow_pcf_radius = u32::from(texels > 0 && quality.tier == QualityTier::High);
    }
    if let Some(enabled) = REQUESTED_SMAA.with(Cell::take) {
        quality.smaa_enabled = enabled;
    }
    for (name, enabled) in REQUESTED_EFFECTS.with(|list| std::mem::take(&mut *list.borrow_mut())) {
        quality.effects.set(&name, enabled);
    }
    if let Some(enabled) = REQUESTED_SPECULAR_AA.with(Cell::take) {
        quality.specular_aa_enabled = enabled;
    }
    if let Some(enabled) = REQUESTED_SSAO.with(Cell::take) {
        quality.ssao_enabled = enabled;
    }
    if let Some(enabled) = REQUESTED_BLOOM.with(Cell::take) {
        quality.bloom_enabled = enabled;
    }
    if let Some(enabled) = REQUESTED_AUTO_EXPOSURE.with(Cell::take) {
        quality.auto_exposure_enabled = enabled;
    }
    // Occlusion reads its own single-sample depth texture (`ve_render::ao_depth`), so it does not limit the sample count.
    // Bevy's renderer (`ve-bevy`) sets the cameras' sample count itself, from the same settings.
    if cfg!(any(feature = "bevy-webgl", feature = "bevy-webgpu")) {
        return;
    }
    let wanted = Msaa::from_samples(quality.msaa_samples);
    for mut msaa in &mut cameras {
        if *msaa != wanted {
            *msaa = wanted;
        }
    }
}
