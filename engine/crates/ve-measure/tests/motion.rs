//! Motion measurements on sampled animation: foot sliding, loop seams and bone stretch. A model that writes a walk cannot see it, so these
//! turn a sampled clip into numbers it can act on (the graph node `check.motion` that carries them is a later job). Pure functions on plain
//! arrays, like the rest of this crate. Written before the body; do not weaken.
#![expect(
    clippy::cast_precision_loss,
    reason = "fixtures build frames from small indices"
)]
use ve_measure::motion::{foot_slide, length_drift, loop_seam};

const DT: f32 = 1.0 / 60.0;

/// A foot that stands on the ground (y = 0) for 30 frames at `slide` m/s along x, then lifts and swings for 30 frames, repeated twice.
fn foot(slide: f32) -> Vec<[f32; 3]> {
    let mut points = Vec::new();
    let mut x = 0.0;
    for _ in 0..2 {
        for _ in 0..30 {
            points.push([x, 0.0, 0.0]);
            x += slide * DT;
        }
        for k in 0..30 {
            let t = k as f32 / 29.0;
            points.push([x, 0.25 * (t * std::f32::consts::PI).sin().abs() + 0.05, 0.0]);
        }
    }
    points
}

#[test]
fn a_planted_foot_does_not_slide_and_a_skating_foot_does() {
    let planted = foot_slide(&foot(0.0), DT, 0.02).unwrap();
    assert!(
        planted.contact_frames >= 30,
        "{} contact frames",
        planted.contact_frames
    );
    assert!(
        planted.max_speed < 1e-4,
        "a planted foot moved at {} m/s",
        planted.max_speed
    );
    let skating = foot_slide(&foot(0.5), DT, 0.02).unwrap();
    assert!(
        (skating.mean_speed - 0.5).abs() < 0.02,
        "mean {}",
        skating.mean_speed
    );
    assert!(
        (skating.max_speed - 0.5).abs() < 0.02,
        "max {}",
        skating.max_speed
    );
}

#[test]
fn only_horizontal_motion_counts_as_sliding() {
    // A foot sinking into and rising out of the ground while staying put is not skating.
    let bobbing: Vec<[f32; 3]> = (0..60)
        .map(|i| [1.0, 0.005 * ((i % 4) as f32), 2.0])
        .collect();
    let report = foot_slide(&bobbing, DT, 0.02).unwrap();
    assert!(report.max_speed < 1e-4, "{}", report.max_speed);
    // Sliding along z counts like sliding along x.
    let along_z: Vec<[f32; 3]> = (0..60).map(|i| [0.0, 0.0, i as f32 * DT]).collect();
    let report = foot_slide(&along_z, DT, 0.02).unwrap();
    assert!(
        (report.mean_speed - 1.0).abs() < 0.02,
        "{}",
        report.mean_speed
    );
}

#[test]
fn a_foot_in_the_air_all_the_time_has_a_lowest_point_in_contact() {
    // Contact is relative to the lowest point of the track, so a character walking on a raised floor is measured the same way.
    let raised: Vec<[f32; 3]> = foot(0.0).iter().map(|p| [p[0], p[1] + 3.0, p[2]]).collect();
    let report = foot_slide(&raised, DT, 0.02).unwrap();
    assert!(report.contact_frames >= 30);
    assert!(report.max_speed < 1e-4);
}

#[test]
fn foot_slide_refuses_bad_input() {
    assert!(foot_slide(&[], DT, 0.02).is_err());
    assert!(
        foot_slide(&[[0.0; 3]], DT, 0.02).is_err(),
        "one frame has no speed"
    );
    assert!(foot_slide(&foot(0.0), 0.0, 0.02).is_err(), "no time step");
    assert!(foot_slide(&foot(0.0), -1.0, 0.02).is_err());
    assert!(
        foot_slide(&foot(0.0), DT, -0.1).is_err(),
        "a negative contact height"
    );
    assert!(
        foot_slide(&[[0.0; 3], [f32::NAN, 0.0, 0.0]], DT, 0.02).is_err(),
        "NaN"
    );
}

/// Two points circling the origin, `frames` samples covering `turns` revolutions.
fn circling(frames: usize, turns: f32) -> Vec<Vec<[f32; 3]>> {
    (0..frames)
        .map(|i| {
            let a = turns * std::f32::consts::TAU * (i as f32) / (frames as f32);
            vec![[a.cos(), 0.0, a.sin()], [2.0 * a.cos(), 1.0, 2.0 * a.sin()]]
        })
        .collect()
}

#[test]
fn a_whole_cycle_loops_without_a_seam() {
    // 60 frames of one full revolution: the last frame sits one step before the first, which is just the next step of the loop.
    let seam = loop_seam(&circling(60, 1.0)).unwrap();
    assert!(
        seam.seamless(),
        "gap {} against a typical step {}",
        seam.gap,
        seam.typical_step
    );
    assert!(seam.typical_step > 0.0);
}

#[test]
fn a_cut_short_cycle_has_a_seam() {
    // Three quarters of a revolution: the loop would jump from there back to the start.
    let seam = loop_seam(&circling(45, 0.75)).unwrap();
    assert!(
        !seam.seamless(),
        "gap {} against a typical step {}",
        seam.gap,
        seam.typical_step
    );
    assert!(seam.gap > 3.0 * seam.typical_step);
}

#[test]
fn a_clip_that_ends_where_it_began_and_a_still_pose_are_seamless() {
    let mut frames = circling(60, 1.0);
    frames.push(frames[0].clone());
    let seam = loop_seam(&frames).unwrap();
    assert!(seam.gap < 1e-6);
    assert!(seam.seamless());
    let still = vec![vec![[1.0, 2.0, 3.0]]; 10];
    let seam = loop_seam(&still).unwrap();
    assert_eq!((seam.gap, seam.typical_step), (0.0, 0.0));
    assert!(seam.seamless());
}

#[test]
fn loop_seam_refuses_bad_input() {
    assert!(loop_seam(&[]).is_err());
    assert!(loop_seam(&[vec![[0.0; 3]]]).is_err(), "one frame");
    assert!(
        loop_seam(&[vec![[0.0; 3]], vec![[0.0; 3], [1.0; 3]]]).is_err(),
        "frames with different point counts"
    );
    assert!(
        loop_seam(&[Vec::new(), Vec::new()]).is_err(),
        "frames with no points"
    );
    assert!(
        loop_seam(&[vec![[0.0; 3]], vec![[f32::INFINITY, 0.0, 0.0]]]).is_err(),
        "infinity"
    );
}

#[test]
fn a_rigid_bone_does_not_drift_and_a_stretching_one_does() {
    // A forearm of length 0.3 swinging about the elbow.
    let elbow: Vec<[f32; 3]> = (0..40).map(|_| [0.0, 1.0, 0.0]).collect();
    let hand = |scale: &dyn Fn(usize) -> f32| -> Vec<[f32; 3]> {
        (0..40)
            .map(|i| {
                let a = i as f32 * 0.1;
                let l = 0.3 * scale(i);
                [l * a.sin(), 1.0 - l * a.cos(), 0.0]
            })
            .collect()
    };
    let rigid = length_drift(&elbow, &hand(&|_| 1.0)).unwrap();
    assert!(rigid < 1e-3, "rigid drift {rigid}");
    let stretchy = length_drift(&elbow, &hand(&|i| 1.0 + 0.1 * (i as f32) / 39.0)).unwrap();
    assert!(
        (stretchy - 0.1).abs() < 0.005,
        "drift {stretchy}, expected about 0.1"
    );
}

#[test]
fn length_drift_refuses_bad_input() {
    let a: Vec<[f32; 3]> = vec![[0.0; 3]; 5];
    assert!(
        length_drift(&a, &a).is_err(),
        "a bone of no length has no drift"
    );
    assert!(
        length_drift(&a, &[[1.0; 3]; 4]).is_err(),
        "different frame counts"
    );
    assert!(length_drift(&[], &[]).is_err());
    assert!(length_drift(&[[0.0; 3]], &[[1.0; 3]]).is_err(), "one frame");
    assert!(length_drift(&a, &[[f32::NAN, 0.0, 0.0]; 5]).is_err());
}
