//! The nearest surface to a small sphere (plan W5/W4): how a particle finds out it is touching a crate, a wall or the floor. Written before the body.
#![expect(clippy::unwrap_used, reason = "fixtures unwrap valid bodies")]
use ve_physics::{BodyDesc, Kind, Layers, Shape, World};

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

fn scene() -> World {
    let mut world = World::new([0.0, -9.81, 0.0]);
    world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: Shape::Box { half: [10.0, 0.5, 10.0] },
            position: [0.0, -0.5, 0.0],
            ..BodyDesc::default()
        })
        .unwrap();
    world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: Shape::Sphere { radius: 1.0 },
            position: [3.0, 1.0, 0.0],
            layers: Layers { member: 2, mask: u32::MAX },
            ..BodyDesc::default()
        })
        .unwrap();
    world
}

#[test]
fn a_sphere_touching_the_floor_gets_the_way_out_and_how_far() {
    let mut world = scene();
    let hit = world.nearest_surface([0.0, 0.3, 0.0], 0.5, u32::MAX).unwrap();
    assert!(near(hit.depth, 0.2), "{hit:?}");
    assert!(near(hit.normal[1], 1.0) && hit.normal[0].abs() < 1e-3, "{hit:?}");
    // Just clear of it: nothing.
    assert!(world.nearest_surface([0.0, 0.6, 0.0], 0.5, u32::MAX).is_none());
}

#[test]
fn a_point_inside_a_solid_is_pushed_out_the_short_way() {
    let mut world = scene();
    // 0.2 below the floor's top face: the way out is up, and the sphere's radius adds to the depth.
    let hit = world.nearest_surface([0.0, -0.2, 0.0], 0.1, u32::MAX).unwrap();
    assert!(near(hit.depth, 0.3), "{hit:?}");
    assert!(near(hit.normal[1], 1.0), "{hit:?}");
}

#[test]
fn curved_and_cornered_surfaces_give_the_right_normals() {
    let mut world = scene();
    let top = world.nearest_surface([3.0, 2.3, 0.0], 0.5, u32::MAX).unwrap();
    assert!(near(top.depth, 0.2) && near(top.normal[1], 1.0), "{top:?}");
    let side = world.nearest_surface([4.3, 1.0, 0.0], 0.5, u32::MAX).unwrap();
    assert!(near(side.depth, 0.2) && near(side.normal[0], 1.0), "{side:?}");
    let slant = world.nearest_surface([3.0 + 0.75, 1.0 + 0.75, 0.0], 0.2, u32::MAX).unwrap();
    // Towards the diagonal: x and y equal, unit length.
    assert!((slant.normal[0] - slant.normal[1]).abs() < 1e-3 && slant.normal[0] > 0.6, "{slant:?}");
    let length = slant.normal.iter().map(|c| c * c).sum::<f32>().sqrt();
    assert!(near(length, 1.0));
}

#[test]
fn the_layer_mask_picks_what_counts_and_bad_input_finds_nothing() {
    let mut world = scene();
    // Only the ball is on group 2: a mask of 2 ignores the floor, a mask of 1 ignores the ball.
    assert!(world.nearest_surface([0.0, 0.3, 0.0], 0.5, 2).is_none());
    assert!(world.nearest_surface([0.0, 0.3, 0.0], 0.5, 1).is_some());
    assert!(world.nearest_surface([3.0, 2.3, 0.0], 0.5, 1).is_none());
    assert!(world.nearest_surface([f32::NAN, 0.0, 0.0], 0.5, u32::MAX).is_none());
    assert!(world.nearest_surface([0.0, 0.3, 0.0], f32::INFINITY, u32::MAX).is_none());
    assert!(world.nearest_surface([0.0, 0.3, 0.0], -1.0, u32::MAX).is_none());
    // A zero radius is a point: touching only when inside.
    assert!(world.nearest_surface([0.0, 0.3, 0.0], 0.0, u32::MAX).is_none());
    assert!(world.nearest_surface([0.0, -0.1, 0.0], 0.0, u32::MAX).is_some());
}

#[test]
fn the_nearest_surface_reports_which_body() {
    let mut world = scene();
    let floor = world.nearest_surface([0.0, 0.3, 0.0], 0.5, u32::MAX).unwrap().body;
    let ball = world.nearest_surface([3.0, 2.3, 0.0], 0.5, u32::MAX).unwrap().body;
    assert_ne!(floor, ball);
}
