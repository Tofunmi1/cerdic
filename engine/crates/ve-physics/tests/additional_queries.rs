//! Query coverage for shapes and layer settings outside the public contract suite.
use ve_physics::{BodyDesc, Kind, Layers, Shape, World};

#[test]
fn mesh_bodies_can_be_raycast() {
    let mut world = World::new([0.0, -9.81, 0.0]);
    let mesh = Shape::Mesh {
        points: vec![[-2.0, 0.0, -2.0], [2.0, 0.0, -2.0], [0.0, 0.0, 2.0]],
        triangles: vec![[0, 2, 1]],
    };
    let id = world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: mesh,
            ..BodyDesc::default()
        })
        .unwrap();
    let hit = world
        .raycast([0.0, 3.0, 0.0], [0.0, -2.0, 0.0], 5.0, u32::MAX)
        .unwrap();
    assert_eq!(hit.body, id);
    assert!((hit.point[1]).abs() < 1.0e-4);
}

#[test]
fn capsule_cast_respects_query_membership_even_if_body_filter_is_narrow() {
    let mut world = World::new([0.0, -9.81, 0.0]);
    let target = world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: Shape::Box { half: [0.5; 3] },
            position: [0.0, 0.0, 0.0],
            layers: Layers { member: 4, mask: 0 },
            ..BodyDesc::default()
        })
        .unwrap();
    let hit = world
        .shape_cast(
            &Shape::Capsule {
                half_height: 0.5,
                radius: 0.25,
            },
            [0.0, 3.0, 0.0],
            [0.0, -1.0, 0.0],
            5.0,
            4,
        )
        .unwrap();
    assert_eq!(hit.body, target);
    assert!((hit.distance - 1.75).abs() < 1.0e-2);
}

#[test]
#[expect(
    clippy::cast_precision_loss,
    reason = "these small indices only place the performance fixture"
)]
fn thousands_of_bodies_step_in_a_measured_release_window() {
    let mut world = World::new([0.0, -9.81, 0.0]);
    for index in 0..2000 {
        let x = (index % 50) as f32 * 1.5;
        let z = (index / 50) as f32 * 1.5;
        world
            .add(&BodyDesc {
                kind: Kind::Static,
                shape: Shape::Box { half: [0.5; 3] },
                position: [x, 0.0, z],
                ..BodyDesc::default()
            })
            .expect("valid fixture body");
    }
    for index in 0..200 {
        world
            .add(&BodyDesc {
                shape: Shape::Sphere { radius: 0.25 },
                position: [index as f32 * 1.5, 5.0, 0.0],
                ..BodyDesc::default()
            })
            .expect("valid fixture body");
    }
    let start = std::time::Instant::now();
    for _ in 0..60 {
        world.step();
    }
    eprintln!(
        "2,000 static + 200 dynamic bodies: 60 steps in {:?}",
        start.elapsed()
    );
}
