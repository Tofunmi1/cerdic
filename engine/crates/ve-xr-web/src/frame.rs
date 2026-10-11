//! One XR frame as plain data, and the pure logic that turns it into engine types.

use bevy_math::Mat4;
use ve_render::{StereoEye, StereoViews};
use ve_xr_math::FovTangents;

/// Floats per controller in the bridge's flat array (must match `CONTROLLER_STRIDE` in `xr.js`):
/// handedness, flags, grip matrix (16), aim matrix (16), buttons (8), axes (4).
pub const CONTROLLER_STRIDE: usize = 46;

/// Near plane for the eye projections, in metres.
pub const NEAR: f32 = 0.05;

/// Which hand a controller belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handedness {
    /// Not assigned to a hand (for example a gaze or transient pointer).
    None,
    /// Left hand.
    Left,
    /// Right hand.
    Right,
}

/// One tracked controller in tracking space (not yet placed in the world).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XrController {
    /// Which hand it is.
    pub handedness: Handedness,
    /// Pose of the grip (where a held object sits), if tracked.
    pub grip: Option<Mat4>,
    /// Pose of the pointing ray origin and direction (-Z), if tracked.
    pub aim: Option<Mat4>,
    /// Analogue value of the first eight buttons, 0 to 1 (trigger, squeeze, thumbstick press, ...).
    pub buttons: [f32; 8],
    /// First four axes, -1 to 1 (thumbstick x and y, ...).
    pub axes: [f32; 4],
}

/// One eye as reported by the runtime for this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XrView {
    /// The eye's pose in tracking space.
    pub world_from_view: Mat4,
    /// The runtime's OpenGL-style projection matrix, column-major.
    pub projection: [f32; 16],
    /// `[x, y, width, height]` in framebuffer pixels, origin bottom-left (GL convention).
    pub viewport: [i32; 4],
}

/// Everything the runtime reported for one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct XrFrame {
    /// Frame timestamp in milliseconds.
    pub time_ms: f64,
    /// The views, normally two (left, right).
    pub views: Vec<XrView>,
    /// Size of the layer's framebuffer in pixels.
    pub framebuffer_size: (u32, u32),
    /// Tracked controllers.
    pub controllers: Vec<XrController>,
}

/// Parses the bridge's flat controller array. A trailing partial record is ignored.
#[must_use]
pub fn parse_controllers(flat: &[f32]) -> Vec<XrController> {
    flat.chunks_exact(CONTROLLER_STRIDE)
        .map(|chunk| {
            // Bit 0: grip tracked, bit 1: aim tracked (small whole numbers stored as floats).
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "the bridge writes 0..=3"
            )]
            let flags = chunk[1] as u8;
            let has_grip = flags & 1 != 0;
            let has_aim = flags & 2 != 0;
            let matrix = |from: usize| {
                let mut m = [0.0_f32; 16];
                m.copy_from_slice(&chunk[from..from + 16]);
                Mat4::from_cols_array(&m)
            };
            let mut buttons = [0.0_f32; 8];
            buttons.copy_from_slice(&chunk[34..42]);
            let mut axes = [0.0_f32; 4];
            axes.copy_from_slice(&chunk[42..46]);
            XrController {
                handedness: match chunk[0] {
                    1.0 => Handedness::Left,
                    2.0 => Handedness::Right,
                    _ => Handedness::None,
                },
                grip: has_grip.then(|| matrix(2)),
                aim: has_aim.then(|| matrix(18)),
                buttons,
                axes,
            }
        })
        .collect()
}

/// Builds the two-eye stereo view for a frame, placing tracking space in the world with `rig`.
///
/// Returns `None` unless the frame has exactly two views with usable projections. Viewports are
/// converted from the runtime's bottom-left origin to the renderer's top-left origin.
#[must_use]
pub fn stereo_from_frame(frame: &XrFrame, rig: Mat4) -> Option<StereoViews> {
    let [left, right] = frame.views.as_slice() else {
        return None;
    };
    let height = i32::try_from(frame.framebuffer_size.1).ok()?;
    let eye = |view: &XrView| -> Option<StereoEye> {
        let fov = FovTangents::from_gl_projection(&view.projection)?;
        let [x, y, width, view_height] = view.viewport;
        let top = height - y - view_height;
        if x < 0 || top < 0 || width <= 0 || view_height <= 0 {
            return None;
        }
        Some(StereoEye {
            world_from_view: rig * view.world_from_view,
            clip_from_view: fov.projection_reverse_z(NEAR),
            viewport: [
                u32::try_from(x).ok()?,
                u32::try_from(top).ok()?,
                u32::try_from(width).ok()?,
                u32::try_from(view_height).ok()?,
            ],
        })
    };
    Some(StereoViews {
        eyes: [eye(left)?, eye(right)?],
    })
}

#[cfg(test)]
mod tests {
    use bevy_math::Vec3;

    use super::*;

    /// An OpenGL perspective matrix for a symmetric 90 degree field of view.
    fn gl_90() -> [f32; 16] {
        let (near, far) = (0.05_f32, 100.0_f32);
        let mut m = [0.0; 16];
        m[0] = 1.0;
        m[5] = 1.0;
        m[10] = -(far + near) / (far - near);
        m[11] = -1.0;
        m[14] = -2.0 * far * near / (far - near);
        m
    }

    fn view(x: f32, viewport: [i32; 4]) -> XrView {
        XrView {
            world_from_view: Mat4::from_translation(Vec3::new(x, 1.6, 0.0)),
            projection: gl_90(),
            viewport,
        }
    }

    fn frame(views: Vec<XrView>) -> XrFrame {
        XrFrame {
            time_ms: 0.0,
            views,
            framebuffer_size: (2000, 1000),
            controllers: vec![],
        }
    }

    #[test]
    fn two_views_become_two_eyes_with_flipped_viewports_and_the_rig_applied() {
        let f = frame(vec![
            view(-0.03, [0, 0, 1000, 1000]),
            view(0.03, [1000, 0, 1000, 1000]),
        ]);
        let rig = Mat4::from_translation(Vec3::new(0.0, 0.0, 2.0));
        let stereo = stereo_from_frame(&f, rig).unwrap();
        assert_eq!(stereo.eyes[0].viewport, [0, 0, 1000, 1000]);
        assert_eq!(stereo.eyes[1].viewport, [1000, 0, 1000, 1000]);
        let left = stereo.eyes[0].world_from_view.w_axis.truncate();
        assert!((left - Vec3::new(-0.03, 1.6, 2.0)).length() < 1e-6);
    }

    #[test]
    fn bottom_left_origin_viewports_are_flipped_to_top_left() {
        // A viewport covering only the bottom half of the framebuffer.
        let f = frame(vec![
            view(-0.03, [0, 0, 1000, 500]),
            view(0.03, [1000, 0, 1000, 500]),
        ]);
        let stereo = stereo_from_frame(&f, Mat4::IDENTITY).unwrap();
        assert_eq!(stereo.eyes[0].viewport, [0, 500, 1000, 500]);
    }

    #[test]
    fn rejects_frames_that_are_not_stereo_or_are_malformed() {
        let one = frame(vec![view(0.0, [0, 0, 1000, 1000])]);
        assert!(stereo_from_frame(&one, Mat4::IDENTITY).is_none());
        let bad_projection = XrView {
            projection: [0.0; 16],
            ..view(0.0, [0, 0, 1000, 1000])
        };
        let f = frame(vec![bad_projection, view(0.0, [1000, 0, 1000, 1000])]);
        assert!(stereo_from_frame(&f, Mat4::IDENTITY).is_none());
        let outside = frame(vec![
            view(0.0, [0, 900, 1000, 500]),
            view(0.0, [1000, 0, 1000, 1000]),
        ]);
        assert!(stereo_from_frame(&outside, Mat4::IDENTITY).is_none());
    }

    #[test]
    fn parses_controller_records() {
        let mut flat = vec![0.0_f32; CONTROLLER_STRIDE * 2];
        // First: right hand, grip only, trigger half pressed, thumbstick right.
        flat[0] = 2.0;
        flat[1] = 1.0;
        flat[2..18]
            .copy_from_slice(&Mat4::from_translation(Vec3::new(0.2, 1.0, -0.3)).to_cols_array());
        flat[34] = 0.5;
        flat[42] = 1.0;
        // Second: left hand, aim only.
        flat[CONTROLLER_STRIDE] = 1.0;
        flat[CONTROLLER_STRIDE + 1] = 2.0;
        let controllers = parse_controllers(&flat);
        assert_eq!(controllers.len(), 2);
        assert_eq!(controllers[0].handedness, Handedness::Right);
        assert!(controllers[0].grip.is_some() && controllers[0].aim.is_none());
        assert!((controllers[0].buttons[0] - 0.5).abs() < f32::EPSILON);
        assert!((controllers[0].axes[0] - 1.0).abs() < f32::EPSILON);
        assert_eq!(controllers[1].handedness, Handedness::Left);
        assert!(controllers[1].grip.is_none() && controllers[1].aim.is_some());
    }

    #[test]
    fn ignores_a_trailing_partial_controller_record() {
        assert!(parse_controllers(&[1.0; CONTROLLER_STRIDE - 1]).is_empty());
        assert_eq!(
            parse_controllers(&vec![0.0; CONTROLLER_STRIDE + 10]).len(),
            1
        );
    }
}
