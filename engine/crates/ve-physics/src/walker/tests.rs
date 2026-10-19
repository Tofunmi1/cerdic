use super::*;
use crate::{BodyDesc, Kind, Shape};

fn floor(world: &mut World) {
    world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: Shape::Box {
                half: [20.0, 0.5, 20.0],
            },
            position: [0.0, -0.5, 0.0],
            ..BodyDesc::default()
        })
        .unwrap();
}

fn stand(world: &mut World, steps: u32) -> WalkerState {
    let mut state = world.walk([0.0, 0.0], None).unwrap();
    for _ in 0..steps {
        state = world.walk([0.0, 0.0], None).unwrap();
    }
    state
}

#[test]
fn the_ground_below_a_point_is_the_highest_surface_under_it() {
    let mut world = World::new([0.0, -9.81, 0.0]);
    floor(&mut world);
    world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: Shape::Box {
                half: [1.0, 1.0, 1.0],
            },
            position: [5.0, 1.0, 0.0],
            ..BodyDesc::default()
        })
        .unwrap();
    assert!(
        (world.ground_below(0.0, 0.0, 50.0).unwrap() - 0.0).abs() < 1e-3,
        "open floor"
    );
    assert!(
        (world.ground_below(5.0, 0.0, 50.0).unwrap() - 2.0).abs() < 1e-3,
        "the top of the block"
    );
    assert!(
        world.ground_below(100.0, 0.0, 50.0).is_none(),
        "nothing beyond the floor"
    );
    assert!(world.ground_below(f32::NAN, 0.0, 50.0).is_none());
}

#[test]
fn a_walker_drops_onto_the_floor_and_stands_on_it() {
    let mut world = World::new([0.0, -9.81, 0.0]);
    floor(&mut world);
    world
        .set_walker(&WalkerDesc {
            position: [0.0, 2.0, 0.0],
            height: 1.7,
            radius: 0.3,
        })
        .unwrap();
    let state = stand(&mut world, 90);
    assert!(state.grounded, "{state:?}");
    assert!(
        (state.position[1] - 0.85).abs() < 0.03,
        "the centre of a 1.7 m walker rests 0.85 m up: {state:?}"
    );
}

#[test]
fn a_walker_walks_at_speed_stops_at_a_wall_and_climbs_a_small_step() {
    let mut world = World::new([0.0, -9.81, 0.0]);
    floor(&mut world);
    world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: Shape::Box {
                half: [0.1, 1.0, 5.0],
            },
            position: [4.0, 1.0, 0.0],
            ..BodyDesc::default()
        })
        .unwrap();
    world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: Shape::Box {
                half: [0.5, 0.1, 5.0],
            },
            position: [2.0, 0.1, 0.0],
            ..BodyDesc::default()
        })
        .unwrap();
    world
        .set_walker(&WalkerDesc {
            position: [0.0, 0.9, 0.0],
            height: 1.7,
            radius: 0.3,
        })
        .unwrap();
    stand(&mut world, 10);
    let mut state = world.walk([0.0, 0.0], None).unwrap();
    for _ in 0..60 {
        state = world.walk([2.0, 0.0], None).unwrap();
    }
    assert!(
        (state.position[0] - 2.0).abs() < 0.2,
        "2 m/s for a second goes about 2 m: {state:?}"
    );
    assert!(
        state.position[1] > 0.9,
        "and it stepped up onto the 0.2 m kerb: {state:?}"
    );
    for _ in 0..240 {
        state = world.walk([2.0, 0.0], None).unwrap();
    }
    assert!(
        state.position[0] < 3.7,
        "the wall at x=3.9 stops it: {state:?}"
    );
}

#[test]
fn a_jump_rises_and_comes_back_down_and_bad_input_is_refused() {
    let mut world = World::new([0.0, -9.81, 0.0]);
    floor(&mut world);
    world
        .set_walker(&WalkerDesc {
            position: [0.0, 0.9, 0.0],
            height: 1.7,
            radius: 0.3,
        })
        .unwrap();
    stand(&mut world, 30);
    let ground = world.walk([0.0, 0.0], None).unwrap().position[1];
    let mut top = ground;
    let mut state = world.walk([0.0, 0.0], Some(4.5)).unwrap();
    for _ in 0..90 {
        top = top.max(state.position[1]);
        state = world.walk([0.0, 0.0], None).unwrap();
    }
    assert!(
        top > ground + 0.8 && top < ground + 1.3,
        "a 4.5 m/s jump rises about 1 m: {}",
        top - ground
    );
    assert!(
        state.grounded && (state.position[1] - ground).abs() < 0.03,
        "and lands again: {state:?}"
    );
    assert!(world.walk([f32::NAN, 0.0], None).is_err());
    assert!(
        world
            .set_walker(&WalkerDesc {
                position: [0.0; 3],
                height: 0.2,
                radius: 0.3
            })
            .is_err()
    );
    let mut empty = World::new([0.0; 3]);
    assert!(
        empty.walk([0.0, 0.0], None).is_err(),
        "no walker, no walking"
    );
}
