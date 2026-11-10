//! CPU references for the grass shader (ADR 0070).
//! Adapted from RedGPU's MIT-licensed landscape grass shaders; see lib/redgpu.

/// Clamped distance from the mesh's lowest vertex, measured in mesh heights.
pub fn height_ratio(y: f32, min_y: f32, inv_mesh_height: f32) -> f32 {
    ((y - min_y) * inv_mesh_height).clamp(0.0, 1.0)
}

/// Exposure-adjusted blade colour, blended into soil over the bottom 40%.
pub fn ground_blend(
    base_rgb: [f32; 3],
    ground_rgb: [f32; 3],
    ratio: f32,
    strength: f32,
    exposure_boost: f32,
) -> [f32; 3] {
    let blend = if strength > 0.01 {
        ((0.4 - ratio) * 2.5).clamp(0.0, 1.0) * strength
    } else {
        0.0
    };
    std::array::from_fn(|i| {
        base_rgb[i] * exposure_boost.max(0.1) * (1.0 - blend) + ground_rgb[i] * blend
    })
}

/// Sky light reaching a vertex: 65% at the roots, full light near the tip.
pub fn sky_occlusion(ratio: f32) -> f32 {
    0.65 + 0.35 * (ratio * 1.43).clamp(0.0, 1.0)
}

/// Contact shadow fading out over the bottom quarter of a blade.
pub fn contact_ao(ratio: f32) -> f32 {
    0.4 + 0.6 * (ratio * 4.0).clamp(0.0, 1.0)
}

/// Unit normal tilted toward world up, more strongly at the tip.
pub fn upward_normal(normal: [f32; 3], ratio: f32) -> [f32; 3] {
    let t = 0.55 + 0.3 * ratio;
    let n = [
        normal[0] * (1.0 - t),
        normal[1] * (1.0 - t) + t,
        normal[2] * (1.0 - t),
    ];
    let len = n.iter().map(|v| v * v).sum::<f32>().sqrt();
    if len > 1e-6 && len.is_finite() {
        n.map(|v| v / len)
    } else {
        [0.0, 1.0, 0.0]
    }
}

/// Amount added to a rotated blade's local y to lift a sunken root.
pub fn sink_correction(local_y: f32, ratio: f32) -> f32 {
    (-local_y).max(0.0) * (1.0 - ratio * 0.7)
}

/// Full size through `near`, then linear shrink to zero at `far`.
/// Invalid distances keep the blade visible; invalid or reversed ranges disable fading.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the clamped ratio lies in 0..=1"
)]
pub fn fade_scale(distance: f32, near: f32, far: f32) -> f32 {
    if !distance.is_finite() || !near.is_finite() || !far.is_finite() || far <= near {
        return 1.0;
    }
    // f64 avoids overflow for finite f32 endpoints of opposite signs.
    (1.0 - (f64::from(distance) - f64::from(near)) / (f64::from(far) - f64::from(near)))
        .clamp(0.0, 1.0) as f32
}
