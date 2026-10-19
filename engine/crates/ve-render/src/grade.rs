//! Viewer-scope grading: exposure before tonemapping, then contrast, saturation and tint on the
//! tonemapped image.
//!
//! Grading is renderer-only. It is a viewer setting, not an authored document value: it never reaches
//! the evaluator, so native and WASI evaluation output is identical with or without it. The transform
//! is bounded and finite for finite input; [`Grading::clamped`] pins every control to its documented
//! range and [`Grading::grade`] is the Rust mirror of the `forward.wgsl` fragment stage.

use std::fmt;
use std::str::FromStr;

use bevy_ecs::prelude::*;
use bevy_render::Extract;

/// Smallest and largest exposure multiplier accepted (neutral is 1.0).
pub const EXPOSURE_RANGE: (f32, f32) = (0.25, 4.0);
/// Contrast bounds (neutral is 1.0, 0.0 is flat mid-grey).
pub const CONTRAST_RANGE: (f32, f32) = (0.0, 2.0);
/// Saturation bounds (neutral is 1.0, 0.0 is greyscale).
pub const SATURATION_RANGE: (f32, f32) = (0.0, 2.0);
/// Per-channel tint bounds (neutral is 1.0).
pub const TINT_RANGE: (f32, f32) = (0.5, 1.5);

/// Rec. 709 luminance weights, shared with `forward.wgsl`.
const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// A named, bounded viewer grading preset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GradePreset {
    /// The engine's ungraded look.
    #[default]
    Neutral,
    /// A gentle studio lift: slightly brighter, a touch more contrast and saturation.
    Studio,
    /// A warm studio: lifts reds and eases blues.
    Warm,
    /// A cool studio: lifts blues and eases reds.
    Cool,
    /// Greyscale: saturation at zero with a small contrast lift.
    Mono,
}

impl GradePreset {
    /// All presets, in menu order.
    pub const ALL: [Self; 5] = [
        Self::Neutral,
        Self::Studio,
        Self::Warm,
        Self::Cool,
        Self::Mono,
    ];

    /// The preset's lowercase name, as accepted by [`FromStr`].
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Studio => "studio",
            Self::Warm => "warm",
            Self::Cool => "cool",
            Self::Mono => "mono",
        }
    }
}

impl fmt::Display for GradePreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for GradePreset {
    type Err = ();

    fn from_str(name: &str) -> Result<Self, ()> {
        Self::ALL
            .into_iter()
            .find(|preset| preset.name() == name)
            .ok_or(())
    }
}

/// The active viewer grading, clamped to the documented ranges.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Grading {
    /// Exposure multiplier applied to scene-referred colour before tonemapping.
    pub exposure: f32,
    /// Contrast around mid grey, applied after tonemapping.
    pub contrast: f32,
    /// Saturation around luminance, applied after tonemapping.
    pub saturation: f32,
    /// Per-channel tint multiplier, applied last.
    pub tint: [f32; 3],
}

impl Default for Grading {
    fn default() -> Self {
        Self::neutral()
    }
}

impl Grading {
    /// The neutral (identity) grading.
    #[must_use]
    pub const fn neutral() -> Self {
        Self {
            exposure: 1.0,
            contrast: 1.0,
            saturation: 1.0,
            tint: [1.0; 3],
        }
    }

    /// The grading of a named preset.
    #[must_use]
    pub const fn for_preset(preset: GradePreset) -> Self {
        match preset {
            GradePreset::Neutral => Self::neutral(),
            GradePreset::Studio => Self {
                exposure: 1.05,
                contrast: 1.08,
                saturation: 1.06,
                tint: [1.0, 1.0, 1.0],
            },
            GradePreset::Warm => Self {
                exposure: 1.05,
                contrast: 1.02,
                saturation: 1.05,
                tint: [1.06, 1.0, 0.94],
            },
            GradePreset::Cool => Self {
                exposure: 1.0,
                contrast: 1.05,
                saturation: 1.0,
                tint: [0.95, 1.0, 1.07],
            },
            GradePreset::Mono => Self {
                exposure: 1.0,
                contrast: 1.1,
                saturation: 0.0,
                tint: [1.0, 1.0, 1.0],
            },
        }
    }

    /// Every control clamped to its documented range; a non-finite control becomes neutral.
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            exposure: bound(self.exposure, EXPOSURE_RANGE),
            contrast: bound(self.contrast, CONTRAST_RANGE),
            saturation: bound(self.saturation, SATURATION_RANGE),
            tint: [
                bound(self.tint[0], TINT_RANGE),
                bound(self.tint[1], TINT_RANGE),
                bound(self.tint[2], TINT_RANGE),
            ],
        }
    }

    /// Applies contrast, saturation and tint to a tonemapped display-referred colour.
    ///
    /// Exposure is applied before tonemapping and so is not part of this function. The order and
    /// arithmetic mirror the `forward.wgsl` fragment stage; keep the two in step.
    #[must_use]
    pub fn grade(&self, color: [f32; 3]) -> [f32; 3] {
        let mut out = [0.0_f32; 3];
        for (channel, value) in color.iter().enumerate() {
            out[channel] = (value - 0.5) * self.contrast + 0.5;
        }
        let luminance = LUMA[0] * out[0] + LUMA[1] * out[1] + LUMA[2] * out[2];
        for (channel, value) in out.iter_mut().enumerate() {
            *value = (luminance + (*value - luminance) * self.saturation) * self.tint[channel];
        }
        out
    }
}

/// Clamps `value` to `range`, replacing a non-finite value with neutral (1.0).
fn bound(value: f32, (low, high): (f32, f32)) -> f32 {
    if value.is_finite() {
        value.clamp(low, high)
    } else {
        1.0
    }
}

/// Copies the main-world [`Grading`] into the render world each frame.
pub(crate) fn extract_grading(mut commands: Commands, grading: Extract<Res<Grading>>) {
    commands.insert_resource(**grading);
}

#[cfg(test)]
#[path = "grade_tests.rs"]
mod tests;
