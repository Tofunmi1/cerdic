//! How the two eyes share one render target.

/// A pixel rectangle inside a render target, origin at the top left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Viewport {
    /// Left edge in pixels.
    pub x: u32,
    /// Top edge in pixels.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Viewport {
    /// Width divided by height, or 1 for an empty viewport.
    #[must_use]
    pub fn aspect(&self) -> f32 {
        if self.height == 0 {
            return 1.0;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "viewport sizes are far below 2^24"
        )]
        {
            self.width as f32 / self.height as f32
        }
    }
}

/// Splits a target into a left and a right eye viewport, side by side. An odd width gives the
/// extra pixel to the right eye, so the two always tile the target exactly.
#[must_use]
pub fn side_by_side(target_width: u32, target_height: u32) -> [Viewport; 2] {
    let left_width = target_width / 2;
    [
        Viewport {
            x: 0,
            y: 0,
            width: left_width,
            height: target_height,
        },
        Viewport {
            x: left_width,
            y: 0,
            width: target_width - left_width,
            height: target_height,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_the_target_exactly() {
        for (w, h) in [
            (1280, 720),
            (1281, 720),
            (2, 1),
            (1, 1),
            (0, 0),
            (4128, 2208),
        ] {
            let [left, right] = side_by_side(w, h);
            assert_eq!(left.x, 0);
            assert_eq!(left.x + left.width, right.x);
            assert_eq!(right.x + right.width, w);
            assert!(left.height == h && right.height == h);
        }
    }

    #[test]
    fn odd_width_favours_the_right_eye() {
        let [left, right] = side_by_side(101, 50);
        assert_eq!((left.width, right.width), (50, 51));
    }

    #[test]
    fn aspect_handles_empty_viewports() {
        let [left, _] = side_by_side(0, 0);
        assert!((left.aspect() - 1.0).abs() < f32::EPSILON);
        let [left, _] = side_by_side(200, 100);
        assert!((left.aspect() - 1.0).abs() < f32::EPSILON);
    }
}
