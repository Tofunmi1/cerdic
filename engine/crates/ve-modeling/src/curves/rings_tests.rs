#![expect(
    clippy::cast_precision_loss,
    reason = "tests build fixed, valid paths and index small arrays"
)]
use super::{MAX_TURN_DEGREES, TooSharp, rings, transport};
use crate::curves::vec3::{cross, dot, length, sub};
use std::f64::consts::TAU;

const CLOSE: f64 = 1.0e-9;

/// `turns` turns of a helix of radius 1 and pitch 0.5, sampled at `per_turn` points per turn.
fn helix(turns: usize, per_turn: usize) -> Vec<[f64; 3]> {
    (0..=turns * per_turn)
        .map(|i| {
            let angle = TAU * i as f64 / per_turn as f64;
            [angle.cos(), angle.sin(), 0.5 * angle / TAU]
        })
        .collect()
}

#[test]
fn every_ring_has_orthonormal_axes() {
    for ring in rings(&helix(3, 24), false).unwrap() {
        assert!((length(ring.normal) - 1.0).abs() < CLOSE);
        assert!((length(ring.binormal) - 1.0).abs() < CLOSE);
        assert!((length(ring.tangent) - 1.0).abs() < CLOSE);
        assert!(dot(ring.normal, ring.tangent).abs() < CLOSE);
        assert!(dot(ring.binormal, ring.tangent).abs() < CLOSE);
        assert!(dot(ring.normal, ring.binormal).abs() < CLOSE);
    }
}

#[test]
fn a_helix_accumulates_no_twist_between_rings() {
    // Carrying ring i's normal along the bend to ring i + 1 gives ring i + 1's normal exactly: nothing is twisted about the
    // tangent beyond what the bend itself does.
    let all = rings(&helix(3, 24), false).unwrap();
    for pair in all.windows(2) {
        let carried = transport(pair[0].normal, pair[0].tangent, pair[1].tangent);
        let gap = length(sub(carried, pair[1].normal));
        assert!(gap < CLOSE, "twist of {gap} between neighbouring rings");
    }
}

#[test]
fn the_twist_left_on_a_closed_loop_is_spread_evenly() {
    // A closed, non-planar loop (a tilted figure of eight) returns with its frame turned about the tangent. Every step,
    // including the one from the last ring back to the first, must show the same small residual.
    let count = 48;
    let path: Vec<[f64; 3]> = (0..count)
        .map(|i| {
            let t = TAU * i as f64 / count as f64;
            [
                t.sin(),
                (2.0 * t).sin() * 0.5,
                t.cos() * 0.6 + (3.0 * t).sin() * 0.2,
            ]
        })
        .collect();
    let all = rings(&path, true).unwrap();
    let step = |from: usize, to: usize| {
        let carried = transport(all[from].normal, all[from].tangent, all[to].tangent);
        dot(cross(carried, all[to].normal), all[to].tangent).atan2(dot(carried, all[to].normal))
    };
    let first = step(0, 1);
    for i in 0..count {
        let residual = step(i, (i + 1) % count);
        assert!(
            (residual - first).abs() < 1.0e-6,
            "step {i} turns {residual}, step 0 turns {first}"
        );
    }
    assert!(
        first.abs() > 1.0e-6,
        "the loop was meant to leave a twist to spread"
    );
}

#[test]
fn a_planar_closed_loop_has_no_residual_twist() {
    let count = 24;
    let path: Vec<[f64; 3]> = (0..count)
        .map(|i| {
            let t = TAU * i as f64 / count as f64;
            [t.cos(), t.sin(), 0.0]
        })
        .collect();
    let all = rings(&path, true).unwrap();
    let carried = transport(
        all[count - 1].normal,
        all[count - 1].tangent,
        all[0].tangent,
    );
    assert!(length(sub(carried, all[0].normal)) < 1.0e-9);
}

#[test]
fn a_right_angle_bend_is_mitred_along_the_bend_only() {
    let radius = 0.05;
    let all = rings(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]], false).unwrap();
    let corner = &all[1];
    let half = std::f64::consts::FRAC_1_SQRT_2;
    // The ring faces the bisector of the two directions.
    assert!((corner.tangent[0] - half).abs() < CLOSE && (corner.tangent[1] - half).abs() < CLOSE);
    assert!((corner.stretch - 1.0 / (std::f64::consts::FRAC_PI_4).cos()).abs() < CLOSE);
    let (mut along_bend, mut across_bend) = (0.0_f64, 0.0_f64);
    for step in 0..720 {
        let offset = corner.offset(TAU * f64::from(step) / 720.0, radius);
        assert!(
            dot(offset, corner.tangent).abs() < 1.0e-12,
            "the ring lies in the bisecting plane"
        );
        along_bend = along_bend.max(dot(offset, corner.miter).abs());
        across_bend = across_bend.max(offset[2].abs());
    }
    // Along the bend the ring is as wide as the tube needs to be (radius / cos 45 degrees); out of the bend plane it is not.
    assert!((along_bend - radius / (std::f64::consts::FRAC_PI_4).cos()).abs() < 1.0e-6);
    assert!((across_bend - radius).abs() < 1.0e-6);
}

#[test]
fn a_straight_run_is_not_stretched() {
    let all = rings(&[[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 2.0, 0.0]], false).unwrap();
    for ring in &all {
        assert!((ring.stretch - 1.0).abs() < CLOSE);
        for step in 0..16 {
            let offset = ring.offset(TAU * f64::from(step) / 16.0, 0.3);
            assert!((length(offset) - 0.3).abs() < CLOSE);
        }
    }
}

#[test]
fn turns_sharper_than_the_limit_are_refused() {
    let corner = |degrees: f64| {
        let turn = degrees.to_radians();
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0 + turn.cos(), turn.sin(), 0.0],
        ]
    };
    assert!(rings(&corner(MAX_TURN_DEGREES - 1.0), false).is_ok());
    assert_eq!(
        rings(&corner(MAX_TURN_DEGREES + 1.0), false).err(),
        Some(TooSharp)
    );
    // A hairpin: the path goes out and comes straight back.
    assert_eq!(
        rings(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]], false).err(),
        Some(TooSharp)
    );
}
