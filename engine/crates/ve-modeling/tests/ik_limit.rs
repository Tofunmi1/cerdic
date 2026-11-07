//! CCD joint limits: no bone turns past `limit`, an easy goal is still reached, and no limit behaves exactly as before. Do not weaken.
#![expect(clippy::unwrap_used, reason = "fixture")]
use glam::{DMat4, DQuat, DVec3};
use ve_modeling::ik::{solve, solve_limited, tip_after};
use ve_modeling::rig::Segment;

/// Four 0.5 m bones standing up the y axis, a tentacle.
fn tentacle() -> Vec<Segment> {
    (0..4)
        .map(|i| Segment {
            head: [0.0, i as f32 * 0.5, 0.0],
            tail: [0.0, (i + 1) as f32 * 0.5, 0.0],
        })
        .collect()
}

fn angle(q: &DQuat) -> f64 {
    2.0 * q.w.abs().clamp(0.0, 1.0).acos()
}

#[test]
fn every_bone_stays_within_the_limit_and_the_chain_still_leans_toward_a_far_goal() {
    let chain = tentacle();
    let goal = DVec3::new(1.5, 0.5, 0.0);
    let limit = 30.0_f64.to_radians();
    let turns = solve_limited(&chain, DMat4::IDENTITY, goal, 64, limit);
    for (i, t) in turns.iter().enumerate() {
        assert!(
            angle(t) <= limit + 1e-9,
            "bone {i} turned {} degrees",
            angle(t).to_degrees()
        );
    }
    let tip = tip_after(&chain, DMat4::IDENTITY, &turns);
    let free = tip_after(
        &chain,
        DMat4::IDENTITY,
        &solve(&chain, DMat4::IDENTITY, goal, 64),
    );
    assert!(
        tip.x > 0.6,
        "the limited chain still leans toward the goal: tip at {tip:?}"
    );
    assert!(
        tip.distance(goal) > free.distance(goal),
        "limits keep it from reaching as far as the free chain"
    );
}

#[test]
fn a_goal_within_the_limits_is_reached() {
    let chain = tentacle();
    let goal = DVec3::new(0.4, 1.8, 0.0);
    let turns = solve_limited(&chain, DMat4::IDENTITY, goal, 64, 60.0_f64.to_radians());
    let tip = tip_after(&chain, DMat4::IDENTITY, &turns);
    assert!(tip.distance(goal) < 0.01, "tip {tip:?} for goal {goal:?}");
    assert!(
        turns
            .iter()
            .all(|t| angle(t) <= 60.0_f64.to_radians() + 1e-9)
    );
}

#[test]
fn a_full_turn_limit_is_the_same_as_no_limit_at_all() {
    let chain = tentacle();
    let goal = DVec3::new(0.7, 1.2, 0.3);
    let free = solve(&chain, DMat4::IDENTITY, goal, 32);
    let full = solve_limited(&chain, DMat4::IDENTITY, goal, 32, std::f64::consts::PI);
    assert_eq!(free, full);
}

#[test]
fn a_tighter_limit_never_reaches_closer() {
    let chain = tentacle();
    let goal = DVec3::new(1.2, 0.6, 0.0);
    let reach = |degrees: f64| {
        let turns = solve_limited(&chain, DMat4::IDENTITY, goal, 64, degrees.to_radians());
        tip_after(&chain, DMat4::IDENTITY, &turns).distance(goal)
    };
    assert!(
        reach(20.0) >= reach(45.0) - 1e-6 && reach(45.0) >= reach(90.0) - 1e-6,
        "{} {} {}",
        reach(20.0),
        reach(45.0),
        reach(90.0)
    );
}
