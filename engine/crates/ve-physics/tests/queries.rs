//! Spatial queries, collision layers, sensors, heightfield colliders, kinematic movers and body capacity. Written before the
//! bodies; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    reason = "fixtures unwrap valid bodies and turn small grid indices into metres"
)]
use ve_physics::{BodyDesc, BodyId, Error, Kind, Layers, MAX_BODIES, Shape, World};

const DOWN: [f32; 3] = [0.0, -1.0, 0.0];

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// A static floor whose top face is at y = 0.
fn floor(world: &mut World) -> BodyId {
    world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: Shape::Box {
                half: [10.0, 0.5, 10.0],
            },
            position: [0.0, -0.5, 0.0],
            ..BodyDesc::default()
        })
        .unwrap()
}

fn ball(world: &mut World, position: [f32; 3], radius: f32, layers: Layers) -> BodyId {
    world
        .add(&BodyDesc {
            shape: Shape::Sphere { radius },
            position,
            layers,
            ..BodyDesc::default()
        })
        .unwrap()
}

fn world() -> World {
    World::new([0.0, -9.81, 0.0])
}

#[test]
fn a_ray_reports_the_nearest_body_with_distance_point_and_normal() {
    let mut world = world();
    let ground = floor(&mut world);
    // Queries work straight after adding, before any step.
    let hit = world
        .raycast([0.0, 5.0, 0.0], DOWN, 20.0, u32::MAX)
        .unwrap();
    assert_eq!(hit.body, ground);
    assert!(near(hit.distance, 5.0, 1e-3), "distance {}", hit.distance);
    assert!(
        near(hit.point[1], 0.0, 1e-3) && near(hit.normal[1], 1.0, 1e-3),
        "{hit:?}"
    );
    // A ball hanging in the way is hit first, on its top.
    let hanging = world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: Shape::Sphere { radius: 0.5 },
            position: [0.0, 2.0, 0.0],
            ..BodyDesc::default()
        })
        .unwrap();
    let hit = world
        .raycast([0.0, 5.0, 0.0], DOWN, 20.0, u32::MAX)
        .unwrap();
    assert_eq!(hit.body, hanging);
    assert!(near(hit.distance, 2.5, 1e-3), "distance {}", hit.distance);
}

#[test]
fn a_ray_ignores_what_is_beyond_its_length_and_what_it_cannot_describe() {
    let mut world = world();
    floor(&mut world);
    assert!(
        world
            .raycast([0.0, 5.0, 0.0], DOWN, 4.0, u32::MAX)
            .is_none(),
        "the floor is 5 m away"
    );
    assert!(
        world
            .raycast([0.0, 5.0, 0.0], [0.0; 3], 20.0, u32::MAX)
            .is_none(),
        "a zero direction hits nothing"
    );
    assert!(
        world
            .raycast([f32::NAN, 5.0, 0.0], DOWN, 20.0, u32::MAX)
            .is_none(),
        "a non-finite origin hits nothing"
    );
    // The direction need not be a unit vector.
    let hit = world
        .raycast([0.0, 5.0, 0.0], [0.0, -3.0, 0.0], 20.0, u32::MAX)
        .unwrap();
    assert!(
        near(hit.distance, 5.0, 1e-3),
        "distance is in metres, not in direction lengths: {}",
        hit.distance
    );
}

#[test]
fn a_query_mask_skips_bodies_that_are_not_in_a_wanted_layer() {
    let mut world = world();
    let ground = floor(&mut world);
    let layer_two = Layers {
        member: 2,
        mask: u32::MAX,
    };
    ball(&mut world, [0.0, 2.0, 0.0], 0.5, layer_two);
    let any = world
        .raycast([0.0, 5.0, 0.0], DOWN, 20.0, u32::MAX)
        .unwrap();
    assert_ne!(any.body, ground, "the ball is nearer");
    let only_default = world.raycast([0.0, 5.0, 0.0], DOWN, 20.0, 1).unwrap();
    assert_eq!(
        only_default.body, ground,
        "a mask of layer 1 does not see the layer 2 ball"
    );
}

#[test]
fn an_overlap_lists_the_bodies_in_a_sphere_in_id_order() {
    let mut world = world();
    let far = ball(&mut world, [8.0, 1.0, 0.0], 0.5, Layers::default());
    let near_a = ball(&mut world, [0.5, 1.0, 0.0], 0.5, Layers::default());
    let near_b = ball(&mut world, [-0.5, 1.0, 0.3], 0.5, Layers::default());
    let found = world.overlap_sphere([0.0, 1.0, 0.0], 1.5, u32::MAX);
    assert_eq!(
        found,
        vec![near_a, near_b],
        "ids ascending, the far ball excluded (far is {far:?})"
    );
    assert!(
        world.overlap_sphere([0.0, 1.0, 0.0], 1.5, 0b100).is_empty(),
        "no body is in layer 3"
    );
}

#[test]
fn a_shape_cast_stops_where_the_swept_shape_first_touches() {
    let mut world = world();
    let ground = floor(&mut world);
    let hit = world
        .shape_cast(
            &Shape::Sphere { radius: 0.25 },
            [0.0, 5.0, 0.0],
            DOWN,
            20.0,
            u32::MAX,
        )
        .unwrap();
    assert_eq!(hit.body, ground);
    assert!(
        near(hit.distance, 4.75, 1e-2),
        "the sphere's surface meets the floor 0.25 m early: {}",
        hit.distance
    );
    assert!(near(hit.normal[1], 1.0, 1e-2), "{hit:?}");
    assert!(
        world
            .shape_cast(
                &Shape::Sphere { radius: 0.25 },
                [0.0, 5.0, 0.0],
                DOWN,
                3.0,
                u32::MAX
            )
            .is_none()
    );
}

#[test]
fn collision_layers_decide_which_bodies_meet() {
    let mut world = world();
    floor(&mut world);
    // Both sides must agree: a ball that only collides with layer 2 passes through the layer 1 floor.
    let ghost = ball(
        &mut world,
        [2.0, 3.0, 0.0],
        0.5,
        Layers { member: 2, mask: 2 },
    );
    let solid = ball(&mut world, [-2.0, 3.0, 0.0], 0.5, Layers::default());
    for _ in 0..180 {
        world.step();
    }
    assert!(
        world.pose(ghost).unwrap().position[1] < -3.0,
        "the ghost fell through the floor"
    );
    assert!(
        near(world.pose(solid).unwrap().position[1], 0.5, 0.05),
        "the ordinary ball rests on the floor"
    );
}

#[test]
fn a_sensor_reports_what_is_inside_without_blocking_it() {
    let mut world = world();
    let zone = world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: Shape::Box {
                half: [1.0, 1.0, 1.0],
            },
            position: [0.0, 0.0, 0.0],
            sensor: true,
            ..BodyDesc::default()
        })
        .unwrap();
    let faller = ball(&mut world, [0.0, 4.0, 0.0], 0.25, Layers::default());
    let mut seen = false;
    for _ in 0..120 {
        world.step();
        seen |= world.touching(zone).unwrap().contains(&faller);
    }
    assert!(seen, "the ball was inside the zone at some step");
    assert!(
        world.pose(faller).unwrap().position[1] < -2.0,
        "a sensor does not stop the ball"
    );
    assert!(
        world
            .contacts()
            .iter()
            .all(|(a, b)| *a != zone && *b != zone),
        "a sensor makes no contact"
    );
    assert!(
        world.touching(faller).is_err() || world.touching(faller).unwrap().is_empty(),
        "only sensors have occupants"
    );
}

/// A heightfield of `n` by `n` samples spanning 20 m by 20 m: samples run row by row, rows along +z, columns along +x, and
/// the field is centred on the body, so the sample at column `c` sits at x = -10 + 20 c / (n - 1).
fn ramp(n: usize, rise: f32) -> Shape {
    let heights = (0..n * n)
        .map(|i| (i % n) as f32 / (n - 1) as f32 * rise)
        .collect();
    Shape::Heightfield {
        rows: n,
        cols: n,
        heights,
        size: [20.0, 20.0],
    }
}

#[test]
fn a_heightfield_collider_follows_its_samples() {
    let mut world = world();
    let hills = world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: ramp(21, 2.0),
            ..BodyDesc::default()
        })
        .unwrap();
    // Height rises 2 m across x from -10 to 10, so it is 1.0 at x = 0 and 1.5 at x = 5.
    for (x, expected) in [(0.0, 1.0), (5.0, 1.5), (-5.0, 0.5)] {
        let hit = world.raycast([x, 10.0, 3.0], DOWN, 30.0, u32::MAX).unwrap();
        assert_eq!(hit.body, hills);
        assert!(
            near(hit.point[1], expected, 0.05),
            "height at x = {x} is {} not {expected}",
            hit.point[1]
        );
    }
    assert!(
        world
            .raycast([30.0, 10.0, 0.0], DOWN, 30.0, u32::MAX)
            .is_none(),
        "nothing beyond the edge"
    );
}

#[test]
fn a_ball_rolls_on_a_flat_heightfield_and_comes_to_rest_on_it() {
    let mut world = world();
    world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: ramp(11, 0.0),
            ..BodyDesc::default()
        })
        .unwrap();
    let rider = ball(&mut world, [1.0, 3.0, 1.0], 0.5, Layers::default());
    for _ in 0..240 {
        world.step();
    }
    assert!(
        near(world.pose(rider).unwrap().position[1], 0.5, 0.05),
        "{:?}",
        world.pose(rider).unwrap()
    );
}

#[test]
fn heightfields_are_refused_when_malformed_or_on_moving_bodies() {
    let mut world = world();
    let short = Shape::Heightfield {
        rows: 3,
        cols: 3,
        heights: vec![0.0; 8],
        size: [4.0, 4.0],
    };
    assert!(matches!(
        world.add(&BodyDesc {
            kind: Kind::Static,
            shape: short,
            ..BodyDesc::default()
        }),
        Err(Error::BadShape(_))
    ));
    let flat = Shape::Heightfield {
        rows: 1,
        cols: 3,
        heights: vec![0.0; 3],
        size: [4.0, 4.0],
    };
    assert!(
        matches!(
            world.add(&BodyDesc {
                kind: Kind::Static,
                shape: flat,
                ..BodyDesc::default()
            }),
            Err(Error::BadShape(_))
        ),
        "one row has no area"
    );
    assert!(matches!(
        world.add(&BodyDesc {
            kind: Kind::Dynamic,
            shape: ramp(5, 1.0),
            ..BodyDesc::default()
        }),
        Err(Error::BadShape(_))
    ));
    let nan = Shape::Heightfield {
        rows: 2,
        cols: 2,
        heights: vec![0.0, f32::NAN, 0.0, 0.0],
        size: [4.0, 4.0],
    };
    assert!(
        world
            .add(&BodyDesc {
                kind: Kind::Static,
                shape: nan,
                ..BodyDesc::default()
            })
            .is_err()
    );
}

#[test]
fn a_moved_kinematic_platform_carries_what_stands_on_it() {
    let mut world = world();
    let platform = world
        .add(&BodyDesc {
            kind: Kind::Kinematic,
            shape: Shape::Box {
                half: [2.0, 0.2, 2.0],
            },
            position: [0.0, 0.0, 0.0],
            friction: 1.0,
            ..BodyDesc::default()
        })
        .unwrap();
    let crate_ = world
        .add(&BodyDesc {
            shape: Shape::Box {
                half: [0.25, 0.25, 0.25],
            },
            position: [0.0, 0.46, 0.0],
            friction: 1.0,
            ..BodyDesc::default()
        })
        .unwrap();
    for _ in 0..30 {
        world.step();
    }
    for step in 1..=120 {
        world
            .move_kinematic(
                platform,
                [0.0, 0.01 * step as f32, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            )
            .unwrap();
        world.step();
    }
    let lift = world.pose(platform).unwrap().position[1];
    let carried = world.pose(crate_).unwrap().position[1];
    assert!(near(lift, 1.2, 1e-3), "the platform went up 1.2 m: {lift}");
    assert!(
        near(carried, lift + 0.45, 0.06),
        "the crate rode up with it: crate {carried}, platform {lift}"
    );
    assert!(
        world
            .move_kinematic(crate_, [0.0; 3], [0.0, 0.0, 0.0, 1.0])
            .is_err(),
        "only kinematic bodies can be moved this way"
    );
}

#[test]
fn a_world_holds_thousands_of_bodies_and_refuses_more_than_its_cap() {
    assert!(
        std::hint::black_box(MAX_BODIES) >= 4096,
        "a world needs room for props: {MAX_BODIES}"
    );
    let mut world = world();
    for i in 0..2000 {
        let x = (i % 50) as f32 * 1.5;
        let z = (i / 50) as f32 * 1.5;
        world
            .add(&BodyDesc {
                kind: Kind::Static,
                shape: Shape::Box { half: [0.5; 3] },
                position: [x, 0.0, z],
                ..BodyDesc::default()
            })
            .unwrap();
    }
    assert_eq!(world.len(), 2000);
    // Queries still find the right one among thousands.
    let hit = world
        .raycast([3.0, 5.0, 4.5], DOWN, 20.0, u32::MAX)
        .unwrap();
    assert!(
        near(hit.point[1], 0.5, 1e-3) && near(hit.point[0], 3.0, 1e-3),
        "{hit:?}"
    );
    for _ in 0..MAX_BODIES - 2000 {
        world
            .add(&BodyDesc {
                kind: Kind::Static,
                shape: Shape::Box { half: [0.1; 3] },
                position: [0.0, 50.0, 0.0],
                ..BodyDesc::default()
            })
            .unwrap();
    }
    assert_eq!(world.add(&BodyDesc::default()), Err(Error::TooManyBodies));
}

#[test]
fn the_same_inputs_give_the_same_bits_including_query_results() {
    let run = || {
        let mut world = world();
        floor(&mut world);
        let ids: Vec<BodyId> = (0..40)
            .map(|i| {
                ball(
                    &mut world,
                    [
                        (i % 8) as f32 * 0.9 - 3.0,
                        1.0 + (i / 8) as f32,
                        (i % 3) as f32,
                    ],
                    0.4,
                    Layers::default(),
                )
            })
            .collect();
        for _ in 0..200 {
            world.step();
        }
        let poses: Vec<[f32; 3]> = ids
            .iter()
            .map(|id| world.pose(*id).unwrap().position)
            .collect();
        (
            poses,
            world
                .raycast([0.3, 9.0, 0.2], DOWN, 30.0, u32::MAX)
                .unwrap()
                .point,
        )
    };
    assert_eq!(run(), run());
}
