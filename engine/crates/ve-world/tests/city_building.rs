//! One building of a modern district from its lot: a shop floor, a shaft with setback tiers when tall, a flat roof with rooftop clutter (air-conditioning units, a water tank, a stair core, an antenna).
//! Two meshes: the walls (UVs in facade units: u every 8 m along a wall, v every 3 floors up) and the roof with its clutter (UVs in metres / 8). Written before the body; do not weaken.
use ve_world::building::{BuildingError, BuildingParams, Mesh, Roof, build};

fn square(half: f32) -> Vec<[f32; 2]> {
    vec![[-half, -half], [half, -half], [half, half], [-half, half]]
}

fn params(footprint: Vec<[f32; 2]>, height: f32) -> BuildingParams {
    BuildingParams {
        footprint,
        height,
        front: 0,
        seed: 3,
        floor: 3.6,
        clutter: 1.0,
        detail: 0.0,
        roof: Roof::Flat,
    }
}

fn top(mesh: &Mesh) -> f32 {
    mesh.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max)
}

fn sane(mesh: &Mesh) {
    assert!(!mesh.indices.is_empty() && mesh.indices.len() % 3 == 0);
    assert!(
        mesh.indices
            .iter()
            .all(|i| (*i as usize) < mesh.positions.len()),
        "indices point at vertices"
    );
    assert_eq!(
        mesh.uvs.len(),
        mesh.indices.len(),
        "one UV per triangle corner"
    );
    assert!(mesh.positions.iter().flatten().all(|c| c.is_finite()));
}

#[test]
fn a_low_building_is_a_box_to_its_height_with_its_roof_apart() {
    let b = build(&params(square(8.0), 12.0)).unwrap();
    sane(&b.walls);
    sane(&b.roof);
    let wall_top = top(&b.walls);
    assert!(
        (wall_top - 12.0).abs() < 0.05,
        "the walls reach the height: {wall_top}"
    );
    assert!(
        b.walls.positions.iter().all(|p| p[1] >= -0.001),
        "nothing below the ground"
    );
    let roof_low = b
        .roof
        .positions
        .iter()
        .map(|p| p[1])
        .fold(f32::MAX, f32::min);
    assert!(
        roof_low >= 11.9,
        "the roof mesh sits on top of the walls: lowest {roof_low}"
    );
    assert!(
        top(&b.roof) <= 12.0 + 7.0,
        "clutter is at most a few metres tall: {}",
        top(&b.roof)
    );
}

#[test]
fn walls_never_leave_the_footprint_by_more_than_a_ledge() {
    let b = build(&params(
        vec![[-9.0, -6.0], [10.0, -7.0], [11.0, 8.0], [-8.0, 9.0]],
        30.0,
    ))
    .unwrap();
    for p in b.walls.positions.iter().chain(&b.roof.positions) {
        assert!(
            p[0] >= -9.4 && p[0] <= 11.4 && p[2] >= -7.4 && p[2] <= 9.4,
            "{p:?} is outside the lot"
        );
    }
}

#[test]
fn facade_uvs_count_8_metre_bays_and_3_floor_repeats() {
    let b = build(&params(square(8.0), 36.0)).unwrap();
    let (mut u_hi, mut v_hi) = (0.0_f32, 0.0_f32);
    for uv in &b.walls.uvs {
        u_hi = u_hi.max(uv[0]);
        v_hi = v_hi.max(uv[1]);
    }
    // a 16 m wall is two bays; a 36 m wall of 3.6 m floors is ten floors, 3 floors to a repeat: 3.33 repeats
    assert!(
        (u_hi - 2.0).abs() < 0.05 || u_hi >= 2.0 && u_hi <= 8.1,
        "u runs to {u_hi}"
    );
    assert!((v_hi - 36.0 / 10.8).abs() < 0.1, "v runs to {v_hi}");
}

#[test]
fn tall_buildings_step_in_and_low_ones_do_not() {
    let tall = build(&params(square(10.0), 110.0)).unwrap();
    let upper_reach = tall
        .walls
        .positions
        .iter()
        .filter(|p| p[1] > 75.0)
        .map(|p| p[0].abs().max(p[2].abs()))
        .fold(0.0_f32, f32::max);
    assert!(
        upper_reach < 10.0 - 1.2,
        "the upper tier is set back: reaches {upper_reach} of 10"
    );
    let low = build(&params(square(10.0), 20.0)).unwrap();
    let low_reach = low
        .walls
        .positions
        .iter()
        .filter(|p| p[1] > 8.0)
        .map(|p| p[0].abs().max(p[2].abs()))
        .fold(0.0_f32, f32::max);
    assert!(
        low_reach > 9.5,
        "a low building is a straight shaft: {low_reach}"
    );
}

#[test]
fn clutter_is_optional_and_stays_on_the_roof() {
    let bare = build(&BuildingParams {
        clutter: 0.0,
        ..params(square(9.0), 25.0)
    })
    .unwrap();
    let full = build(&params(square(9.0), 25.0)).unwrap();
    assert!(
        full.roof.indices.len() > bare.roof.indices.len() + 36,
        "clutter adds triangles: {} against {}",
        full.roof.indices.len(),
        bare.roof.indices.len()
    );
    assert!(top(&bare.roof) <= 25.0 + 0.05, "no clutter: a flat roof");
    for p in &full.roof.positions {
        assert!(
            p[0].abs() <= 9.01 && p[2].abs() <= 9.01,
            "clutter stays over the building: {p:?}"
        );
    }
}

#[test]
fn a_building_is_a_few_hundred_triangles_at_most() {
    for (half, height) in [(5.0, 9.0), (8.0, 30.0), (12.0, 150.0)] {
        let b = build(&params(square(half), height)).unwrap();
        let triangles = (b.walls.indices.len() + b.roof.indices.len()) / 3;
        assert!(
            (12..=420).contains(&triangles),
            "{triangles} triangles for {half} m x {height} m"
        );
    }
}

#[test]
fn the_same_seed_builds_the_same_building_and_another_seed_a_different_roof() {
    let a = build(&params(square(9.0), 25.0)).unwrap();
    let b = build(&params(square(9.0), 25.0)).unwrap();
    let c = build(&BuildingParams {
        seed: 99,
        ..params(square(9.0), 25.0)
    })
    .unwrap();
    assert_eq!(a, b);
    assert_ne!(
        a.roof.positions, c.roof.positions,
        "the clutter is placed differently"
    );
}

#[test]
fn bad_buildings_are_refused_with_the_name() {
    for (p, name) in [
        (params(vec![[0.0, 0.0], [1.0, 0.0]], 10.0), "footprint"),
        (
            params(
                vec![[-5.0, -5.0], [-5.0, 5.0], [5.0, 5.0], [5.0, -5.0]],
                10.0,
            ),
            "footprint",
        ), // clockwise
        (params(square(5.0), 1.0), "height"),
        (params(square(5.0), 900.0), "height"),
        (
            BuildingParams {
                floor: 0.5,
                ..params(square(5.0), 10.0)
            },
            "floor",
        ),
        (
            BuildingParams {
                clutter: 2.0,
                ..params(square(5.0), 10.0)
            },
            "clutter",
        ),
    ] {
        let error: BuildingError = build(&p).expect_err(name);
        assert!(error.to_string().contains(name), "{name}: {error}");
    }
}
