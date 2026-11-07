//! Raycast-wheel vehicles (plan W5): a chassis body with wheels that hold it up on springs and push it along with tyre friction.
//! Written before the bodies. The checks are physical: ride height from the weight and the spring, acceleration, braking, cornering without
//! sliding or rolling over, a landing that settles, determinism.
#![expect(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    reason = "fixtures unwrap valid data and count small loops"
)]
use ve_physics::vehicle::{Vehicle, VehicleDesc, Wheel};
use ve_physics::{BodyDesc, BodyId, Error, Kind, Shape, World};

const GRAVITY: [f32; 3] = [0.0, -9.81, 0.0];

fn ground(world: &mut World) {
    world
        .add(&BodyDesc {
            kind: Kind::Static,
            shape: Shape::Box {
                half: [200.0, 0.5, 200.0],
            },
            position: [0.0, -0.5, 0.0],
            friction: 0.8,
            ..BodyDesc::default()
        })
        .unwrap();
}

/// A car about 1.8 m wide and 4 m long, 1.2 tonnes, on four wheels; +Z is forward, +Y up.
fn car(world: &mut World, at: [f32; 3]) -> (BodyId, VehicleDesc) {
    let chassis = world
        .add(&BodyDesc {
            shape: Shape::Box {
                half: [0.9, 0.25, 2.0],
            },
            position: at,
            density: 330.0,
            friction: 0.2,
            angular_damping: 0.5,
            ..BodyDesc::default()
        })
        .unwrap();
    let wheel = |x: f32, z: f32, steers: bool, drives: bool| Wheel {
        offset: [x, -0.15, z],
        radius: 0.35,
        suspension: 0.35,
        stiffness: 30_000.0,
        damping: 4_000.0,
        grip: 1.1,
        steers,
        drives,
    };
    let desc = VehicleDesc {
        chassis,
        wheels: vec![
            wheel(-0.85, 1.3, true, false),
            wheel(0.85, 1.3, true, false),
            wheel(-0.85, -1.3, false, true),
            wheel(0.85, -1.3, false, true),
        ],
        engine_force: 5_000.0,
        brake_force: 12_000.0,
        max_steer: 0.5,
    };
    (chassis, desc)
}

fn run(world: &mut World, vehicle: &mut Vehicle, seconds: f32) {
    for _ in 0..(seconds * 60.0) as u32 {
        vehicle.update(world);
        world.step();
    }
}

fn forward(world: &World, id: BodyId) -> [f32; 3] {
    let q = world.pose(id).unwrap().rotation;
    // Rotate +Z by q.
    [2.0 * (q[0] * q[2] + q[3] * q[1]), 2.0 * (q[1] * q[2] - q[3] * q[0]), 1.0 - 2.0 * (q[0] * q[0] + q[1] * q[1])]
}

fn up_y(world: &World, id: BodyId) -> f32 {
    let q = world.pose(id).unwrap().rotation;
    1.0 - 2.0 * (q[0] * q[0] + q[2] * q[2])
}

fn speed_along(world: &World, id: BodyId, direction: [f32; 3]) -> f32 {
    let v = world.velocity(id).unwrap();
    v[0] * direction[0] + v[1] * direction[1] + v[2] * direction[2]
}

#[test]
fn a_car_settles_on_its_springs_at_a_ride_height_set_by_its_weight() {
    let mut world = World::new(GRAVITY);
    ground(&mut world);
    let (chassis, desc) = car(&mut world, [0.0, 1.2, 0.0]);
    let mut vehicle = Vehicle::new(&mut world, &desc).unwrap();
    run(&mut world, &mut vehicle, 3.0);
    let y = world.pose(chassis).unwrap().position[1];
    // Rest compression x: 4 wheels * k * x = weight, so x = m g / (4 k) with m = 330 * 3.6 kg.
    let weight = 330.0 * 1.8 * 0.5 * 4.0 * 9.81;
    let compression = weight / (4.0 * 30_000.0);
    // Wheel hub rests 0.15 under the chassis centre; the wheel's bottom is `suspension + radius - compression` below the connection point.
    let expected = 0.35 + 0.35 - compression + 0.15;
    assert!((y - expected).abs() < 0.06, "ride height {y}, expected about {expected}");
    for i in 0..4 {
        assert!(vehicle.wheel(i).in_contact, "wheel {i} is on the ground");
    }
    // At rest means it stays where it is: no drift sideways or lengthwise, and the height does not change over the next second. (A velocity
    // read right after a step shows up to one step of gravity, g / 60 = 0.16 m/s, because the springs push before the world adds gravity;
    // that is not motion, so the vertical reading is held to that bound and the height is what is checked.)
    let before = world.pose(chassis).unwrap().position;
    run(&mut world, &mut vehicle, 1.0);
    let after = world.pose(chassis).unwrap().position;
    let moved = [after[0] - before[0], after[1] - before[1], after[2] - before[2]];
    assert!(moved.iter().all(|c| c.abs() < 0.005), "it moved by {moved:?} in a second");
    let v = world.velocity(chassis).unwrap();
    assert!(v[0].abs() < 0.02 && v[2].abs() < 0.02 && v[1].abs() < 0.1, "at rest: {v:?}");
    assert!(up_y(&world, chassis) > 0.99, "level");
}

#[test]
fn throttle_drives_it_forward_to_a_sensible_speed_and_no_throttle_coasts_to_a_stop() {
    let mut world = World::new(GRAVITY);
    ground(&mut world);
    let (chassis, desc) = car(&mut world, [0.0, 1.0, 0.0]);
    let mut vehicle = Vehicle::new(&mut world, &desc).unwrap();
    run(&mut world, &mut vehicle, 1.0);
    vehicle.drive(1.0, 0.0, 0.0);
    run(&mut world, &mut vehicle, 3.0);
    let heading = forward(&world, chassis);
    let speed = speed_along(&world, chassis, heading);
    // 5 kN on 1.2 t is about 4 m/s^2 before drag and rolling resistance: about 12 m/s after 3 s, well below 20.
    assert!(speed > 6.0 && speed < 20.0, "speed after 3 s of throttle: {speed}");
    let z = world.pose(chassis).unwrap().position[2];
    assert!(z > 8.0, "it travelled forward: z = {z}");
    vehicle.drive(0.0, 0.0, 0.0);
    run(&mut world, &mut vehicle, 12.0);
    let slow = speed_along(&world, chassis, forward(&world, chassis));
    assert!(slow < speed * 0.5, "coasting slows it: {speed} -> {slow}");
}

#[test]
fn brakes_stop_it_and_reverse_throttle_backs_it_up() {
    let mut world = World::new(GRAVITY);
    ground(&mut world);
    let (chassis, desc) = car(&mut world, [0.0, 1.0, 0.0]);
    let mut vehicle = Vehicle::new(&mut world, &desc).unwrap();
    run(&mut world, &mut vehicle, 1.0);
    vehicle.drive(1.0, 0.0, 0.0);
    run(&mut world, &mut vehicle, 3.0);
    vehicle.drive(0.0, 0.0, 1.0);
    run(&mut world, &mut vehicle, 4.0);
    let v = speed_along(&world, chassis, forward(&world, chassis));
    assert!(v.abs() < 0.5, "stopped by the brakes: {v}");
    vehicle.drive(-1.0, 0.0, 0.0);
    run(&mut world, &mut vehicle, 3.0);
    let back = speed_along(&world, chassis, forward(&world, chassis));
    assert!(back < -2.0, "reversing: {back}");
}

#[test]
fn steering_turns_it_without_sliding_sideways_or_rolling_over() {
    let mut world = World::new(GRAVITY);
    ground(&mut world);
    let (chassis, desc) = car(&mut world, [0.0, 1.0, 0.0]);
    let mut vehicle = Vehicle::new(&mut world, &desc).unwrap();
    run(&mut world, &mut vehicle, 1.0);
    vehicle.drive(0.6, 0.0, 0.0);
    run(&mut world, &mut vehicle, 2.0);
    let before = forward(&world, chassis);
    vehicle.drive(0.4, 1.0, 0.0);
    let mut lowest_up = 1.0_f32;
    let mut worst_slide = 0.0_f32;
    for _ in 0..(3.0 * 60.0) as u32 {
        vehicle.update(&mut world);
        world.step();
        lowest_up = lowest_up.min(up_y(&world, chassis));
        let heading = forward(&world, chassis);
        let v = world.velocity(chassis).unwrap();
        let along = v[0] * heading[0] + v[2] * heading[2];
        let side = (v[0] * heading[2] - v[2] * heading[0]).abs();
        if along > 3.0 {
            worst_slide = worst_slide.max(side / along);
        }
    }
    let after = forward(&world, chassis);
    // Positive steer turns towards +X (right when facing +Z).
    let turned = before[2] * after[2] + before[0] * after[0];
    assert!(turned < 0.7, "the heading changed: dot {turned}");
    assert!(after[0] > before[0] + 0.3, "it turned towards the steered side: {before:?} -> {after:?}");
    assert!(lowest_up > 0.85, "it did not roll: up {lowest_up}");
    assert!(worst_slide < 0.35, "the tyres grip: sideways speed was {worst_slide} of forward");
}

#[test]
fn dropped_from_height_it_lands_on_its_wheels_and_settles() {
    let mut world = World::new(GRAVITY);
    ground(&mut world);
    let (chassis, desc) = car(&mut world, [0.0, 2.5, 0.0]);
    let mut vehicle = Vehicle::new(&mut world, &desc).unwrap();
    // In the air no wheel touches anything.
    run(&mut world, &mut vehicle, 0.2);
    assert!((0..4).all(|i| !vehicle.wheel(i).in_contact), "airborne");
    run(&mut world, &mut vehicle, 5.0);
    assert!((0..4).all(|i| vehicle.wheel(i).in_contact));
    assert!(up_y(&world, chassis) > 0.98, "landed level");
    let v = world.velocity(chassis).unwrap();
    assert!(v.iter().all(|c| c.abs() < 0.1), "settled: {v:?}");
}

#[test]
fn the_same_drive_gives_the_same_car_every_time() {
    let run_once = || {
        let mut world = World::new(GRAVITY);
        ground(&mut world);
        let (chassis, desc) = car(&mut world, [0.0, 1.0, 0.0]);
        let mut vehicle = Vehicle::new(&mut world, &desc).unwrap();
        run(&mut world, &mut vehicle, 1.0);
        vehicle.drive(1.0, 0.5, 0.0);
        run(&mut world, &mut vehicle, 3.0);
        let p = world.pose(chassis).unwrap();
        p.position.iter().chain(&p.rotation).map(|v| v.to_bits()).collect::<Vec<u32>>()
    };
    assert_eq!(run_once(), run_once());
}

#[test]
fn bad_vehicles_are_refused_with_a_reason() {
    let mut world = World::new(GRAVITY);
    ground(&mut world);
    let (_, good) = car(&mut world, [0.0, 1.0, 0.0]);
    let refuse = |world: &mut World, edit: &dyn Fn(&mut VehicleDesc)| {
        let mut d = good.clone();
        edit(&mut d);
        Vehicle::new(world, &d).err().unwrap()
    };
    assert!(matches!(refuse(&mut world, &|d| d.wheels.clear()), Error::BadValue(_)), "no wheels");
    assert!(matches!(refuse(&mut world, &|d| d.wheels = vec![d.wheels[0]; 9]), Error::BadValue(_)), "too many wheels");
    assert!(matches!(refuse(&mut world, &|d| d.wheels[0].radius = 0.0), Error::BadValue(_)), "radius");
    assert!(matches!(refuse(&mut world, &|d| d.wheels[1].stiffness = -1.0), Error::BadValue(_)), "stiffness");
    assert!(matches!(refuse(&mut world, &|d| d.wheels[2].suspension = f32::NAN), Error::BadValue(_)), "finite");
    assert!(matches!(refuse(&mut world, &|d| d.engine_force = -5.0), Error::BadValue(_)), "engine");
    assert!(matches!(refuse(&mut world, &|d| d.max_steer = 2.0), Error::BadValue(_)), "steering angle");
    assert!(matches!(refuse(&mut world, &|d| d.chassis = BodyId(99)), Error::NoSuchBody), "chassis must exist");
    // Inputs outside their range are clamped, not refused: a joystick can overshoot.
    let mut vehicle = Vehicle::new(&mut world, &good).unwrap();
    vehicle.drive(5.0, -9.0, 3.0);
    vehicle.update(&mut world);
    world.step();
}
