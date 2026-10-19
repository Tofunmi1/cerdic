//! 1D blend weights and step-size independence.

use crate::machine::Machine;
use crate::preset::{IDLE, RUN, WALK};

use super::{blend_graph, lengths, time_of, total, weight};

#[test]
fn blend1d_interpolates_and_normalizes() {
    let graph = blend_graph();
    graph.validate().unwrap();
    let mut machine = Machine::new(graph, &lengths());
    let cases = [
        (0.0_f32, [1.0_f32, 0.0, 0.0]),
        (0.7, [0.5, 0.5, 0.0]),
        (1.4, [0.0, 1.0, 0.0]),
        (2.7, [0.0, 0.5, 0.5]),
        (4.0, [0.0, 0.0, 1.0]),
        (9.0, [0.0, 0.0, 1.0]),
    ];
    for (speed, expected) in cases {
        machine.set_float("speed", speed);
        let pose = machine.step(0.0);
        assert!(
            (weight(&pose, IDLE) - expected[0]).abs() < 1e-4,
            "speed {speed}"
        );
        assert!(
            (weight(&pose, WALK) - expected[1]).abs() < 1e-4,
            "speed {speed}"
        );
        assert!(
            (weight(&pose, RUN) - expected[2]).abs() < 1e-4,
            "speed {speed}"
        );
        assert!((total(&pose) - 1.0).abs() < 1e-4, "speed {speed}");
    }
}

#[test]
fn one_big_step_matches_sixty_small_ones() {
    let mut big = Machine::new(blend_graph(), &lengths());
    big.set_float("speed", 2.7);
    let pose_big = big.step(1.0);

    let mut small = Machine::new(blend_graph(), &lengths());
    small.set_float("speed", 2.7);
    let mut pose_small = small.step(0.0);
    for _ in 0..60 {
        pose_small = small.step(1.0 / 60.0);
    }

    for clip in [IDLE, WALK, RUN] {
        assert!(
            (weight(&pose_big, clip) - weight(&pose_small, clip)).abs() < 1e-3,
            "{clip}"
        );
    }
    assert!((time_of(&pose_big, IDLE) - time_of(&pose_small, IDLE)).abs() < 1e-3);
    assert!((time_of(&pose_big, RUN) - time_of(&pose_small, RUN)).abs() < 1e-3);
}
