//! Position-based fluids: a dam break settles in its box, keeps its volume and rest density, flows around colliders, and is deterministic.
#![expect(clippy::unwrap_used, reason = "fixtures")]
use ve_world::cloth::Collider;
use ve_world::fluid::Fluid;

const BOX: ([f32; 3], [f32; 3]) = ([0.0, 0.0, 0.0], [1.0, 1.0, 0.4]);

fn tank() -> Fluid {
    // A column of water 0.4 x 0.5 x 0.4 in a 1 x 1 x 0.4 tank, particles 0.05 apart.
    let mut fluid = Fluid::block([0.0, 0.0, 0.0], [0.4, 0.5, 0.4], 0.05).unwrap();
    fluid.settings.bounds = Some(BOX);
    fluid
}

fn run(fluid: &mut Fluid, seconds: f32) {
    for _ in 0..(seconds * 60.0) as usize {
        assert!(fluid.step(1.0 / 60.0), "the solver stayed finite");
    }
}

#[test]
fn a_dam_break_spreads_out_and_settles_inside_the_tank() {
    let mut fluid = tank();
    let before = fluid.pos.len();
    run(&mut fluid, 2.5);
    assert_eq!(fluid.pos.len(), before);
    for p in &fluid.pos {
        assert!(
            p[0] > -0.001
                && p[0] < 1.001
                && p[1] > -0.001
                && p[1] < 1.001
                && p[2] > -0.001
                && p[2] < 0.401,
            "escaped the tank: {p:?}"
        );
    }
    let speed = fluid
        .vel
        .iter()
        .map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
        .fold(0.0, f32::max);
    assert!(
        speed < 1.5,
        "it has come mostly to rest: fastest particle {speed} m/s"
    );
    // It flowed along the floor: the column was 0.4 wide, now it covers most of the 1 m floor.
    let reach = fluid.pos.iter().map(|p| p[0]).fold(0.0, f32::max);
    assert!(reach > 0.8, "the water spread across the tank: {reach}");
}

#[test]
fn the_settled_water_keeps_its_volume() {
    let mut fluid = tank();
    run(&mut fluid, 3.0);
    // 0.4 x 0.5 x 0.4 = 0.08 m3 spread over a 1 x 0.4 floor is 0.2 m deep. Surface particles sit half a spacing under the surface.
    let top = fluid.pos.iter().map(|p| p[1]).fold(0.0, f32::max);
    assert!(
        top > 0.17 && top < 0.30,
        "the surface settles near 0.2 m: {top}"
    );
}

#[test]
fn deep_water_stays_near_rest_density() {
    let mut fluid = tank();
    run(&mut fluid, 3.0);
    let mean = (0..fluid.pos.len())
        .map(|i| fluid.density_ratio(i))
        .sum::<f32>()
        / fluid.pos.len() as f32;
    assert!(
        mean > 0.75 && mean < 1.25,
        "mean density against rest: {mean}"
    );
}

#[test]
fn water_is_pushed_around_a_sphere_and_never_enters_it() {
    let mut fluid = Fluid::block([0.35, 0.6, 0.05], [0.65, 0.9, 0.35], 0.05).unwrap();
    fluid.settings.bounds = Some(BOX);
    fluid.colliders.push(Collider::Sphere {
        center: [0.5, 0.4, 0.2],
        radius: 0.15,
    });
    run(&mut fluid, 1.5);
    let deepest = fluid
        .pos
        .iter()
        .map(|p| {
            let d = [p[0] - 0.5, p[1] - 0.4, p[2] - 0.2];
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() - 0.15
        })
        .fold(f32::MAX, f32::min);
    assert!(
        deepest > -0.02,
        "no particle sinks into the sphere: {deepest}"
    );
}

#[test]
fn the_same_fluid_falls_the_same_way_every_time() {
    let (mut a, mut b) = (tank(), tank());
    run(&mut a, 0.5);
    run(&mut b, 0.5);
    assert_eq!(a.pos, b.pos);
    assert_eq!(a.vel, b.vel);
}

#[test]
fn the_surface_of_a_block_is_a_closed_outward_mesh_just_outside_the_particles() {
    let fluid = Fluid::block([0.0, 0.0, 0.0], [0.4, 0.5, 0.4], 0.05).unwrap();
    let s = ve_world::fluid::surface(&fluid.pos, 0.05, 0.035, 0.4).unwrap();
    assert!(s.triangles.len() > 400, "{} triangles", s.triangles.len());
    // Closed: every edge is shared by exactly two triangles.
    let mut edges = std::collections::BTreeMap::new();
    for t in &s.triangles {
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    assert!(
        edges.values().all(|&n| n == 2),
        "{} open edges",
        edges.values().filter(|&&n| n != 2).count()
    );
    // Outward: the signed volume of a closed outward mesh is positive, and about the block's volume (0.08) plus a skin of half a spacing.
    let volume: f32 = s
        .triangles
        .iter()
        .map(|t| {
            let (a, b, c) = (
                s.positions[t[0] as usize],
                s.positions[t[1] as usize],
                s.positions[t[2] as usize],
            );
            (a[0] * (b[1] * c[2] - b[2] * c[1])
                + a[1] * (b[2] * c[0] - b[0] * c[2])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0
        })
        .sum();
    assert!(volume > 0.07 && volume < 0.13, "enclosed volume {volume}");
    // Normals are unit and point away from the middle for the corner vertices.
    assert!(
        s.normals
            .iter()
            .all(|n| ((n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() - 1.0).abs() < 1e-3)
    );
    let top = s
        .positions
        .iter()
        .enumerate()
        .max_by(|a, b| a.1[1].total_cmp(&b.1[1]))
        .unwrap()
        .0;
    assert!(
        s.normals[top][1] > 0.5,
        "the top faces up: {:?}",
        s.normals[top]
    );
}

#[test]
fn no_points_or_a_huge_grid_give_no_surface() {
    assert!(ve_world::fluid::surface(&[], 0.05, 0.03, 0.4).is_none());
    assert!(
        ve_world::fluid::surface(&[[0.0; 3], [100.0; 3]], 0.05, 0.01, 0.4).is_none(),
        "a trillion grid points"
    );
}

#[test]
fn bad_blocks_and_too_many_particles_are_refused() {
    assert!(Fluid::block([0.0; 3], [1.0, 1.0, 1.0], 0.0).is_none());
    assert!(Fluid::block([0.0; 3], [0.0, 1.0, 1.0], 0.05).is_none());
    assert!(
        Fluid::block([0.0; 3], [10.0, 10.0, 10.0], 0.05).is_none(),
        "8 million particles"
    );
    assert!(Fluid::block([0.0; 3], [f32::NAN, 1.0, 1.0], 0.05).is_none());
}

#[test]
fn thicker_liquid_flows_slower() {
    let mut water = tank();
    let mut honey = tank();
    honey.settings.viscosity = 0.5;
    // Early on, before either has hit the far wall: the front of the honey is behind the front of the water.
    run(&mut water, 0.3);
    run(&mut honey, 0.3);
    let reach = |f: &Fluid| f.pos.iter().map(|p| p[0]).fold(0.0, f32::max);
    assert!(
        reach(&honey) < reach(&water) - 0.01,
        "honey {} against water {}",
        reach(&honey),
        reach(&water)
    );
}
