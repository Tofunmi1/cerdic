//! Measurements for sampled animation tracks.

use crate::MeasureError;

/// Foot sliding measured across frames that are near the track's lowest point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FootSlide {
    /// Number of frames classified as contact.
    pub contact_frames: usize,
    /// Mean horizontal speed over consecutive contact-frame pairs, in metres per second.
    pub mean_speed: f32,
    /// Maximum horizontal speed over consecutive contact-frame pairs, in metres per second.
    pub max_speed: f32,
}

/// The positional gap and average per-frame movement for a sampled loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoopSeam {
    /// Largest distance between a point in the last frame and its first-frame position.
    pub gap: f32,
    /// Mean distance moved per point per consecutive frame pair.
    pub typical_step: f32,
}

impl LoopSeam {
    /// Returns whether the end-to-start jump is no larger than one and a half typical steps.
    #[must_use]
    pub fn seamless(&self) -> bool {
        self.gap <= 1.5 * self.typical_step
    }
}

/// Measures horizontal foot speed while the foot is near its lowest sampled height.
///
/// # Errors
/// Returns [`MeasureError`] if there are fewer than two frames, timing or contact settings are
/// invalid, or any coordinate is non-finite.
pub fn foot_slide(
    foot: &[[f32; 3]],
    dt: f32,
    contact_height: f32,
) -> Result<FootSlide, MeasureError> {
    if foot.len() < 2 {
        return Err(MeasureError::MotionFrames {
            measurement: "foot_slide",
            frames: foot.len(),
        });
    }
    if !dt.is_finite() || dt <= 0.0 {
        return Err(MeasureError::MotionSetting {
            measurement: "foot_slide",
            setting: "dt",
            requirement: "finite and positive",
        });
    }
    if !contact_height.is_finite() || contact_height < 0.0 {
        return Err(MeasureError::MotionSetting {
            measurement: "foot_slide",
            setting: "contact_height",
            requirement: "finite and non-negative",
        });
    }
    for (frame_index, point) in foot.iter().enumerate() {
        validate_frame("foot_slide", frame_index, std::slice::from_ref(point))?;
    }
    let lowest = foot
        .iter()
        .map(|point| point[1])
        .fold(f32::INFINITY, f32::min);
    let in_contact = |point: &[f32; 3]| point[1] <= lowest + contact_height;
    let contact_frames = foot.iter().filter(|point| in_contact(point)).count();
    let mut total_speed = 0.0;
    let mut pair_count = 0;
    let mut max_speed: f32 = 0.0;
    for pair in foot.windows(2) {
        if in_contact(&pair[0]) && in_contact(&pair[1]) {
            let dx = pair[1][0] - pair[0][0];
            let dz = pair[1][2] - pair[0][2];
            let speed = (dx * dx + dz * dz).sqrt() / dt;
            total_speed += speed;
            pair_count += 1;
            max_speed = max_speed.max(speed);
        }
    }
    let mean_speed = if pair_count == 0 {
        0.0
    } else {
        total_speed / pair_count as f32
    };
    Ok(FootSlide {
        contact_frames,
        mean_speed,
        max_speed,
    })
}

/// Measures the end-to-start gap and typical step size for consistently tracked points.
///
/// # Errors
/// Returns [`MeasureError`] if there are fewer than two frames, point counts differ, a frame has
/// no points, or any coordinate is non-finite.
pub fn loop_seam(frames: &[Vec<[f32; 3]>]) -> Result<LoopSeam, MeasureError> {
    if frames.len() < 2 {
        return Err(MeasureError::MotionFrames {
            measurement: "loop_seam",
            frames: frames.len(),
        });
    }
    let point_count = frames[0].len();
    if point_count == 0 {
        return Err(MeasureError::MotionNoPoints);
    }
    for (frame_index, frame) in frames.iter().enumerate() {
        if frame.len() != point_count {
            return Err(MeasureError::MotionPointCount {
                frame: frame_index,
                actual: frame.len(),
                expected: point_count,
            });
        }
        validate_frame("loop_seam", frame_index, frame)?;
    }
    let last = &frames[frames.len() - 1];
    let mut gap: f32 = 0.0;
    for (first, final_point) in frames[0].iter().zip(last) {
        gap = gap.max(distance(first, final_point));
    }
    let mut total_step = 0.0;
    for pair in frames.windows(2) {
        for (before, after) in pair[0].iter().zip(&pair[1]) {
            total_step += distance(before, after);
        }
    }
    let steps = (frames.len() - 1) * point_count;
    Ok(LoopSeam {
        gap,
        typical_step: total_step / steps as f32,
    })
}

/// Measures the largest proportional change in length between two sampled bone endpoints.
///
/// # Errors
/// Returns [`MeasureError`] if endpoint sample counts differ, there are fewer than two frames, the
/// first-frame bone length is zero, or any coordinate is non-finite.
pub fn length_drift(a: &[[f32; 3]], b: &[[f32; 3]]) -> Result<f32, MeasureError> {
    if a.len() != b.len() {
        return Err(MeasureError::MotionLengthMismatch {
            a: a.len(),
            b: b.len(),
        });
    }
    if a.len() < 2 {
        return Err(MeasureError::MotionFrames {
            measurement: "length_drift",
            frames: a.len(),
        });
    }
    for (frame_index, (first, second)) in a.iter().zip(b).enumerate() {
        validate_frame("length_drift", frame_index, &[*first, *second])?;
    }
    let reference = distance(&a[0], &b[0]);
    if reference == 0.0 {
        return Err(MeasureError::MotionZeroLength);
    }
    let mut drift: f32 = 0.0;
    for (first, second) in a.iter().zip(b) {
        drift = drift.max((distance(first, second) - reference).abs() / reference);
    }
    Ok(drift)
}

fn validate_frame(
    measurement: &'static str,
    frame: usize,
    points: &[[f32; 3]],
) -> Result<(), MeasureError> {
    for (point_index, point) in points.iter().enumerate() {
        if point.iter().any(|coordinate| !coordinate.is_finite()) {
            return Err(MeasureError::MotionNonFinite {
                measurement,
                frame,
                point: point_index,
            });
        }
    }
    Ok(())
}

fn distance(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let dz = b[2] - a[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}
