use super::*;
use crate::Shape;

const GRAVITY: [f32; 3] = [0.0, -9.81, 0.0];

fn ground(world: &mut World) -> BodyId {
    world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: Shape::Box {
                half: [20.0, 0.5, 20.0],
            },
            position: [0.0, -0.5, 0.0],
            ..BodyDesc::default()
        })
        .unwrap()
}

fn cube(world: &mut World, at: [f32; 3]) -> BodyId {
    world
        .add(&BodyDesc {
            shape: Shape::Box { half: [0.5; 3] },
            position: at,
            friction: 0.7,
            ..BodyDesc::default()
        })
        .unwrap()
}

/// Steps the world for about `tenths` tenths of a second.
fn run(world: &mut World, tenths: u16) {
    // Six steps of 1/60 s make a tenth of a second.
    for _ in 0..u32::from(tenths) * 6 {
        world.step();
    }
}

#[test]
fn a_push_changes_the_speed_without_touching_the_spin() {
    let mut world = World::new([0.0; 3]);
    let ball = cube(&mut world, [0.0, 5.0, 0.0]);
    world.set_velocity(ball, [1.0, 0.0, 0.0], [0.0, 2.0, 0.0]).unwrap();
    world.push(ball, [0.0, 3.0, 0.0]).unwrap();
    assert_eq!(world.velocity(ball).unwrap(), [1.0, 3.0, 0.0]);
    assert!(world.push(ball, [f32::NAN, 0.0, 0.0]).is_err());
    assert!(world.push(BodyId(99), [0.0; 3]).is_err());
    run(&mut world, 5);
    assert!(world.pose(ball).unwrap().position[1] > 6.0, "the push lifted it");
}

#[test]
fn a_removed_body_leaves_with_its_joints_and_the_other_ids_stay() {
    use crate::{JointDesc, JointKind};
    let mut world = World::new(GRAVITY);
    let floor = ground(&mut world);
    let doomed = cube(&mut world, [0.0, 3.0, 0.0]);
    let kept = cube(&mut world, [5.0, 3.0, 0.0]);
    world
        .add_joint(&JointDesc {
            a: doomed,
            b: None,
            kind: JointKind::Hinge,
            anchor: [0.0, 4.0, 0.0],
            axis: [0.0, 0.0, 1.0],
            ..JointDesc::default()
        })
        .unwrap();
    assert_eq!((world.len(), world.joint_count()), (3, 1));
    world.remove(doomed).unwrap();
    assert_eq!((world.len(), world.joint_count()), (2, 0));
    assert!(world.pose(doomed).is_err());
    assert!(world.remove(doomed).is_err());
    // The other bodies keep their ids and the world still runs: the kept cube falls to the floor and rests there.
    run(&mut world, 30);
    let at = world.pose(kept).unwrap().position;
    assert!((at[1] - 0.5).abs() < 0.05, "kept cube rests on the floor: {at:?}");
    assert!(world.pose(floor).is_ok());
    // A new body after a removal gets a fresh id.
    let fresh = cube(&mut world, [9.0, 3.0, 0.0]);
    assert!(fresh != doomed && fresh != kept && fresh != floor);
}

#[test]
fn a_dropped_box_comes_to_rest_on_the_ground_and_touches_it() {
    let mut world = World::new(GRAVITY);
    let floor = ground(&mut world);
    let block = cube(&mut world, [0.0, 2.0, 0.0]);
    run(&mut world, 30);
    let pose = world.pose(block).unwrap();
    assert!(
        (pose.position[1] - 0.5).abs() < 0.02,
        "rests with its base on the ground: {pose:?}"
    );
    assert!(world.velocity(block).unwrap()[1].abs() < 0.05);
    assert_eq!(world.contacts(), vec![(floor, block)]);
}

#[test]
fn restitution_makes_a_ball_bounce_and_none_makes_it_stop() {
    let peak_after_first_bounce = |restitution: f32| {
        let mut world = World::new(GRAVITY);
        ground(&mut world);
        let ball = world
            .add(&BodyDesc {
                shape: Shape::Sphere { radius: 0.25 },
                position: [0.0, 3.0, 0.0],
                restitution,
                ..BodyDesc::default()
            })
            .unwrap();
        let (mut peak, mut landed) = (0.0_f32, false);
        for _ in 0..240 {
            world.step();
            let y = world.pose(ball).unwrap().position[1];
            if y < 0.3 {
                landed = true;
            } else if landed {
                peak = peak.max(y);
            }
        }
        peak
    };
    assert!(
        peak_after_first_bounce(0.8) > 1.0,
        "a lively ball rebounds high"
    );
    assert!(peak_after_first_bounce(0.0) < 0.4, "a dead one stays down");
}

#[test]
fn the_same_world_gives_the_same_bits_every_time() {
    let stack = || {
        let mut world = World::new(GRAVITY);
        ground(&mut world);
        let ids: Vec<BodyId> = (0..8_u8)
            .map(|layer| {
                cube(
                    &mut world,
                    [
                        0.07 * f32::from(layer % 2),
                        0.5 + f32::from(layer) * 1.02,
                        0.0,
                    ],
                )
            })
            .collect();
        run(&mut world, 50);
        ids.iter()
            .flat_map(|id| {
                let pose = world.pose(*id).unwrap();
                pose.position
                    .into_iter()
                    .chain(pose.rotation)
                    .map(f32::to_bits)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(stack(), stack());
}

#[test]
fn a_kinematic_body_pushes_a_dynamic_one_that_stands_in_its_way() {
    let mut world = World::new([0.0; 3]);
    let pusher = world
        .add(&BodyDesc {
            kind: Kind::Kinematic,
            shape: Shape::Box { half: [0.5; 3] },
            position: [-2.0, 0.0, 0.0],
            ..BodyDesc::default()
        })
        .unwrap();
    let ball = world
        .add(&BodyDesc {
            shape: Shape::Sphere { radius: 0.5 },
            position: [0.0, 0.0, 0.0],
            angular_damping: 1.0,
            ..BodyDesc::default()
        })
        .unwrap();
    for step in 0..120_u16 {
        world
            .set_pose(
                pusher,
                [-2.0 + f32::from(step) * 0.04, 0.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            )
            .unwrap();
        world.step();
    }
    assert!(
        world.pose(ball).unwrap().position[0] > 1.0,
        "the ball was shoved along +x: {:?}",
        world.pose(ball).unwrap()
    );
    assert!(
        (world.pose(pusher).unwrap().position[0] - (-2.0 + 119.0 * 0.04)).abs() < 0.05,
        "the pusher went where it was told"
    );
}

#[test]
fn a_thrown_ball_follows_a_parabola_and_holding_freezes_it() {
    let mut world = World::new(GRAVITY);
    let ball = world
        .add(&BodyDesc {
            shape: Shape::Sphere { radius: 0.1 },
            position: [0.0, 5.0, 0.0],
            angular_damping: 0.0,
            ..BodyDesc::default()
        })
        .unwrap();
    world.set_velocity(ball, [3.0, 4.0, 0.0], [0.0; 3]).unwrap();
    run(&mut world, 10);
    let pose = world.pose(ball).unwrap();
    assert!((pose.position[0] - 3.0).abs() < 0.01, "{pose:?}");
    assert!(
        (pose.position[1] - (5.0 + 4.0 - 0.5 * 9.81)).abs() < 0.1,
        "{pose:?}"
    );
    world.set_kind(ball, Kind::Kinematic).unwrap();
    let held = world.pose(ball).unwrap();
    run(&mut world, 5);
    assert_eq!(
        world.pose(ball).unwrap(),
        held,
        "a kinematic ball hangs where it is"
    );
    world.set_kind(ball, Kind::Dynamic).unwrap();
    run(&mut world, 5);
    assert!(
        world.pose(ball).unwrap().position[1] < held.position[1],
        "let go, it falls again"
    );
}

#[test]
fn convex_and_mesh_shapes_work_and_bad_input_is_refused() {
    let mut world = World::new(GRAVITY);
    let pyramid = Shape::Convex {
        points: vec![
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.0, -0.5, 0.5],
            [0.0, 0.5, 0.0],
        ],
    };
    let id = world
        .add(&BodyDesc {
            shape: pyramid,
            position: [0.0, 1.0, 0.0],
            ..BodyDesc::default()
        })
        .unwrap();
    let floor = Shape::Mesh {
        points: vec![[-5.0, 0.0, -5.0], [5.0, 0.0, -5.0], [0.0, 0.0, 5.0]],
        triangles: vec![[0, 2, 1]],
    };
    world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: floor.clone(),
            ..BodyDesc::default()
        })
        .unwrap();
    run(&mut world, 20);
    assert!(
        world.pose(id).unwrap().position[1] > -0.1,
        "the pyramid sits on the mesh floor, not through it"
    );
    assert!(
        matches!(
            world.add(&BodyDesc {
                shape: floor,
                ..BodyDesc::default()
            }),
            Err(Error::BadShape(_))
        ),
        "a moving mesh is refused"
    );
    assert!(
        world
            .add(&BodyDesc {
                shape: Shape::Sphere { radius: 0.0 },
                ..BodyDesc::default()
            })
            .is_err()
    );
    assert!(
        world
            .add(&BodyDesc {
                restitution: 2.0,
                ..BodyDesc::default()
            })
            .is_err()
    );
    assert!(
        world
            .add(&BodyDesc {
                position: [f32::NAN, 0.0, 0.0],
                ..BodyDesc::default()
            })
            .is_err()
    );
    assert!(world.pose(BodyId(99)).is_err());
    let mut full = World::new(GRAVITY);
    for _ in 0..crate::MAX_BODIES {
        full.add(&BodyDesc::default()).unwrap();
    }
    assert_eq!(full.add(&BodyDesc::default()), Err(Error::TooManyBodies));
}
