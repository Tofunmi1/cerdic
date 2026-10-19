//! Inverse kinematics for a chain of bone parts (`rig.ik`, ADR 0032): cyclic coordinate descent.
//!
//! The chain is a list of bones from the root to the tip, each with a rest segment (head, tail) in world coordinates, as
//! `rig.bone` records. A bone turns about its head; the next bone's head is where this one ends, and the whole chain hangs
//! from `root_parent`, the world pose of the root bone's parent. The solver finds the turn of every bone (as a rotation
//! about its head, in its parent's frame, the same frame the viewer composes poses in) that brings the tail of the last
//! bone to the goal. A goal that cannot be reached leaves the chain straight and pointing at it.
//!
//! Only `+ - * /` and `sqrt` are used (in `f64`), so the result is the same on every target.
use glam::{DMat3, DMat4, DQuat, DVec3};

use crate::rig::Segment;

/// Most bones in one chain.
pub const MAX_CHAIN: usize = 8;
/// Most solver passes per frame.
pub const MAX_ITERATIONS: u32 = 64;
/// How close the tip must get to stop early, in metres.
const TOLERANCE: f64 = 1e-4;
/// The sideways bend, in radians, that gets a stuck straight chain moving.
const KINK: f64 = 0.15;

fn point(p: [f32; 3]) -> DVec3 {
    DVec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2]))
}

/// The world pose of every bone and the bones' head positions for the given local turns.
fn forward(
    joints: &[Segment],
    root_parent: DMat4,
    turns: &[DQuat],
) -> (Vec<DMat4>, Vec<DVec3>, DVec3) {
    let (mut worlds, mut heads) = (
        Vec::with_capacity(joints.len()),
        Vec::with_capacity(joints.len()),
    );
    let mut current = root_parent;
    for (joint, turn) in joints.iter().zip(turns) {
        let head = point(joint.head);
        heads.push(current.transform_point3(head));
        current *= DMat4::from_translation(head)
            * DMat4::from_quat(*turn)
            * DMat4::from_translation(-head);
        worlds.push(current);
    }
    let tip = joints.last().map_or(DVec3::ZERO, |last| {
        worlds[worlds.len() - 1].transform_point3(point(last.tail))
    });
    (worlds, heads, tip)
}

/// The rotation part of a pose, with any scale taken out.
fn rotation_of(m: DMat4) -> DQuat {
    let basis = DMat3::from_cols(
        m.x_axis.truncate().normalize_or_zero(),
        m.y_axis.truncate().normalize_or_zero(),
        m.z_axis.truncate().normalize_or_zero(),
    );
    DQuat::from_mat3(&basis).normalize()
}

/// Solves the chain for `goal` (world coordinates), starting from the rest pose, and returns each bone's local turn.
/// `root_parent` is the world pose of the part the first bone hangs from (the identity if it hangs from nothing).
#[must_use]
pub fn solve(joints: &[Segment], root_parent: DMat4, goal: DVec3, iterations: u32) -> Vec<DQuat> {
    let mut turns = vec![DQuat::IDENTITY; joints.len()];
    let reach: f64 = joints
        .iter()
        .map(|joint| point(joint.tail).distance(point(joint.head)))
        .sum();
    for _ in 0..iterations.min(MAX_ITERATIONS) {
        let (_, _, tip) = forward(joints, root_parent, &turns);
        if tip.distance(goal) < TOLERANCE {
            break;
        }
        for i in (0..joints.len()).rev() {
            let (worlds, heads, tip) = forward(joints, root_parent, &turns);
            let (to_tip, to_goal) = (tip - heads[i], goal - heads[i]);
            if to_tip.length_squared() < 1e-18 || to_goal.length_squared() < 1e-18 {
                continue;
            }
            // The world-space turn that swings the tip onto the line to the goal, expressed in this joint's parent frame.
            let (a, b) = (to_tip.normalize(), to_goal.normalize());
            let mut delta = DQuat::from_rotation_arc(a, b);
            // Stuck: tip and goal lie on one line through this joint (a straight chain and a goal on its line), so
            // no swing about the joint helps. Bend it a little to one fixed side and let later passes finish the job.
            // (A goal out of reach is reached by stretching straight at it, which is right as it is: no bend.)
            let out_of_reach = goal.distance(heads[0]) >= reach - 1e-9;
            if delta.angle_between(DQuat::IDENTITY) < 1e-9
                && !out_of_reach
                && tip.distance(goal) > TOLERANCE
            {
                let side = a.cross(DVec3::Y);
                let axis = if side.length_squared() > 1e-12 {
                    side.normalize()
                } else {
                    a.cross(DVec3::Z).normalize()
                };
                delta = DQuat::from_axis_angle(axis, KINK);
            }
            let parent = if i == 0 {
                rotation_of(root_parent)
            } else {
                rotation_of(worlds[i - 1])
            };
            turns[i] = (parent.inverse() * delta * parent * turns[i]).normalize();
        }
    }
    turns
}

/// The viewer's entry point, in plain arrays: solves the chain for `goal` (world) hanging from `root_parent` (a
/// column-major pose) and returns each bone's local pose matrix (a turn about its head), root first.
#[must_use]
pub fn solve_poses(
    joints: &[Segment],
    root_parent: [f32; 16],
    goal: [f32; 3],
    iterations: u32,
) -> Vec<[f32; 16]> {
    let parent = DMat4::from_cols_array(&root_parent.map(f64::from));
    let turns = solve(joints, parent, point(goal), iterations);
    joints
        .iter()
        .zip(&turns)
        .map(|(joint, turn)| bone_pose(joint, *turn))
        .collect()
}

/// The local pose matrix of a bone turned by `turn` about its head (column-major, for the viewer's `f32` poses).
#[must_use]
pub fn bone_pose(segment: &Segment, turn: DQuat) -> [f32; 16] {
    let head = point(segment.head);
    (DMat4::from_translation(head) * DMat4::from_quat(turn) * DMat4::from_translation(-head))
        .as_mat4()
        .to_cols_array()
}

/// Where the tip of the chain ends up for the given turns (for tests and diagnostics).
#[must_use]
pub fn tip_after(joints: &[Segment], root_parent: DMat4, turns: &[DQuat]) -> DVec3 {
    forward(joints, root_parent, turns).2
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two bones of length one along +X from the origin: shoulder at 0, elbow at 1, hand at 2.
    fn arm() -> [Segment; 2] {
        [
            Segment {
                head: [0.0, 0.0, 0.0],
                tail: [1.0, 0.0, 0.0],
            },
            Segment {
                head: [1.0, 0.0, 0.0],
                tail: [2.0, 0.0, 0.0],
            },
        ]
    }

    #[test]
    fn a_reachable_goal_is_reached_and_bones_keep_their_length() {
        let arm = arm();
        for goal in [
            DVec3::new(1.0, 1.0, 0.0),
            DVec3::new(0.5, -1.2, 0.8),
            DVec3::new(0.0, 1.9, 0.0),
            DVec3::new(-1.0, 0.5, 0.5),
        ] {
            let turns = solve(&arm, DMat4::IDENTITY, goal, MAX_ITERATIONS);
            let (worlds, heads, tip) = forward(&arm, DMat4::IDENTITY, &turns);
            assert!(
                tip.distance(goal) < 1e-3,
                "goal {goal:?}: the tip is at {tip:?}"
            );
            let upper = heads[1].distance(heads[0]);
            let lower = worlds[1]
                .transform_point3(point(arm[1].tail))
                .distance(heads[1]);
            assert!(
                (upper - 1.0).abs() < 1e-9 && (lower - 1.0).abs() < 1e-9,
                "rigid bones: {upper} {lower}"
            );
        }
    }

    #[test]
    fn an_unreachable_goal_leaves_the_chain_straight_and_pointing_at_it() {
        let arm = arm();
        let goal = DVec3::new(0.0, 5.0, 0.0);
        let turns = solve(&arm, DMat4::IDENTITY, goal, 32);
        let (_, heads, tip) = forward(&arm, DMat4::IDENTITY, &turns);
        assert!(
            tip.distance(DVec3::new(0.0, 2.0, 0.0)) < 1e-3,
            "stretched straight up: {tip:?}"
        );
        assert!(
            heads[1].distance(DVec3::new(0.0, 1.0, 0.0)) < 1e-3,
            "the elbow is on the line: {:?}",
            heads[1]
        );
    }

    #[test]
    fn a_goal_on_the_straight_line_inside_reach_folds_the_chain() {
        // The arm lies along +X; a goal 0.8 m out on that same line has an angle of zero at every joint, so a plain
        // swing never starts. The solver must bend the elbow on its own, in the same way every time.
        let arm = arm();
        for goal in [
            DVec3::new(0.8, 0.0, 0.0),
            DVec3::new(0.3, 0.0, 0.0),
            DVec3::new(-0.5, 0.0, 0.0),
        ] {
            let turns = solve(&arm, DMat4::IDENTITY, goal, MAX_ITERATIONS);
            let tip = tip_after(&arm, DMat4::IDENTITY, &turns);
            assert!(
                tip.distance(goal) < 1e-2,
                "goal {goal:?}: the tip is at {tip:?}"
            );
            assert_eq!(
                turns,
                solve(&arm, DMat4::IDENTITY, goal, MAX_ITERATIONS),
                "the same bend every time"
            );
        }
    }

    #[test]
    fn a_goal_at_the_rest_tip_moves_nothing() {
        let turns = solve(&arm(), DMat4::IDENTITY, DVec3::new(2.0, 0.0, 0.0), 32);
        assert!(
            turns
                .iter()
                .all(|t| t.angle_between(DQuat::IDENTITY) < 1e-9),
            "{turns:?}"
        );
    }

    #[test]
    fn a_chain_that_hangs_from_a_moved_parent_reaches_in_world_space() {
        // The whole arm is carried 3 m along +Y by its parent; a goal beside it is still reached.
        let parent = DMat4::from_translation(DVec3::new(0.0, 3.0, 0.0));
        let goal = DVec3::new(1.0, 4.0, 0.0);
        let turns = solve(&arm(), parent, goal, 32);
        assert!(tip_after(&arm(), parent, &turns).distance(goal) < 1e-3);
        // And a rotated parent: turned a quarter about Z, the rest tip is at (0, 2, 0); reach (-1, 1, 0).
        let turned = DMat4::from_quat(DQuat::from_rotation_z(std::f64::consts::FRAC_PI_2));
        let goal = DVec3::new(-1.0, 1.0, 0.0);
        let turns = solve(&arm(), turned, goal, 32);
        assert!(
            tip_after(&arm(), turned, &turns).distance(goal) < 1e-3,
            "{:?}",
            tip_after(&arm(), turned, &turns)
        );
    }

    #[test]
    fn three_bones_reach_and_the_result_is_the_same_every_time() {
        let chain = [
            Segment {
                head: [0.0, 0.0, 0.0],
                tail: [0.0, 1.0, 0.0],
            },
            Segment {
                head: [0.0, 1.0, 0.0],
                tail: [0.0, 2.0, 0.0],
            },
            Segment {
                head: [0.0, 2.0, 0.0],
                tail: [0.0, 3.0, 0.0],
            },
        ];
        let goal = DVec3::new(1.5, 1.5, 1.0);
        let a = solve(&chain, DMat4::IDENTITY, goal, 48);
        let b = solve(&chain, DMat4::IDENTITY, goal, 48);
        assert_eq!(a, b);
        assert!(tip_after(&chain, DMat4::IDENTITY, &a).distance(goal) < 1e-2);
    }
}
