//! Parity corpus: the same physics scenes run natively and in WASI must print identical bits.
#![expect(clippy::print_stdout, reason = "parity corpus output")]

use ve_physics::{BodyDesc, JointDesc, JointKind, Kind, Shape, World};

fn print_poses(label: &str, world: &World) {
    let mut bits: Vec<u32> = Vec::new();
    for index in 0..world.len() {
        if let Ok(pose) = world.pose(ve_physics::BodyId(index)) {
            bits.extend(
                pose.position
                    .iter()
                    .chain(&pose.rotation)
                    .map(|v| v.to_bits()),
            );
        }
    }
    println!(
        "{label} bodies={} contacts={:?} bits={bits:08x?}",
        world.len(),
        world.contacts()
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A tower of boxes settling on a floor.
    let mut tower = World::new([0.0, -9.81, 0.0]);
    tower.add(&BodyDesc {
        kind: Kind::Static,
        shape: Shape::Box {
            half: [20.0, 0.5, 20.0],
        },
        position: [0.0, -0.5, 0.0],
        ..BodyDesc::default()
    })?;
    for layer in 0..8_u8 {
        tower.add(&BodyDesc {
            shape: Shape::Box { half: [0.5; 3] },
            position: [
                0.07 * f32::from(layer % 2),
                0.5 + f32::from(layer) * 1.02,
                0.0,
            ],
            friction: 0.7,
            ..BodyDesc::default()
        })?;
    }
    for _ in 0..300 {
        tower.step();
    }
    print_poses("tower", &tower);

    // Balls, a capsule and a convex rock falling on a triangle-mesh hill, with a kinematic pusher.
    let mut yard = World::new([0.0, -9.81, 0.0]);
    let hill = Shape::Mesh {
        points: vec![
            [-6.0, 0.0, -6.0],
            [6.0, 0.0, -6.0],
            [6.0, 0.0, 6.0],
            [-6.0, 0.0, 6.0],
            [0.0, 1.5, 0.0],
        ],
        triangles: vec![
            [0, 4, 1],
            [1, 4, 2],
            [2, 4, 3],
            [3, 4, 0],
            [0, 1, 2],
            [0, 2, 3],
        ],
    };
    yard.add(&BodyDesc {
        kind: Kind::Static,
        shape: hill,
        ..BodyDesc::default()
    })?;
    yard.add(&BodyDesc {
        shape: Shape::Sphere { radius: 0.3 },
        position: [0.4, 3.0, 0.1],
        restitution: 0.6,
        ..BodyDesc::default()
    })?;
    yard.add(&BodyDesc {
        shape: Shape::Capsule {
            half_height: 0.3,
            radius: 0.15,
        },
        position: [-0.5, 2.5, 0.3],
        rotation: [0.2, 0.0, 0.1, 0.97],
        ..BodyDesc::default()
    })?;
    yard.add(&BodyDesc {
        shape: Shape::Convex {
            points: vec![
                [-0.3, -0.3, -0.3],
                [0.3, -0.3, -0.3],
                [0.0, -0.3, 0.4],
                [0.0, 0.4, 0.0],
                [0.2, 0.1, 0.2],
            ],
        },
        position: [0.0, 2.0, -0.4],
        ..BodyDesc::default()
    })?;
    let pusher = yard.add(&BodyDesc {
        kind: Kind::Kinematic,
        shape: Shape::Box { half: [0.3; 3] },
        position: [-4.0, 0.4, 0.0],
        ..BodyDesc::default()
    })?;
    for step in 0..240_u16 {
        yard.set_pose(
            pusher,
            [-4.0 + f32::from(step) * 0.02, 0.4, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        )?;
        yard.step();
    }
    print_poses("yard", &yard);

    // Joints: a three-link ball chain hung from the world, a limited door with a motor, a slider, a spring and a weld.
    let mut rig = World::new([0.0, -9.81, 0.0]);
    let link = |at: [f32; 3]| BodyDesc {
        shape: Shape::Sphere { radius: 0.1 },
        position: at,
        angular_damping: 0.0,
        ..BodyDesc::default()
    };
    let (l1, l2, l3) = (
        rig.add(&link([0.5, 3.0, 0.0]))?,
        rig.add(&link([1.0, 3.0, 0.0]))?,
        rig.add(&link([1.5, 3.0, 0.0]))?,
    );
    rig.add_joint(&JointDesc {
        a: l1,
        b: None,
        kind: JointKind::Ball,
        anchor: [0.0, 3.0, 0.0],
        ..JointDesc::default()
    })?;
    rig.add_joint(&JointDesc {
        a: l1,
        b: Some(l2),
        kind: JointKind::Ball,
        anchor: [0.75, 3.0, 0.0],
        ..JointDesc::default()
    })?;
    rig.add_joint(&JointDesc {
        a: l2,
        b: Some(l3),
        kind: JointKind::Ball,
        anchor: [1.25, 3.0, 0.0],
        ..JointDesc::default()
    })?;
    let door = rig.add(&BodyDesc {
        shape: Shape::Box {
            half: [0.5, 1.0, 0.04],
        },
        position: [-2.5, 1.0, 0.0],
        ..BodyDesc::default()
    })?;
    rig.add_joint(&JointDesc {
        a: door,
        b: None,
        kind: JointKind::Hinge,
        anchor: [-3.0, 1.0, 0.0],
        axis: [0.0, 1.0, 0.0],
        limits: Some([-1.2, 1.2]),
        motor_speed: 1.0,
        motor_force: 20.0,
        ..JointDesc::default()
    })?;
    let drawer = rig.add(&BodyDesc {
        shape: Shape::Box {
            half: [0.3, 0.1, 0.2],
        },
        position: [3.0, 1.0, 0.0],
        ..BodyDesc::default()
    })?;
    rig.add_joint(&JointDesc {
        a: drawer,
        b: None,
        kind: JointKind::Slider,
        anchor: [3.0, 1.0, 0.0],
        axis: [1.0, 0.0, 0.0],
        limits: Some([-0.4, 0.4]),
        damping: 0.0,
        ..JointDesc::default()
    })?;
    rig.set_velocity(drawer, [2.0, 0.0, 0.0], [0.0; 3])?;
    let bob = rig.add(&link([5.0, 1.0, 0.0]))?;
    rig.add_joint(&JointDesc {
        a: bob,
        b: None,
        kind: JointKind::Spring,
        anchor: [5.0, 1.0, 0.0],
        anchor_b: [5.0, 3.0, 0.0],
        rest_length: 1.5,
        stiffness: 30.0,
        damping: 3.0,
        ..JointDesc::default()
    })?;
    for _ in 0..240 {
        rig.step();
    }
    print_poses("rig", &rig);
    Ok(())
}
