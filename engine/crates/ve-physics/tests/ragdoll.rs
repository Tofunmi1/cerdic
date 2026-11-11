//! Ragdolls: a skeleton of capsule bones joined by ball and hinge joints with limits.
//! Written before the bodies. The checks are physical: it settles on the ground, joints do not tear apart, limits hold, nothing explodes.
#![expect(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    reason = "fixtures unwrap valid data and count small loops"
)]
use ve_physics::ragdoll::{Bone, BoneJoint, MAX_BONES, RagdollDesc, build};
use ve_physics::{BodyDesc, BodyId, Error, Kind, Shape, World};

const GRAVITY: [f32; 3] = [0.0, -9.81, 0.0];

fn ground(world: &mut World) {
    world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: Shape::Box {
                half: [30.0, 0.5, 30.0],
            },
            position: [0.0, -0.5, 0.0],
            friction: 0.8,
            ..BodyDesc::default()
        })
        .unwrap();
}

fn bone(parent: Option<usize>, head: [f32; 3], tail: [f32; 3], radius: f32, joint: BoneJoint) -> Bone {
    Bone {
        parent,
        head,
        tail,
        radius,
        joint,
    }
}

fn lifted(mut bones: Vec<Bone>, by: f32) -> Vec<Bone> {
    for b in &mut bones {
        b.head[1] += by;
        b.tail[1] += by;
    }
    bones
}

/// A person standing: pelvis, chest, head, an arm (upper arm, forearm) and a leg (thigh, shin).
fn person() -> Vec<Bone> {
    let ball = |swing| BoneJoint::Ball {
        swing,
        twist: [-0.4, 0.4],
    };
    vec![
        bone(None, [0.0, 1.0, 0.0], [0.0, 1.3, 0.0], 0.12, BoneJoint::Fixed),
        bone(Some(0), [0.0, 1.3, 0.0], [0.0, 1.6, 0.0], 0.13, ball(0.5)),
        bone(Some(1), [0.0, 1.6, 0.0], [0.0, 1.8, 0.0], 0.1, ball(0.6)),
        bone(Some(1), [0.0, 1.55, 0.0], [0.3, 1.55, 0.0], 0.05, ball(1.2)),
        bone(
            Some(3),
            [0.3, 1.55, 0.0],
            [0.6, 1.55, 0.0],
            0.045,
            BoneJoint::Hinge {
                axis: [0.0, 0.0, 1.0],
                limits: [0.0, 2.4],
            },
        ),
        bone(Some(0), [0.1, 1.0, 0.0], [0.1, 0.55, 0.0], 0.07, ball(0.9)),
        bone(
            Some(5),
            [0.1, 0.55, 0.0],
            [0.1, 0.1, 0.0],
            0.06,
            BoneJoint::Hinge {
                axis: [1.0, 0.0, 0.0],
                limits: [-2.3, 0.0],
            },
        ),
    ]
}

fn desc(bones: Vec<Bone>) -> RagdollDesc {
    RagdollDesc {
        bones,
        density: 400.0,
        friction: 0.8,
        self_collide: false,
        joint_damping: 0.3,
    }
}

fn rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let u = [q[0], q[1], q[2]];
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let t = cross(u, v);
    let t = [t[0] + q[3] * v[0], t[1] + q[3] * v[1], t[2] + q[3] * v[2]];
    let c = cross(u, t);
    [v[0] + 2.0 * c[0], v[1] + 2.0 * c[1], v[2] + 2.0 * c[2]]
}

fn along(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// Where a bone's head and tail are now, from its body's pose.
fn ends(world: &World, id: BodyId, bone: &Bone) -> ([f32; 3], [f32; 3]) {
    let pose = world.pose(id).unwrap();
    let half = along(bone.head, bone.tail) * 0.5;
    let up = rotate(pose.rotation, [0.0, half, 0.0]);
    let (p, dn) = (pose.position, rotate(pose.rotation, [0.0, -half, 0.0]));
    ([p[0] + dn[0], p[1] + dn[1], p[2] + dn[2]], [p[0] + up[0], p[1] + up[1], p[2] + up[2]])
}

/// Where each bone's joint is, as a point in its parent's own frame (taken at the start, from the skeleton). A branch joins its parent
/// anywhere along it (a hip is at the pelvis's head, a shoulder mid-chest), so the parent's tail is not the place to compare with.
fn anchors(world: &World, ids: &[BodyId], bones: &[Bone]) -> Vec<[f32; 3]> {
    bones
        .iter()
        .map(|b| {
            let Some(parent) = b.parent else {
                return [0.0; 3];
            };
            let pose = world.pose(ids[parent]).unwrap();
            let d = [b.head[0] - pose.position[0], b.head[1] - pose.position[1], b.head[2] - pose.position[2]];
            let conjugate = [-pose.rotation[0], -pose.rotation[1], -pose.rotation[2], pose.rotation[3]];
            rotate(conjugate, d)
        })
        .collect()
}

/// The worst gap between a bone's head and the joint's point on its parent.
fn tear(world: &World, ids: &[BodyId], bones: &[Bone], anchors: &[[f32; 3]]) -> f32 {
    bones
        .iter()
        .enumerate()
        .filter_map(|(i, b)| {
            let parent = b.parent?;
            let (head, _) = ends(world, ids[i], b);
            let pose = world.pose(ids[parent]).unwrap();
            let r = rotate(pose.rotation, anchors[i]);
            let on_parent = [pose.position[0] + r[0], pose.position[1] + r[1], pose.position[2] + r[2]];
            Some(along(head, on_parent))
        })
        .fold(0.0, f32::max)
}

#[test]
fn a_dropped_ragdoll_lands_settles_and_stays_in_one_piece() {
    let bones = lifted(person(), 1.0);
    let mut world = World::new(GRAVITY);
    ground(&mut world);
    let ragdoll = build(&mut world, &desc(bones.clone())).unwrap();
    assert_eq!(ragdoll.bodies.len(), bones.len());
    let joints = anchors(&world, &ragdoll.bodies, &bones);
    let (mut worst_tear, mut fastest) = (0.0_f32, 0.0_f32);
    for _ in 0..(5.0 * 60.0) as u32 {
        world.step();
        worst_tear = worst_tear.max(tear(&world, &ragdoll.bodies, &bones, &joints));
        for id in &ragdoll.bodies {
            let v = world.velocity(*id).unwrap();
            fastest = fastest.max((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt());
        }
    }
    assert!(worst_tear < 0.1, "joints stay together: worst gap {worst_tear} m");
    assert!(fastest < 20.0, "nothing explodes: fastest {fastest} m/s");
    for (id, b) in ragdoll.bodies.iter().zip(&bones) {
        let pose = world.pose(*id).unwrap();
        assert!(pose.position[1] >= b.radius - 0.03, "a bone sank into the ground: y = {}", pose.position[1]);
        let v = world.velocity(*id).unwrap();
        assert!(v.iter().all(|c| c.abs() < 0.35), "settled: {v:?}");
    }
}

#[test]
fn the_ragdoll_starts_exactly_where_the_skeleton_is() {
    let bones = person();
    let mut world = World::new(GRAVITY);
    let ragdoll = build(&mut world, &desc(bones.clone())).unwrap();
    for (id, b) in ragdoll.bodies.iter().zip(&bones) {
        let (head, tail) = ends(&world, *id, b);
        assert!(along(head, b.head) < 1e-4 && along(tail, b.tail) < 1e-4, "{head:?} {tail:?} vs {:?} {:?}", b.head, b.tail);
    }
}

#[test]
fn a_hinge_stays_inside_its_limits_even_when_slammed() {
    // A straight arm along +X: both bones share one orientation at rest, so the hinge angle is the relative turn about Z.
    let arm = vec![
        bone(None, [0.0, 2.0, 0.0], [0.4, 2.0, 0.0], 0.05, BoneJoint::Fixed),
        bone(
            Some(0),
            [0.4, 2.0, 0.0],
            [0.8, 2.0, 0.0],
            0.05,
            BoneJoint::Hinge {
                axis: [0.0, 0.0, 1.0],
                limits: [0.0, 2.4],
            },
        ),
    ];
    for push in [-9.0_f32, 9.0] {
        let mut world = World::new(GRAVITY);
        ground(&mut world);
        let ragdoll = build(&mut world, &desc(arm.clone())).unwrap();
        world.set_velocity(ragdoll.bodies[1], [0.0; 3], [0.0, 0.0, push]).unwrap();
        let (mut lowest, mut highest) = (f32::MAX, f32::MIN);
        for _ in 0..(3.0 * 60.0) as u32 {
            world.step();
            let (p, c) = (world.pose(ragdoll.bodies[0]).unwrap().rotation, world.pose(ragdoll.bodies[1]).unwrap().rotation);
            // relative = conj(p) * c; about Z the angle is 2 * atan2(z, w).
            let rel_z = p[3] * c[2] - p[0] * c[1] + p[1] * c[0] - p[2] * c[3];
            let rel_w = p[3] * c[3] + p[0] * c[0] + p[1] * c[1] + p[2] * c[2];
            let mut angle = 2.0 * rel_z.atan2(rel_w);
            if angle > std::f32::consts::PI {
                angle -= std::f32::consts::TAU;
            }
            lowest = lowest.min(angle);
            highest = highest.max(angle);
        }
        assert!(lowest > -0.25 && highest < 2.65, "push {push}: the elbow went {lowest}..{highest}, past its 0..2.4 limits");
    }
}

#[test]
fn a_ball_joint_cone_limit_keeps_a_limb_near_its_socket_direction() {
    let limb = |swing| {
        vec![
            bone(None, [0.0, 3.0, 0.0], [0.5, 3.0, 0.0], 0.06, BoneJoint::Fixed),
            bone(
                Some(0),
                [0.5, 3.0, 0.0],
                [1.0, 3.0, 0.0],
                0.06,
                BoneJoint::Ball {
                    swing,
                    twist: [-0.2, 0.2],
                },
            ),
        ]
    };
    let widest = |swing: f32| {
        let mut world = World::new(GRAVITY);
        ground(&mut world);
        let bones = limb(swing);
        let ragdoll = build(&mut world, &desc(bones)).unwrap();
        world.set_velocity(ragdoll.bodies[1], [0.0; 3], [3.0, 2.0, 5.0]).unwrap();
        let mut worst = 0.0_f32;
        for _ in 0..(4.0 * 60.0) as u32 {
            world.step();
            let a = rotate(world.pose(ragdoll.bodies[0]).unwrap().rotation, [0.0, 1.0, 0.0]);
            let b = rotate(world.pose(ragdoll.bodies[1]).unwrap().rotation, [0.0, 1.0, 0.0]);
            let dot = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).clamp(-1.0, 1.0);
            worst = worst.max(dot.acos());
        }
        worst
    };
    let limited = widest(0.3);
    let free = widest(std::f32::consts::PI);
    assert!(limited < 0.3 * 1.5 + 0.12, "a 0.3 rad cone held the limb to {limited} rad");
    assert!(free > 0.9, "the test is only meaningful if an unlimited joint swings wide: {free}");
}

#[test]
fn the_same_ragdoll_falls_the_same_way_every_time() {
    let run = || {
        let bones = lifted(person(), 0.8);
        let mut world = World::new(GRAVITY);
        ground(&mut world);
        let ragdoll = build(&mut world, &desc(bones)).unwrap();
        for _ in 0..240 {
            world.step();
        }
        ragdoll
            .bodies
            .iter()
            .flat_map(|id| {
                let p = world.pose(*id).unwrap();
                p.position.iter().chain(&p.rotation).map(|v| v.to_bits()).collect::<Vec<_>>()
            })
            .collect::<Vec<u32>>()
    };
    assert_eq!(run(), run());
}

#[test]
fn with_self_collision_off_bones_of_one_ragdoll_do_not_push_each_other() {
    // A thigh folded back onto the pelvis overlaps it; with self-collision off nothing pushes them apart.
    let mut bones = person();
    bones[5].tail = [0.1, 1.2, 0.0];
    bones[5].head = [0.1, 1.0, 0.0];
    bones.truncate(6);
    let mut world = World::new([0.0; 3]);
    let ragdoll = build(&mut world, &desc(bones)).unwrap();
    for _ in 0..120 {
        world.step();
    }
    let v = world.velocity(ragdoll.bodies[0]).unwrap();
    assert!(v.iter().all(|c| c.abs() < 0.05), "overlapping bones of one body must not fight: {v:?}");
}

/// Applies a column-major 4x4 to a point.
fn transform(m: [f32; 16], p: [f32; 3]) -> [f32; 3] {
    [
        m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
        m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
        m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
    ]
}

#[test]
fn a_bones_pose_matrix_carries_its_rest_segment_to_where_its_body_is() {
    use ve_physics::ragdoll::{bone_pose, rest_frame};
    let bones = lifted(person(), 1.0);
    let mut world = World::new(GRAVITY);
    ground(&mut world);
    let ragdoll = build(&mut world, &desc(bones.clone())).unwrap();
    let rest: Vec<_> = bones.iter().map(rest_frame).collect();
    // At rest the pose is the identity (the mesh stays where it was authored).
    for (b, (position, rotation)) in bones.iter().zip(&rest) {
        let m = bone_pose(*position, *rotation, *position, *rotation);
        assert!(along(transform(m, b.head), b.head) < 1e-5 && along(transform(m, b.tail), b.tail) < 1e-5);
    }
    // After the fall, carrying each bone's authored head and tail through its pose lands them on the body's own head and tail.
    for _ in 0..(3.0 * 60.0) as u32 {
        world.step();
    }
    for (i, b) in bones.iter().enumerate() {
        let pose = world.pose(ragdoll.bodies[i]).unwrap();
        let m = bone_pose(rest[i].0, rest[i].1, pose.position, pose.rotation);
        let (head, tail) = ends(&world, ragdoll.bodies[i], b);
        assert!(along(transform(m, b.head), head) < 1e-3, "bone {i} head");
        assert!(along(transform(m, b.tail), tail) < 1e-3, "bone {i} tail");
        // It is a rigid motion: lengths are kept.
        assert!((along(transform(m, b.head), transform(m, b.tail)) - along(b.head, b.tail)).abs() < 1e-3);
    }
}

#[test]
fn bad_skeletons_are_refused_with_a_reason() {
    let mut world = World::new(GRAVITY);
    let refuse = |world: &mut World, bones: Vec<Bone>| build(world, &desc(bones)).unwrap_err();
    assert!(matches!(refuse(&mut world, Vec::new()), Error::BadValue(_)), "no bones");
    let mut b = person();
    b[1].parent = Some(4);
    assert!(matches!(refuse(&mut world, b), Error::BadValue(_)), "a parent must come before its child");
    let mut b = person();
    b[2].radius = 0.0;
    assert!(matches!(refuse(&mut world, b), Error::BadValue(_)), "a bone needs a radius");
    let mut b = person();
    b[2].tail = b[2].head;
    assert!(matches!(refuse(&mut world, b), Error::BadValue(_)), "a bone needs a length");
    let mut b = person();
    b[3].head[0] = f32::NAN;
    assert!(matches!(refuse(&mut world, b), Error::BadValue(_)), "numbers must be finite");
    let many: Vec<Bone> = (0..=MAX_BONES)
        .map(|i| bone(i.checked_sub(1), [0.0, i as f32 * 0.1, 0.0], [0.0, i as f32 * 0.1 + 0.1, 0.0], 0.03, BoneJoint::Fixed))
        .collect();
    assert!(matches!(refuse(&mut world, many), Error::BadValue(_)), "too many bones");
    let mut b = person();
    b[4].joint = BoneJoint::Hinge {
        axis: [0.0; 3],
        limits: [0.0, 1.0],
    };
    assert!(matches!(refuse(&mut world, b), Error::BadValue(_)), "a hinge needs an axis");
    let mut b = person();
    b[1].joint = BoneJoint::Ball {
        swing: -0.5,
        twist: [0.0, 0.0],
    };
    assert!(matches!(refuse(&mut world, b), Error::BadValue(_)), "a swing limit cannot be negative");
    assert!(world.is_empty(), "a refused ragdoll leaves no bodies behind");
}
