//! Per-view uniform initialization and optional view effects.

use bevy_math::{Mat4, Vec4};

use super::ViewUniform;
use crate::{SkyBackground, grade::Grading, quality::RenderQuality};

/// The clip-from-world matrix of each slot of a view (one for a camera, one per eye for a stereo view), as `prepare_view_uniforms`
/// builds them, for anything that must agree with what is drawn (culling).
pub(crate) fn view_clip_matrices(
    view: &bevy_render::view::ExtractedView,
    stereo: Option<&crate::stereo::StereoViews>,
) -> Vec<Mat4> {
    match stereo {
        Some(stereo) => stereo
            .eyes
            .iter()
            .map(|eye| eye.clip_from_view * eye.world_from_view.inverse())
            .collect(),
        None => {
            vec![view.clip_from_world.unwrap_or_else(|| {
                view.clip_from_view * view.world_from_view.to_matrix().inverse()
            })]
        }
    }
}

impl ViewUniform {
    /// The uniform for an eye or camera at `world_from_view` with projection `clip_from_view`.
    pub(crate) fn new(world_from_view: Mat4, clip_from_view: Mat4) -> Self {
        Self {
            clip_from_world: clip_from_view * world_from_view.inverse(),
            camera_position: world_from_view.w_axis.truncate().extend(1.0),
            exposure: Vec4::new(1.0, 0.0, 0.0, 0.0),
            grading: Vec4::new(1.0, 1.0, 0.0, 0.0),
            tint: Vec4::new(1.0, 1.0, 1.0, 0.0),
            effects: Vec4::ZERO,
            world_from_clip: (clip_from_view * world_from_view.inverse()).inverse(),
            viewport: Vec4::ZERO,
            horizon: Vec4::ZERO,
            zenith: Vec4::ZERO,
            ground: Vec4::ZERO,
            sun: Vec4::ZERO,
            cloud: Vec4::ZERO,
            wind: Vec4::new(1.0, 0.0, 0.0, 0.0),
        }
    }

    /// Writes the wind field the foliage vertex shaders bend with.
    pub(crate) fn set_wind(&mut self, wind: &crate::WindField) {
        self.wind = wind.uniform();
    }

    /// Writes the active viewer grading into this view's uniforms (see [`Grading`]).
    pub(crate) fn set_grading(&mut self, grading: &Grading) {
        self.exposure.x = grading.exposure;
        self.grading = Vec4::new(grading.contrast, grading.saturation, 0.0, 0.0);
        self.tint = Vec4::new(grading.tint[0], grading.tint[1], grading.tint[2], 0.0);
    }

    pub(crate) fn set_quality(&mut self, quality: &RenderQuality) {
        self.grading.w = if quality.tier == crate::QualityTier::Low {
            0.0
        } else {
            1.0
        };
        self.effects.x = if quality.specular_aa_enabled {
            1.0
        } else {
            0.0
        };
        self.effects.y = if quality.ssao_enabled { 1.0 } else { 0.0 };
        self.effects.z = if quality.bloom_enabled { 1.0 } else { 0.0 };
        self.effects.w = if quality.stochastic_sampling_enabled() {
            1.0
        } else {
            0.0
        };
    }

    /// Writes how much of the exposure correction this frame applies and the brightness it aims for (both zero when off).
    pub(crate) fn set_auto_exposure(&mut self, quality: &RenderQuality, delta: f32) {
        let on = quality.auto_exposure_enabled;
        self.exposure.y = if on {
            crate::exposure::adaptation(delta)
        } else {
            0.0
        };
        self.exposure.z = if on { crate::exposure::TARGET } else { 0.0 };
    }

    pub(super) fn set_sky(&mut self, sky: &SkyBackground, viewport: [u32; 4]) {
        #[expect(
            clippy::cast_precision_loss,
            reason = "viewports are bounded by browser render targets"
        )]
        {
            self.viewport = Vec4::new(
                viewport[0] as f32,
                viewport[1] as f32,
                viewport[2] as f32,
                viewport[3] as f32,
            );
        }
        // The fourth lane tells the forward shader there is an authored sky to reflect (the sky pass ignores it).
        let authored = if sky.enabled { 1.0 } else { 0.0 };
        self.horizon = Vec4::new(sky.horizon[0], sky.horizon[1], sky.horizon[2], authored);
        self.zenith = Vec4::new(sky.zenith[0], sky.zenith[1], sky.zenith[2], 0.0);
        self.ground = Vec4::new(sky.ground[0], sky.ground[1], sky.ground[2], 0.0);
        self.sun = Vec4::new(
            sky.sun_direction[0],
            sky.sun_direction[1],
            sky.sun_direction[2],
            sky.sun_intensity,
        );
        #[expect(
            clippy::cast_precision_loss,
            reason = "cloud seeds are bounded to one million"
        )]
        let seed = sky.seed as f32;
        self.cloud = Vec4::new(
            seed,
            sky.cloud_cover,
            sky.turbidity,
            if sky.clouds { 1.0 } else { 0.0 },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_low_quality_disables_stochastic_sampling() {
        for tier in crate::QualityTier::ALL {
            let mut uniform = ViewUniform::new(Mat4::IDENTITY, Mat4::IDENTITY);
            uniform.set_quality(&RenderQuality::for_tier(tier));
            assert_eq!(uniform.effects.w > 0.5, tier != crate::QualityTier::Low);
        }
    }

    #[test]
    fn specular_aa_is_opt_in_in_the_per_view_uniform() {
        let world_from_view = Mat4::IDENTITY;
        let mut uniform = ViewUniform::new(world_from_view, Mat4::IDENTITY);
        let mut quality = RenderQuality::default();
        uniform.set_quality(&quality);
        assert_eq!(uniform.effects.x, 0.0);

        quality.specular_aa_enabled = true;
        quality.ssao_enabled = true;
        quality.bloom_enabled = true;
        uniform.set_quality(&quality);
        assert_eq!(uniform.effects.x, 1.0);
        assert_eq!(uniform.effects.y, 1.0);
        assert_eq!(uniform.effects.z, 1.0);
    }
}
