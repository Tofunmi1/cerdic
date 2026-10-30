//! Exercise the actual browser simulation system on a native ECS world.
#![expect(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    reason = "test fixtures and bounded frame counts"
)]
use bevy_asset::{Assets, RenderAssetUsages};
use bevy_ecs::prelude::*;
use bevy_mesh::{Mesh, Mesh3d, PrimitiveTopology};
use bevy_transform::components::GlobalTransform;
use serde_json::json;
use ve_render::{SurfaceWater, VeMaterial, water_sim::WaterSimTextures};

#[path = "../src/animation/clock.rs"]
mod animation;
mod projection {
    use bevy_ecs::prelude::*;
    #[derive(Component)]
    pub(crate) struct Authored;
    impl Authored {
        pub(crate) fn name(&self) -> &str {
            "lake"
        }
    }
}
mod fx_bursts {
    thread_local! { pub(crate) static REQUESTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
    pub(crate) fn request(_: &str, _: [f64; 3], _: f32) {
        REQUESTED.with(|n| n.set(n.get() + 1));
    }
}
#[path = "../src/water_sim.rs"]
mod water_sim;

fn fixture() -> World {
    let mut world = World::new();
    world.insert_resource(animation::AnimationClock {
        seconds: 0.0,
        playing: true,
        speed: 1.0,
    });
    world.init_resource::<WaterSimTextures>();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 10.0],
            [10.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [0.0, 0.0, 10.0],
            [10.0, 0.0, 10.0],
        ],
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.5, 0.5, 0.5, 0.0]; 6]);
    let mut assets = Assets::<Mesh>::default();
    let handle = assets.add(mesh);
    world.insert_resource(assets);
    world.spawn((
        projection::Authored,
        Mesh3d(handle),
        GlobalTransform::default(),
        VeMaterial {
            recipe: ve_render::ProceduralRecipe::Water,
            finish: ve_render::SurfaceFinish {
                water: Some(SurfaceWater {
                    sim: true,
                    sim_resolution: 32,
                    ..SurfaceWater::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        },
    ));
    world
}

#[test]
fn filled_water_uses_the_integrated_three_dimensional_solver() {
    let mut world = fixture();
    for mut material in world.query::<&mut VeMaterial>().iter_mut(&mut world) {
        if let Some(water) = material.finish.water.as_mut() {
            water.volumetric = true;
        }
    }
    let status = water_sim::method(&mut world, "water.status", &json!({})).unwrap();
    let part = &status["parts"][0];
    assert_eq!(part["solver"], "3d_pbf_reference");
    assert!(part["particles"].as_u64().unwrap() > 0);

    let disturbance = json!({"x":5,"z":5,"amount":0.2,"radius":1.2,"splash":false});
    assert_eq!(
        water_sim::method(&mut world, "water.disturb", &disturbance).unwrap()["disturbed"],
        1
    );
    for frame in 1..=4 {
        world.resource_mut::<animation::AnimationClock>().seconds = frame as f32 / 60.0;
        water_sim::update(&mut world);
    }
    let status = water_sim::method(&mut world, "water.status", &json!({})).unwrap();
    assert!(status["parts"][0]["steps"].as_u64().unwrap() >= 4);
    assert!(status["parts"][0]["max_abs"].as_f64().unwrap() > 0.0);
}

#[test]
fn status_early_pokes_live_heights_and_pause_resume() {
    let mut world = fixture();
    let status = water_sim::method(&mut world, "water.status", &json!({})).unwrap();
    let part = &status["parts"][0];
    assert_eq!(part["name"], "lake");
    assert_eq!(part["resolution"], 32);
    assert_eq!(part["wet_cells"], 1024);
    assert_eq!(part["steps"], 0);
    assert_eq!(part["time"], 0.0);
    assert_eq!(part["max_abs"], 0.0);
    assert_eq!(part["volume"], 0.0);
    assert_eq!(part["cell"], 0.3125);
    let poke = json!({"x":5,"z":5,"amount":0.12,"radius":1.2,"splash":false});
    water_sim::method(&mut world, "water.disturb", &poke).unwrap();
    assert_eq!(fx_bursts::REQUESTED.with(std::cell::Cell::get), 0);
    let initial = water_sim::method(&mut world, "water.height", &poke).unwrap()["height"]
        .as_f64()
        .unwrap();
    let look = SurfaceWater::default();
    let waves =
        ve_render::water_waves::WaveField::new(look.seed, look.ripple, look.flow, look.direction);
    assert!(initial - f64::from(waves.height(5.0, 5.0, 0.0)) > 0.11);
    for i in 1..=30 {
        world.resource_mut::<animation::AnimationClock>().seconds = i as f32 / 60.0;
        water_sim::update(&mut world);
    }
    let later = water_sim::method(&mut world, "water.height", &poke).unwrap()["height"]
        .as_f64()
        .unwrap();
    assert!((later - initial).abs() > 0.01);
    let before = water_sim::method(&mut world, "water.status", &json!({})).unwrap();
    assert!(before["parts"][0]["steps"].as_u64().unwrap() >= 29);
    world.resource_mut::<animation::AnimationClock>().playing = false;
    water_sim::update(&mut world);
    assert_eq!(
        before,
        water_sim::method(&mut world, "water.status", &json!({})).unwrap()
    );
    water_sim::method(&mut world, "water.disturb", &poke).unwrap();
    world.resource_mut::<animation::AnimationClock>().playing = true;
    world.resource_mut::<animation::AnimationClock>().seconds += 0.1;
    water_sim::update(&mut world);
    let after = water_sim::method(&mut world, "water.status", &json!({})).unwrap();
    assert!(
        after["parts"][0]["steps"].as_u64().unwrap()
            > before["parts"][0]["steps"].as_u64().unwrap()
    );
}

#[test]
fn parsing_splash_defaults_and_empty_status() {
    let mut world = fixture();
    for params in [
        json!({"x":"bad","z":5}),
        json!({"x":5,"z":5,"radius":0}),
        json!({"x":5,"z":5,"splash":0}),
        json!({"x":5,"z":5,"part":4}),
    ] {
        assert!(water_sim::method(&mut world, "water.disturb", &params).is_err());
    }
    water_sim::method(&mut world, "water.disturb", &json!({"x":5,"z":5})).unwrap();
    assert_eq!(fx_bursts::REQUESTED.with(std::cell::Cell::get), 1);
    world
        .query::<&mut VeMaterial>()
        .single_mut(&mut world)
        .unwrap()
        .finish
        .water = None;
    assert_eq!(
        water_sim::method(&mut world, "water.status", &json!({})).unwrap(),
        json!({"parts":[]})
    );
}

#[test]
fn idle_viewer_clock_runs_open_water_and_respects_explicit_pause() {
    let mut world = fixture();
    world.insert_resource(animation::AnimationClock::default());
    world.init_resource::<ve_render::WindField>();
    let mut real = bevy_time::Time::<bevy_time::Real>::default();
    real.update_with_duration(std::time::Duration::ZERO);
    world.insert_resource(real);
    world
        .query::<&mut VeMaterial>()
        .single_mut(&mut world)
        .unwrap()
        .recipe = ve_render::ProceduralRecipe::None;
    // A poke immediately after loading, before any sim update or status query.
    let poke = json!({"x":5,"z":5,"radius":1.2,"amount":0.12,"splash":false});
    assert_eq!(
        water_sim::method(&mut world, "water.disturb", &poke).unwrap()["disturbed"],
        1
    );
    let mut schedule = Schedule::default();
    schedule.set_executor(bevy_ecs::schedule::SingleThreadedExecutor::new());
    schedule.add_systems(
        (
            animation::apply_requested,
            animation::advance,
            water_sim::update,
        )
            .chain(),
    );
    for _ in 0..60 {
        world
            .resource_mut::<bevy_time::Time<bevy_time::Real>>()
            .update_with_duration(std::time::Duration::from_secs_f64(1.0 / 60.0));
        schedule.run(&mut world);
    }
    let time = world.resource::<animation::AnimationClock>().seconds;
    assert!((time - 1.0).abs() < 0.001);
    assert_eq!(
        world
            .query::<&VeMaterial>()
            .single(&world)
            .unwrap()
            .water_time,
        time
    );
    let status = water_sim::method(&mut world, "water.status", &json!({})).unwrap();
    assert!(status["parts"][0]["steps"].as_u64().unwrap() >= 59);
    animation::control(false, -1.0);
    schedule.run(&mut world);
    assert_eq!(world.resource::<animation::AnimationClock>().seconds, time);
    assert_eq!(
        water_sim::method(&mut world, "water.status", &json!({})).unwrap(),
        status
    );
    animation::set_speed(2.0);
    animation::control(true, -1.0);
    schedule.run(&mut world);
    assert!(world.resource::<animation::AnimationClock>().seconds > time + 0.03);
}
