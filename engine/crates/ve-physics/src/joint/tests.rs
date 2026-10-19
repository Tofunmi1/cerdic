use super::*;
use crate::{BodyDesc, Kind, Shape};

const GRAVITY: [f32; 3] = [0.0, -9.81, 0.0];

fn ball(world: &mut World, at: [f32; 3]) -> BodyId {
    world
        .add(&BodyDesc {
            shape: Shape::Sphere { radius: 0.15 },
            position: at,
            angular_damping: 0.0,
            ..BodyDesc::default()
        })
        .unwrap()
}

fn run(world: &mut World, steps: u32) {
    for _ in 0..steps {
        world.step();
    }
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn a_hinged_pendulum_swings_down_and_keeps_its_length() {
    let mut world = World::new(GRAVITY);
    let bob = ball(&mut world, [1.0, 2.0, 0.0]);
    world
        .add_joint(&JointDesc {
            a: bob,
            b: None,
            kind: JointKind::Hinge,
            anchor: [0.0, 2.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            damping: 0.0,
            ..JointDesc::default()
        })
        .unwrap();
    let mut lowest = f32::MAX;
    for _ in 0..90 {
        world.step();
        let p = world.pose(bob).unwrap().position;
        lowest = lowest.min(p[1]);
        assert!(
            (distance(p, [0.0, 2.0, 0.0]) - 1.0).abs() < 0.03,
            "the string stays taut: {p:?}"
        );
        assert!(p[2].abs() < 0.02, "it swings in one plane: {p:?}");
    }
    assert!(lowest < 1.2, "it swung down from the horizontal: {lowest}");
}

#[test]
fn a_weighted_rod_pendulum_swings_in_its_plane_for_ten_seconds() {
    let mut world = World::new(GRAVITY);
    let rod = world
        .add(&BodyDesc {
            shape: Shape::Box {
                half: [0.75, 0.025, 0.025],
            },
            position: [-5.25, 4.0, -3.0],
            density: 30.0,
            angular_damping: 0.0,
            ..BodyDesc::default()
        })
        .unwrap();
    let bob = world
        .add(&BodyDesc {
            shape: Shape::Sphere { radius: 0.3 },
            position: [-4.5, 4.0, -3.0],
            density: 4.0,
            angular_damping: 0.0,
            ..BodyDesc::default()
        })
        .unwrap();
    world
        .add_joint(&JointDesc {
            a: rod,
            b: None,
            kind: JointKind::Hinge,
            anchor: [-6.0, 4.0, -3.0],
            axis: [0.0, 0.0, 1.0],
            damping: 0.02,
            ..JointDesc::default()
        })
        .unwrap();
    world
        .add_joint(&JointDesc {
            a: bob,
            b: Some(rod),
            kind: JointKind::Fixed,
            anchor: [-4.5, 4.0, -3.0],
            ..JointDesc::default()
        })
        .unwrap();
    let (mut worst_plane, mut worst_length) = (0.0_f32, 0.0_f32);
    for _ in 0..600 {
        world.step();
        let p = world.pose(bob).unwrap().position;
        worst_plane = worst_plane.max((p[2] + 3.0).abs());
        worst_length = worst_length.max((distance(p, [-6.0, 4.0, -3.0]) - 1.5).abs());
    }
    assert!(
        worst_plane < 0.05,
        "it stays in its plane: drifted {worst_plane} m"
    );
    assert!(
        worst_length < 0.1,
        "and the weld holds: stretched {worst_length} m"
    );
}

#[test]
fn hinge_limits_stop_a_door_and_a_motor_spins_a_wheel() {
    let mut world = World::new([0.0; 3]);
    let door = world
        .add(&BodyDesc {
            shape: Shape::Box {
                half: [0.5, 1.0, 0.04],
            },
            position: [0.5, 1.0, 0.0],
            angular_damping: 0.0,
            ..BodyDesc::default()
        })
        .unwrap();
    world
        .add_joint(&JointDesc {
            a: door,
            b: None,
            kind: JointKind::Hinge,
            anchor: [0.0, 1.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            limits: Some([0.0, 1.5]),
            damping: 0.0,
            ..JointDesc::default()
        })
        .unwrap();
    world
        .set_velocity(door, [0.0; 3], [0.0, -6.0, 0.0])
        .unwrap();
    run(&mut world, 120);
    let [x, _, z] = world.pose(door).unwrap().position;
    let angle = z.atan2(x).abs();
    assert!(
        angle <= 1.5 + 0.08,
        "the door stops at its limit (turned {angle} rad)"
    );
    assert!(
        angle > 1.2,
        "and it did swing out to the limit ({angle} rad)"
    );

    let mut spin = World::new([0.0; 3]);
    let wheel = spin
        .add(&BodyDesc {
            shape: Shape::Cylinder {
                half_height: 0.05,
                radius: 0.4,
            },
            angular_damping: 0.0,
            ..BodyDesc::default()
        })
        .unwrap();
    spin.add_joint(&JointDesc {
        a: wheel,
        b: None,
        kind: JointKind::Hinge,
        anchor: [0.0; 3],
        axis: [0.0, 1.0, 0.0],
        motor_speed: 3.0,
        motor_force: 50.0,
        ..JointDesc::default()
    })
    .unwrap();
    run(&mut spin, 120);
    let pose = spin.pose(wheel).unwrap();
    // The wheel turned about Y: its rotation quaternion is mostly in y/w and well away from the identity.
    assert!(
        pose.rotation[1].abs() > 0.3,
        "the motor turned the wheel: {pose:?}"
    );
}

#[test]
fn a_slider_moves_along_its_axis_only_and_a_fixed_joint_holds() {
    let mut world = World::new(GRAVITY);
    let drawer = world
        .add(&BodyDesc {
            shape: Shape::Box {
                half: [0.3, 0.1, 0.2],
            },
            position: [0.0, 1.0, 0.0],
            ..BodyDesc::default()
        })
        .unwrap();
    world
        .add_joint(&JointDesc {
            a: drawer,
            b: None,
            kind: JointKind::Slider,
            anchor: [0.0, 1.0, 0.0],
            axis: [1.0, 0.0, 0.0],
            limits: Some([-0.5, 0.5]),
            damping: 0.0,
            ..JointDesc::default()
        })
        .unwrap();
    world
        .set_velocity(drawer, [4.0, 0.0, 1.0], [0.0; 3])
        .unwrap();
    run(&mut world, 120);
    let p = world.pose(drawer).unwrap().position;
    assert!(
        p[0] > 0.3 && p[0] <= 0.55,
        "it slid out to its limit: {p:?}"
    );
    assert!(
        (p[1] - 1.0).abs() < 0.02 && p[2].abs() < 0.02,
        "and moved only along X, not down or sideways: {p:?}"
    );

    let mut welded = World::new(GRAVITY);
    let post = welded
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: Shape::Box {
                half: [0.1, 1.0, 0.1],
            },
            position: [0.0, 1.0, 0.0],
            ..BodyDesc::default()
        })
        .unwrap();
    let arm = welded
        .add(&BodyDesc {
            shape: Shape::Box {
                half: [0.5, 0.05, 0.05],
            },
            position: [0.6, 2.0, 0.0],
            ..BodyDesc::default()
        })
        .unwrap();
    welded
        .add_joint(&JointDesc {
            a: post,
            b: Some(arm),
            kind: JointKind::Fixed,
            anchor: [0.1, 2.0, 0.0],
            ..JointDesc::default()
        })
        .unwrap();
    run(&mut welded, 120);
    let p = welded.pose(arm).unwrap().position;
    assert!(
        distance(p, [0.6, 2.0, 0.0]) < 0.03,
        "a welded arm does not sag: {p:?}"
    );
}

#[test]
fn a_spring_hangs_a_ball_near_its_rest_length_and_bad_joints_are_refused() {
    let mut world = World::new(GRAVITY);
    let bob = ball(&mut world, [0.0, 1.0, 0.0]);
    world
        .add_joint(&JointDesc {
            a: bob,
            b: None,
            kind: JointKind::Spring,
            anchor: [0.0, 1.0, 0.0],
            anchor_b: [0.0, 3.0, 0.0],
            rest_length: 1.5,
            stiffness: 400.0,
            damping: 20.0,
            ..JointDesc::default()
        })
        .unwrap();
    run(&mut world, 360);
    let y = world.pose(bob).unwrap().position[1];
    assert!(
        (y - 1.5).abs() < 0.1,
        "the ball hangs 1.5 m below its anchor point at y=3: {y}"
    );

    assert!(
        world
            .add_joint(&JointDesc {
                a: BodyId(99),
                ..JointDesc::default()
            })
            .is_err()
    );
    assert!(
        world
            .add_joint(&JointDesc {
                a: bob,
                axis: [0.0; 3],
                ..JointDesc::default()
            })
            .is_err(),
        "a hinge needs an axis"
    );
    assert!(
        world
            .add_joint(&JointDesc {
                a: bob,
                limits: Some([1.0, 0.0]),
                ..JointDesc::default()
            })
            .is_err()
    );
    assert!(
        world
            .add_joint(&JointDesc {
                a: bob,
                anchor: [f32::NAN, 0.0, 0.0],
                ..JointDesc::default()
            })
            .is_err()
    );
    assert!(
        world
            .add_joint(&JointDesc {
                a: bob,
                stiffness: -1.0,
                ..JointDesc::default()
            })
            .is_err()
    );
}
