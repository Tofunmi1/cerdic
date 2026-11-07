//! Analytic limb IK (plan W2): the two-bone solver (with pole vector, twist, soften and weight) and the aim solver.
//! Written before the bodies. The checks are geometric (reach, bone length, plane, continuity), not golden numbers.
use glam::{DQuat, DVec3};
use ve_modeling::ik_limb::{Aim, AimGoal, TwoBone, TwoBoneGoal, aim, two_bone};

const EPS: f64 = 1.0e-9;

/// A shoulder at the origin, an elbow bent a little towards +Y (so the bend plane is unambiguous), a wrist beyond: bones 1.0 and 1.2 long.
fn arm() -> TwoBone {
    let upper = 1.0;
    let lower = 1.2;
    let bend = 0.3_f64;
    let mid = DVec3::new(upper * bend.cos(), upper * bend.sin(), 0.0);
    let end = mid + DVec3::new(lower, 0.0, 0.0);
    TwoBone {
        start: DVec3::ZERO,
        mid,
        end,
    }
}

fn goal(target: DVec3) -> TwoBoneGoal {
    TwoBoneGoal {
        target,
        pole: DVec3::Y,
        soften: 0.0,
        twist: 0.0,
        weight: 1.0,
    }
}

fn lengths(chain: &TwoBone) -> (f64, f64) {
    (
        chain.mid.distance(chain.start),
        chain.end.distance(chain.mid),
    )
}

#[test]
fn a_reachable_target_is_reached_and_bones_keep_their_length() {
    let chain = arm();
    let (upper, lower) = lengths(&chain);
    for target in [
        DVec3::new(1.5, 0.5, 0.0),
        DVec3::new(0.5, 1.0, 1.0),
        DVec3::new(-1.0, 0.8, 0.3),
        DVec3::new(0.0, -1.5, 0.5),
        DVec3::new(0.3, 0.2, 0.2),
    ] {
        let solution = two_bone(&chain, &goal(target));
        assert!(solution.reached, "{target:?} is within reach");
        let (mid, end) = solution.pose(&chain);
        assert!(
            end.distance(target) < 1.0e-8,
            "end misses {target:?}: {end:?}"
        );
        assert!((mid.distance(chain.start) - upper).abs() < 1.0e-8);
        assert!((end.distance(mid) - lower).abs() < 1.0e-8);
    }
}

#[test]
fn a_target_out_of_reach_leaves_the_arm_straight_and_pointing_at_it() {
    let chain = arm();
    let (upper, lower) = lengths(&chain);
    let direction = DVec3::new(2.0, 1.0, -1.0).normalize();
    let solution = two_bone(&chain, &goal(direction * 10.0));
    assert!(!solution.reached);
    let (mid, end) = solution.pose(&chain);
    assert!(end.distance(direction * (upper + lower)) < 1.0e-6);
    assert!(
        mid.distance(direction * upper) < 1.0e-6,
        "straight, not bent"
    );
}

#[test]
fn a_target_too_close_folds_the_arm_without_breaking_it() {
    let chain = arm();
    let (upper, lower) = lengths(&chain);
    // Closer than |upper - lower| cannot be reached at all.
    let target = DVec3::new(0.05, 0.0, 0.0);
    let solution = two_bone(&chain, &goal(target));
    assert!(!solution.reached);
    let (mid, end) = solution.pose(&chain);
    assert!(mid.is_finite() && end.is_finite());
    assert!((mid.distance(chain.start) - upper).abs() < 1.0e-8);
    assert!((end.distance(mid) - lower).abs() < 1.0e-8);
    assert!(
        end.length() < (upper - lower).abs() + 1.0e-3,
        "as close as the bones allow"
    );
}

#[test]
fn the_elbow_bends_towards_the_pole() {
    let chain = arm();
    for pole in [DVec3::Y, -DVec3::Y, DVec3::Z, DVec3::new(1.0, -1.0, 1.0)] {
        let target = DVec3::new(1.4, 0.0, 0.0);
        let solution = two_bone(
            &chain,
            &TwoBoneGoal {
                pole,
                ..goal(target)
            },
        );
        let (mid, _) = solution.pose(&chain);
        let axis = (target - chain.start).normalize();
        let sideways = mid - axis * mid.dot(axis);
        let wanted = pole - axis * pole.dot(axis);
        assert!(
            sideways.normalize().dot(wanted.normalize()) > 0.999,
            "pole {pole:?}: elbow {mid:?}"
        );
    }
}

#[test]
fn sweeping_the_target_around_the_shoulder_never_flips_the_elbow() {
    let chain = arm();
    let mut previous: Option<DVec3> = None;
    for step in 0..720 {
        let angle = f64::from(step) * std::f64::consts::TAU / 720.0;
        // A circle of radius 1.5 around the +Y axis through the shoulder: the target crosses every side but never the pole line.
        let target = DVec3::new(1.5 * angle.cos(), 0.4, 1.5 * angle.sin());
        let (mid, _) = two_bone(&chain, &goal(target)).pose(&chain);
        if let Some(before) = previous {
            assert!(
                mid.distance(before) < 0.05,
                "elbow jumps {} at step {step}",
                mid.distance(before)
            );
        }
        previous = Some(mid);
    }
}

#[test]
fn soften_eases_the_arm_in_before_full_stretch_and_never_overshoots() {
    let chain = arm();
    let (upper, lower) = lengths(&chain);
    let reach = upper + lower;
    let soft = |distance: f64| {
        let solution = two_bone(
            &chain,
            &TwoBoneGoal {
                soften: 0.8,
                ..goal(DVec3::new(distance, 0.0, 0.0))
            },
        );
        solution.pose(&chain).1.length()
    };
    // `soften` 0.8 eases in over the last 80% of the chain's length: below 20% of it the target is hit exactly.
    assert!((soft(0.1 * reach) - 0.1 * reach).abs() < 1.0e-8);
    // Inside the soft zone the arm stops short of the target, never reaches the full length, and keeps growing (no dead zone, no jump).
    let mut last = soft(0.25 * reach);
    for step in 1..=200 {
        let wanted = reach * 0.25 + reach * 0.75 * f64::from(step) / 200.0;
        let got = soft(wanted);
        assert!(
            got <= wanted + EPS,
            "softened reach {got} exceeds the target {wanted}"
        );
        assert!(got < reach, "softened reach {got} touches full stretch");
        assert!(got >= last - 1.0e-9, "reach shrinks at {wanted}");
        assert!(got - last < 0.01, "reach jumps at {wanted}");
        last = got;
    }
}

#[test]
fn weight_zero_does_nothing_and_half_weight_turns_half_way() {
    let chain = arm();
    let target = DVec3::new(0.2, 1.4, 0.3);
    let none = two_bone(
        &chain,
        &TwoBoneGoal {
            weight: 0.0,
            ..goal(target)
        },
    );
    assert!(!none.reached);
    assert!(none.start.angle_between(DQuat::IDENTITY) < EPS);
    assert!(none.mid.angle_between(DQuat::IDENTITY) < EPS);
    let half = two_bone(
        &chain,
        &TwoBoneGoal {
            weight: 0.5,
            ..goal(target)
        },
    );
    // The blend is a normalised lerp (arithmetic only, as ozz does), so half weight is near, not exactly, half the angle.
    let angle = |w: f64| {
        two_bone(
            &chain,
            &TwoBoneGoal {
                weight: w,
                ..goal(target)
            },
        )
        .start
        .angle_between(DQuat::IDENTITY)
    };
    let ratio = angle(0.5) / angle(1.0);
    assert!(
        (0.35..0.65).contains(&ratio),
        "half weight turns {ratio} of the way"
    );
    assert!(angle(0.25) < angle(0.5) && angle(0.5) < angle(0.75) && angle(0.75) < angle(1.0));
    assert!(!half.reached, "a half-weight solve does not claim to reach");
}

#[test]
fn twist_swings_the_elbow_around_the_shoulder_to_target_line() {
    let chain = arm();
    let target = DVec3::new(1.4, 0.0, 0.0);
    let axis = DVec3::X;
    let plain = two_bone(&chain, &goal(target)).pose(&chain).0;
    let twisted = two_bone(
        &chain,
        &TwoBoneGoal {
            twist: std::f64::consts::FRAC_PI_2,
            ..goal(target)
        },
    )
    .pose(&chain)
    .0;
    let flat = |p: DVec3| p - axis * p.dot(axis);
    let angle = flat(plain).angle_between(flat(twisted));
    assert!(
        (angle - std::f64::consts::FRAC_PI_2).abs() < 1.0e-6,
        "twisted by {angle}"
    );
    assert!(
        (plain.dot(axis) - twisted.dot(axis)).abs() < 1.0e-9,
        "twist keeps the elbow's distance along the axis"
    );
}

#[test]
fn degenerate_inputs_stay_finite() {
    let chain = arm();
    for target in [
        chain.start,
        DVec3::new(f64::MIN_POSITIVE, 0.0, 0.0),
        DVec3::new(1.0e12, 0.0, 0.0),
    ] {
        let (mid, end) = two_bone(&chain, &goal(target)).pose(&chain);
        assert!(mid.is_finite() && end.is_finite(), "target {target:?}");
    }
    // A pole along the shoulder-to-target line gives no bend direction: the current bend is kept.
    let target = DVec3::new(1.4, 0.0, 0.0);
    let solution = two_bone(
        &chain,
        &TwoBoneGoal {
            pole: DVec3::X,
            ..goal(target)
        },
    );
    let (mid, end) = solution.pose(&chain);
    assert!(end.distance(target) < 1.0e-8 && mid.is_finite());
    // Zero-length bones do not divide by zero.
    let folded = TwoBone {
        start: DVec3::ZERO,
        mid: DVec3::ZERO,
        end: DVec3::X,
    };
    let (mid, end) = two_bone(&folded, &goal(DVec3::Y)).pose(&folded);
    assert!(mid.is_finite() && end.is_finite());
    // A pole of zero length is ignored.
    let none = two_bone(
        &chain,
        &TwoBoneGoal {
            pole: DVec3::ZERO,
            ..goal(target)
        },
    );
    assert!(none.pose(&chain).1.distance(target) < 1.0e-8);
}

#[test]
fn the_same_input_gives_bit_identical_output() {
    let chain = arm();
    let g = TwoBoneGoal {
        soften: 0.3,
        twist: 0.2,
        weight: 0.9,
        ..goal(DVec3::new(0.7, 0.9, -0.4))
    };
    let (a, b) = (two_bone(&chain, &g), two_bone(&chain, &g));
    assert_eq!(
        a.start.to_array().map(f64::to_bits),
        b.start.to_array().map(f64::to_bits)
    );
    assert_eq!(
        a.mid.to_array().map(f64::to_bits),
        b.mid.to_array().map(f64::to_bits)
    );
}

// ---------- in a chain of bone parts ----------

fn segments(chain: &TwoBone) -> [ve_modeling::rig::Segment; 2] {
    let f = |v: DVec3| v.as_vec3().to_array();
    [
        ve_modeling::rig::Segment {
            head: f(chain.start),
            tail: f(chain.mid),
        },
        ve_modeling::rig::Segment {
            head: f(chain.mid),
            tail: f(chain.end),
        },
    ]
}

#[test]
fn the_two_bone_solver_drives_a_chain_of_bone_parts_hanging_from_a_moving_parent() {
    use glam::DMat4;
    use ve_modeling::ik::{solve_two_bone, tip_after};
    // Segments are the rest pose in world space; the parent is where the part the chain hangs from is NOW, turned and moved from rest.
    let joints = segments(&arm());
    let rest_shoulder = arm().start;
    for parent in [
        DMat4::IDENTITY,
        DMat4::from_rotation_translation(DQuat::from_rotation_y(0.7), DVec3::new(2.0, 0.5, -1.0)),
        DMat4::from_rotation_translation(
            DQuat::from_euler(glam::EulerRot::XYZ, 0.5, -0.4, 1.1),
            DVec3::new(-3.0, 1.0, 0.2),
        ),
    ] {
        let shoulder = parent.transform_point3(rest_shoulder);
        for offset in [
            DVec3::new(0.4, 1.2, 0.3),
            DVec3::new(-1.0, 0.2, 0.9),
            DVec3::new(0.0, -1.0, 1.2),
        ] {
            let target = shoulder + offset;
            let turns = solve_two_bone(&joints, parent, &goal(target));
            assert_eq!(turns.len(), 2);
            let tip = tip_after(&joints, parent, &turns);
            assert!(
                tip.distance(target) < 1.0e-4,
                "tip {tip:?} misses {target:?}"
            );
        }
    }
    // Chains that are not two bones are left alone rather than half-solved.
    let three = [joints[0], joints[1], joints[1]];
    assert!(
        solve_two_bone(&three, DMat4::IDENTITY, &goal(DVec3::Y))
            .iter()
            .all(|q| q.angle_between(DQuat::IDENTITY) < 1.0e-6)
    );
}

#[test]
fn the_aim_solver_turns_a_single_bone_part_to_look_at_its_target_under_a_moving_parent() {
    use glam::DMat4;
    use ve_modeling::ik::{solve_aim, tip_after};
    // A head bone: pivot at the neck (0, 1.7, 0), looking along +Z for 0.2 m.
    let head = [ve_modeling::rig::Segment {
        head: [0.0, 1.7, 0.0],
        tail: [0.0, 1.7, 0.2],
    }];
    let neck = DVec3::new(0.0, 1.7, 0.0);
    for parent in [
        DMat4::IDENTITY,
        DMat4::from_rotation_translation(DQuat::from_rotation_y(1.2), DVec3::new(1.0, 0.0, 2.0)),
    ] {
        let pivot = parent.transform_point3(neck);
        for offset in [
            DVec3::new(3.0, 0.5, 1.0),
            DVec3::new(-2.0, 1.0, -4.0),
            DVec3::new(0.0, 4.0, 0.3),
        ] {
            let goal = AimGoal {
                target: pivot + offset,
                pole: DVec3::Y,
                twist: 0.0,
                weight: 1.0,
            };
            let turns = solve_aim(&head, parent, &goal, DVec3::Y);
            assert_eq!(turns.len(), 1);
            let tip = tip_after(&head, parent, &turns);
            let looking = (tip - pivot).normalize();
            assert!(
                looking.dot(offset.normalize()) > 1.0 - 1.0e-9,
                "looks along {looking:?}, not at {offset:?}"
            );
        }
    }
    // More than one bone is not an aim: left at rest.
    let two = [head[0], head[0]];
    assert!(
        solve_aim(&two, DMat4::IDENTITY, &look(DVec3::X), DVec3::Y)
            .iter()
            .all(|q| q.angle_between(DQuat::IDENTITY) < 1.0e-6)
    );
}

// ---------- aim ----------

fn head(rotation: DQuat) -> Aim {
    // A head whose local +Z looks forward and +Y is up.
    Aim {
        position: DVec3::new(0.0, 1.7, 0.0),
        rotation,
        forward: DVec3::Z,
        up: DVec3::Y,
    }
}

fn look(target: DVec3) -> AimGoal {
    AimGoal {
        target,
        pole: DVec3::Y,
        twist: 0.0,
        weight: 1.0,
    }
}

#[test]
fn aim_turns_the_forward_axis_onto_the_target_and_keeps_the_head_upright() {
    for rotation in [
        DQuat::IDENTITY,
        DQuat::from_rotation_y(2.0),
        DQuat::from_euler(glam::EulerRot::XYZ, 0.4, -1.1, 0.7),
    ] {
        for target in [
            DVec3::new(3.0, 2.0, 1.0),
            DVec3::new(-2.0, 1.0, -4.0),
            DVec3::new(0.5, 1.7, 3.0),
            DVec3::new(0.0, 5.0, 0.4),
        ] {
            let joint = head(rotation);
            let solution = aim(&joint, &look(target));
            assert!(solution.reached);
            let turned = solution.correction * rotation;
            let forward = turned * joint.forward;
            let wanted = (target - joint.position).normalize();
            assert!(
                forward.dot(wanted) > 1.0 - 1.0e-9,
                "{target:?}: forward {forward:?}"
            );
            let up = turned * joint.up;
            // Upright: the up axis leans towards the pole and lies in the plane of forward and pole.
            assert!(up.dot(DVec3::Y) > 0.0);
            assert!(
                up.dot(forward.cross(DVec3::Y)).abs() < 1.0e-9,
                "up is out of the forward-pole plane"
            );
            assert!((turned.length() - 1.0).abs() < 1.0e-9);
        }
    }
}

#[test]
fn aim_at_a_target_already_in_front_changes_nothing() {
    let joint = head(DQuat::IDENTITY);
    let solution = aim(&joint, &look(joint.position + DVec3::new(0.0, 0.0, 4.0)));
    // `angle_between` goes through acos, which cannot resolve better than about 1e-8 near zero, hence the tolerance.
    assert!(solution.correction.angle_between(DQuat::IDENTITY) < 1.0e-6);
}

#[test]
fn aim_weight_blends_and_twist_rolls_about_the_view_axis() {
    let joint = head(DQuat::IDENTITY);
    let target = joint.position + DVec3::new(4.0, 0.0, 0.0);
    let full = aim(&joint, &look(target));
    let half = aim(
        &joint,
        &AimGoal {
            weight: 0.5,
            ..look(target)
        },
    );
    let ratio = half.correction.angle_between(DQuat::IDENTITY)
        / full.correction.angle_between(DQuat::IDENTITY);
    assert!((0.35..0.65).contains(&ratio), "half weight turns {ratio}");
    let none = aim(
        &joint,
        &AimGoal {
            weight: 0.0,
            ..look(target)
        },
    );
    assert!(none.correction.angle_between(DQuat::IDENTITY) < EPS && !none.reached);
    let rolled = aim(
        &joint,
        &AimGoal {
            twist: std::f64::consts::FRAC_PI_2,
            ..look(target)
        },
    );
    let up = rolled.correction * joint.up;
    assert!(
        up.dot(DVec3::Y).abs() < 1.0e-9,
        "rolled a quarter turn about the view axis: {up:?}"
    );
    assert!(
        (rolled.correction * joint.forward).dot(DVec3::X) > 1.0 - 1.0e-9,
        "twist keeps looking at the target"
    );
}

#[test]
fn aim_degenerate_inputs_stay_finite() {
    let joint = head(DQuat::from_rotation_x(0.3));
    // On the joint, straight above (parallel to the pole) and behind.
    for target in [
        joint.position,
        joint.position + DVec3::new(0.0, 3.0, 0.0),
        joint.position + DVec3::new(0.0, -3.0, 0.0),
        joint.position + DVec3::new(0.0, 0.0, -3.0),
    ] {
        let solution = aim(&joint, &look(target));
        assert!(
            solution.correction.is_finite() && (solution.correction.length() - 1.0).abs() < 1.0e-9,
            "target {target:?}"
        );
    }
    // A zero pole falls back to the current up.
    let solution = aim(
        &joint,
        &AimGoal {
            pole: DVec3::ZERO,
            ..look(joint.position + DVec3::X)
        },
    );
    assert!(solution.correction.is_finite());
}
