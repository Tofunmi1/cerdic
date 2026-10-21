//! Procedural gait for any number of legs: where each foot plants and how it swings, from the body's motion and the ground height.
//! The numbers are worked out by hand from the behaviour in `docs/handoff-codex-gait.md`.
use ve_gait::{BodyPose, Gait, GaitError, GaitSpec, Leg};

const DT: f64 = 1.0 / 60.0;

fn flat(_x: f64, _z: f64) -> f64 {
    0.0
}

fn slope(x: f64, _z: f64) -> f64 {
    0.2 * x
}

/// Eight legs around a body, hips at height 0.75, alternating phases 0 and 0.5, feet 1.15 m out from the body centre.
fn spider() -> GaitSpec {
    let legs = (0..8)
        .map(|i| {
            let a = (f64::from(i) + 0.5) / 8.0 * std::f64::consts::TAU;
            Leg {
                hip: [0.3 * a.cos(), 0.0, 0.3 * a.sin()],
                rest_foot: [1.15 * a.cos(), 0.0, 1.15 * a.sin()],
                reach: 1.3,
                phase: if i % 2 == 0 { 0.0 } else { 0.5 },
            }
        })
        .collect();
    GaitSpec {
        legs,
        stride_length: 0.6,
        step_height: 0.18,
        duty: 0.6,
    }
}

fn at(x: f64, y: f64, z: f64) -> BodyPose {
    BodyPose {
        position: [x, y, z],
        yaw: 0.0,
    }
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn a_standing_body_plants_every_foot_at_its_rest_place_on_the_ground() {
    let spec = spider();
    let mut gait = Gait::new(spec.clone()).unwrap();
    for _ in 0..120 {
        gait.step(at(2.0, 0.75, -1.0), [0.0; 3], DT, &flat);
    }
    for (leg, foot) in spec.legs.iter().zip(gait.feet()) {
        assert!(foot.planted, "a standing body keeps its feet down");
        let expected = [2.0 + leg.rest_foot[0], 0.0, -1.0 + leg.rest_foot[2]];
        assert!(
            dist(foot.position, expected) < 1e-9,
            "{:?} against {expected:?}",
            foot.position
        );
    }
}

#[test]
fn a_planted_foot_does_not_slide_while_the_body_walks() {
    let mut gait = Gait::new(spider()).unwrap();
    let mut x = 0.0;
    let mut previous = gait.feet().to_vec();
    let mut checked = 0;
    for _ in 0..360 {
        x += 1.0 * DT;
        gait.step(at(x, 0.75, 0.0), [1.0, 0.0, 0.0], DT, &flat);
        for (before, now) in previous.iter().zip(gait.feet()) {
            if before.planted && now.planted {
                assert!(
                    dist(before.position, now.position) < 1e-9,
                    "a planted foot moved: {:?} to {:?}",
                    before.position,
                    now.position
                );
                checked += 1;
            }
        }
        previous = gait.feet().to_vec();
    }
    assert!(
        checked > 1000,
        "planted feet were actually compared ({checked})"
    );
}

#[test]
fn no_foot_is_ever_farther_from_its_hip_than_its_reach() {
    let spec = spider();
    let mut gait = Gait::new(spec.clone()).unwrap();
    let mut x = 0.0;
    for _ in 0..600 {
        x += 1.6 * DT;
        gait.step(at(x, 0.75, 0.0), [1.6, 0.0, 0.0], DT, &flat);
        for (leg, foot) in spec.legs.iter().zip(gait.feet()) {
            // The hip is the body's position plus its offset (yaw 0 here).
            let hip = [x + leg.hip[0], 0.75 + leg.hip[1], leg.hip[2]];
            assert!(
                dist(hip, foot.position) <= leg.reach + 1e-6,
                "foot out of reach: {:?} from hip {hip:?}",
                foot.position
            );
        }
    }
}

#[test]
fn at_least_half_the_feet_are_down_at_every_moment_with_alternating_phases() {
    let mut gait = Gait::new(spider()).unwrap();
    let mut x = 0.0;
    let mut least = usize::MAX;
    for step in 0..600 {
        x += 1.0 * DT;
        gait.step(at(x, 0.75, 0.0), [1.0, 0.0, 0.0], DT, &flat);
        if step > 60 {
            least = least.min(gait.feet().iter().filter(|foot| foot.planted).count());
        }
    }
    assert!(
        least >= 4,
        "only {least} feet were down at the worst moment"
    );
}

#[test]
fn planted_feet_sit_on_sloping_ground_and_swinging_feet_lift_to_the_step_height() {
    let spec = spider();
    let mut gait = Gait::new(spec.clone()).unwrap();
    let mut x = 0.0;
    let mut highest_lift: f64 = 0.0;
    for _ in 0..600 {
        x += 1.0 * DT;
        gait.step(at(x, 0.75 + 0.2 * x, 0.0), [1.0, 0.2, 0.0], DT, &slope);
        for foot in gait.feet() {
            let ground = slope(foot.position[0], foot.position[2]);
            if foot.planted {
                assert!(
                    (foot.position[1] - ground).abs() < 1e-9,
                    "a planted foot floats or sinks: {} against {ground}",
                    foot.position[1]
                );
            } else {
                highest_lift = highest_lift.max(foot.position[1] - ground);
            }
        }
    }
    assert!(
        highest_lift >= 0.9 * spec.step_height,
        "swinging feet lifted only {highest_lift} m of {}",
        spec.step_height
    );
    assert!(
        highest_lift <= 1.05 * spec.step_height,
        "and not above the step height: {highest_lift}"
    );
}

#[test]
fn the_same_motion_gives_bit_identical_feet() {
    let run = || {
        let mut gait = Gait::new(spider()).unwrap();
        for step in 0..200 {
            let t = f64::from(step) * DT;
            gait.step(
                at(t, 0.75, (t * 2.0).sin()),
                [1.0, 0.0, 2.0 * (t * 2.0).cos()],
                DT,
                &flat,
            );
        }
        gait.feet()
            .iter()
            .flat_map(|f| f.position)
            .map(f64::to_bits)
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}

#[test]
fn a_turning_body_carries_the_rest_places_round_with_its_yaw() {
    let spec = spider();
    let mut gait = Gait::new(spec.clone()).unwrap();
    let pose = BodyPose {
        position: [0.0, 0.75, 0.0],
        yaw: std::f64::consts::FRAC_PI_2,
    };
    for _ in 0..120 {
        gait.step(pose, [0.0; 3], DT, &flat);
    }
    // A quarter turn about +y takes (x, z) to (z, -x).
    for (leg, foot) in spec.legs.iter().zip(gait.feet()) {
        let expected = [leg.rest_foot[2], 0.0, -leg.rest_foot[0]];
        assert!(
            dist(foot.position, expected) < 1e-9,
            "{:?} against {expected:?}",
            foot.position
        );
    }
}

#[test]
fn bad_specs_are_refused_with_a_typed_error() {
    let ok = spider();
    let mut none = ok.clone();
    none.legs.clear();
    assert!(matches!(Gait::new(none), Err(GaitError::NoLegs)));
    let mut stride = ok.clone();
    stride.stride_length = 0.0;
    assert!(matches!(
        Gait::new(stride),
        Err(GaitError::OutOfRange("stride_length"))
    ));
    let mut duty = ok.clone();
    duty.duty = 1.2;
    assert!(matches!(
        Gait::new(duty),
        Err(GaitError::OutOfRange("duty"))
    ));
    let mut nan = ok.clone();
    nan.legs[0].reach = f64::NAN;
    assert!(matches!(Gait::new(nan), Err(GaitError::NotFinite(_))));
    let mut many = ok;
    many.legs = (0..40).map(|_| many.legs[0].clone()).collect();
    assert!(matches!(
        Gait::new(many),
        Err(GaitError::TooManyLegs { max: 32 })
    ));
}
