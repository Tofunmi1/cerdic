//! Lens warp: a post step that bends the finished picture the way a lens would. A wide perspective
//! view stretches its edges; a fisheye lens keeps angles per pixel even, and a headset's lenses
//! need the opposite bend so the picture looks flat again through them.
//!
//! The warp is part of the present pass: with a warp on, every view renders offscreen and the
//! present pass resamples it. With the warp off nothing here costs anything.

use bevy_ecs::prelude::*;
use bevy_math::Vec4;
use bevy_render::Extract;
use bevy_render::render_resource::{ShaderType, UniformBuffer};
use bevy_render::renderer::{RenderDevice, RenderQueue};

/// How the picture is bent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LensMode {
    /// Untouched.
    #[default]
    Off,
    /// Equidistant fisheye over the whole frame: angle from the centre grows evenly with distance
    /// from it. Wants a wide source view; pixels the source cannot supply are black.
    Fisheye,
    /// Barrel distortion on each half of the frame, as a split-screen headset view needs.
    Headset,
}

impl LensMode {
    /// The number the shader switches on.
    const fn code(self) -> f32 {
        match self {
            Self::Off => 0.0,
            Self::Fisheye => 1.0,
            Self::Headset => 2.0,
        }
    }
}

/// The lens warp for the window's views. Set it from the app; it applies on the next frame.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct LensWarp {
    /// The kind of bend.
    pub mode: LensMode,
    /// Tangent of half the source view's vertical field of view: how wide the picture being
    /// resampled is. Only the fisheye uses it.
    pub source_half_fov_tan: f32,
}

impl Default for LensWarp {
    fn default() -> Self {
        Self {
            mode: LensMode::Off,
            source_half_fov_tan: 1.0,
        }
    }
}

impl LensWarp {
    /// Whether views must render offscreen for the warp to resample them.
    pub(crate) fn active(self) -> bool {
        self.mode != LensMode::Off
    }

    /// A warp with usable numbers: a non-finite or tiny tangent would divide by zero in the shader.
    fn uniform(self) -> LensUniform {
        let tan = if self.source_half_fov_tan.is_finite() {
            self.source_half_fov_tan.clamp(0.05, 50.0)
        } else {
            1.0
        };
        LensUniform {
            params: Vec4::new(self.mode.code(), tan, 0.0, 0.0),
        }
    }
}

/// What the present shader reads: x the mode, y the tangent of half the source field of view.
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub(crate) struct LensUniform {
    params: Vec4,
}

/// The lens uniform for this frame.
#[derive(Resource, Default)]
pub(crate) struct LensGpu {
    pub(crate) buffer: UniformBuffer<LensUniform>,
}

/// Copies the app's lens warp into the render world.
pub(crate) fn extract_lens(mut commands: Commands, lens: Extract<Option<Res<LensWarp>>>) {
    commands.insert_resource(lens.as_deref().copied().unwrap_or_default());
}

/// Uploads the lens uniform.
pub(crate) fn prepare_lens(
    lens: Res<LensWarp>,
    mut gpu: ResMut<LensGpu>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    gpu.buffer.set(lens.uniform());
    gpu.buffer.write_buffer(&device, &queue);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_is_the_default_and_needs_no_offscreen_target() {
        assert!(!LensWarp::default().active());
        let fisheye = LensWarp {
            mode: LensMode::Fisheye,
            ..LensWarp::default()
        };
        assert!(fisheye.active());
    }

    #[test]
    fn the_uniform_carries_the_mode_and_survives_bad_numbers() {
        let warp = |tan: f32| LensWarp {
            mode: LensMode::Headset,
            source_half_fov_tan: tan,
        };
        assert_eq!(warp(2.0).uniform().params, Vec4::new(2.0, 2.0, 0.0, 0.0));
        for bad in [f32::NAN, f32::INFINITY, 0.0, -3.0] {
            let tan = warp(bad).uniform().params.y;
            assert!(tan.is_finite() && tan >= 0.05, "{bad} became {tan}");
        }
    }
}
