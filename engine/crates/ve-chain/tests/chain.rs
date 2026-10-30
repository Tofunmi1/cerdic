//! Secondary motion for tails, ears, hair and cloth edges: a pinned Verlet chain that follows its root with lag and settles back to its rest pose.
//! Every number is worked out by hand from the behaviour in `docs/handoff-codex-chain.md`.
use ve_chain::{Chain, ChainError, ChainSpec, RootPose};

const DT: f64 = 1.0 / 60.0;

fn pose(x: f64, y: f64, z: f64) -> RootPose {
    RootPose {
        position: [x, y, z],
        rotation: [0.0, 0.0, 0.0, 1.0],
    }
}

/// A four-segment tail hanging along -z from the root, each segment 0.5 m.
fn tail(stiffness: f64, gravity: f64) -> ChainSpec {
    ChainSpec {
        rest: vec![[0.0, 0.0, -0.5]; 4],
        stiffness,
        damping: 4.0,
        gravity: [0.0, -gravity, 0.0],
        iterations: 8,
    }
}

fn run(chain: &mut Chain, root: RootPose, steps: usize) {
    for _ in 0..steps {
        chain.step(root, DT);
    }
}

fn length(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn a_chain_at_rest_with_no_gravity_stays_exactly_at_rest() {
    let mut chain = Chain::new(tail(0.5, 0.0), pose(1.0, 2.0, 3.0)).unwrap();
    run(&mut chain, pose(1.0, 2.0, 3.0), 200);
    let points = chain.points();
    assert_eq!(points.len(), 5, "the root plus four segments");
    for (i, point) in points.iter().enumerate() {
        let expected = [1.0, 2.0, [3.0, 2.5, 2.0, 1.5, 1.0][i]];
        assert!(
            length(*point, expected) < 1e-9,
            "point {i}: {point:?} against {expected:?}"
        );
    }
}

#[test]
fn segment_lengths_are_kept_through_violent_root_motion() {
    let mut chain = Chain::new(tail(0.3, 9.8), pose(0.0, 0.0, 0.0)).unwrap();
    for step in 0..240 {
        let t = f64::from(step) * DT;
        chain.step(
            pose(
                3.0 * (t * 9.0).sin(),
                2.0 * (t * 7.0).cos(),
                1.0 * (t * 5.0).sin(),
            ),
            DT,
        );
        let points = chain.points();
        for pair in points.windows(2) {
            let d = length(pair[0], pair[1]);
            assert!(
                (d - 0.5).abs() < 1e-3,
                "step {step}: segment is {d} m, not 0.5"
            );
        }
    }
}

#[test]
fn the_tip_lags_a_moving_root_and_catches_up_when_it_stops() {
    let mut chain = Chain::new(tail(0.4, 0.0), pose(0.0, 0.0, 0.0)).unwrap();
    // The root walks along +x at 2 m/s for one second: the tip trails behind it in x.
    let mut x = 0.0;
    for _ in 0..60 {
        x += 2.0 * DT;
        chain.step(pose(x, 0.0, 0.0), DT);
    }
    let tip = *chain.points().last().unwrap();
    assert!(
        tip[0] < x - 0.05,
        "the tip ({}) trails the root ({x})",
        tip[0]
    );
    // The root stops; with stiffness the chain springs back to hang along -z from it within 5 seconds.
    run(&mut chain, pose(x, 0.0, 0.0), 300);
    let tip = *chain.points().last().unwrap();
    assert!(length(tip, [x, 0.0, -2.0]) < 1e-3, "tip settled at {tip:?}");
}

#[test]
fn without_stiffness_gravity_hangs_the_chain_straight_down() {
    let mut chain = Chain::new(tail(0.0, 9.8), pose(0.0, 5.0, 0.0)).unwrap();
    run(&mut chain, pose(0.0, 5.0, 0.0), 900);
    let points = chain.points();
    for pair in points.windows(2) {
        assert!(
            pair[1][1] < pair[0][1] - 0.4,
            "each point hangs below the one before: {pair:?}"
        );
    }
    let tip = *points.last().unwrap();
    assert!(
        (tip[1] - 3.0).abs() < 1e-2,
        "the tip hangs the full 2 m below the root, y = {}",
        tip[1]
    );
    assert!(
        tip[0].abs() < 1e-2 && tip[2].abs() < 1e-2,
        "straight down: {tip:?}"
    );
}

#[test]
fn a_rotated_root_carries_the_rest_pose_with_it() {
    // A quarter turn about +y takes -z to -x (right-handed): the rest chain then hangs along -x.
    let half = std::f64::consts::FRAC_1_SQRT_2;
    let turned = RootPose {
        position: [0.0, 0.0, 0.0],
        rotation: [0.0, half, 0.0, half],
    };
    let mut chain = Chain::new(tail(0.5, 0.0), turned).unwrap();
    run(&mut chain, turned, 100);
    let tip = *chain.points().last().unwrap();
    assert!(length(tip, [-2.0, 0.0, 0.0]) < 1e-6, "tip at {tip:?}");
}

#[test]
fn the_same_inputs_give_bit_identical_results() {
    let run_once = || {
        let mut chain = Chain::new(tail(0.3, 9.8), pose(0.0, 0.0, 0.0)).unwrap();
        for step in 0..120 {
            let t = f64::from(step) * DT;
            chain.step(pose((t * 6.0).sin(), (t * 4.0).cos(), 0.0), DT);
        }
        chain
            .points()
            .iter()
            .flatten()
            .map(|v| v.to_bits())
            .collect::<Vec<_>>()
    };
    assert_eq!(run_once(), run_once());
}

#[test]
fn a_huge_time_step_does_not_blow_the_chain_up() {
    let mut chain = Chain::new(tail(0.3, 9.8), pose(0.0, 0.0, 0.0)).unwrap();
    for _ in 0..10 {
        chain.step(pose(100.0, 0.0, 0.0), 5.0);
    }
    for point in chain.points() {
        assert!(point.iter().all(|v| v.is_finite()), "{point:?}");
    }
    for pair in chain.points().windows(2) {
        assert!((length(pair[0], pair[1]) - 0.5).abs() < 1e-3);
    }
}

#[test]
fn bad_specs_are_refused_with_a_typed_error() {
    let ok = tail(0.5, 9.8);
    let root = pose(0.0, 0.0, 0.0);
    let mut empty = ok.clone();
    empty.rest.clear();
    assert!(matches!(Chain::new(empty, root), Err(ChainError::Empty)));
    let mut zero = ok.clone();
    zero.rest[1] = [0.0, 0.0, 0.0];
    assert!(matches!(
        Chain::new(zero, root),
        Err(ChainError::ZeroLength { index: 1 })
    ));
    let mut nan = ok.clone();
    nan.stiffness = f64::NAN;
    assert!(matches!(
        Chain::new(nan, root),
        Err(ChainError::NotFinite(_))
    ));
    let mut long = ok.clone();
    long.rest = vec![[0.0, 0.0, -0.1]; 65];
    assert!(matches!(
        Chain::new(long, root),
        Err(ChainError::TooLong { max: 64 })
    ));
    let mut stiff = ok;
    stiff.stiffness = 1.5;
    assert!(matches!(
        Chain::new(stiff, root),
        Err(ChainError::OutOfRange("stiffness"))
    ));
}
