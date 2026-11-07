//! Street detail on district buildings (the New York look): fire escapes on the street front of mid-rise buildings, shop awnings, a cornice on the roof
//! edge, a stoop on low houses, and stepped setbacks that end in a spire on towers. All of it is behind `detail` (0 to 1): 0 builds exactly what a building was
//! before, so existing districts do not change. Written before the code; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_world::building::{BuildingError, BuildingParams, Mesh, Roof, build};

/// A square lot of half-width `half`; edge 0 runs from (-half, -half) to (half, -half), so the street front faces -z.
fn square(half: f32) -> Vec<[f32; 2]> {
    vec![[-half, -half], [half, -half], [half, half], [-half, half]]
}

fn params(height: f32, detail: f32) -> BuildingParams {
    BuildingParams {
        footprint: square(10.0),
        height,
        front: 0,
        seed: 3,
        floor: 3.6,
        clutter: 1.0,
        detail,
        roof: Roof::Flat,
    }
}

fn range(mesh: &Mesh, axis: usize) -> (f32, f32) {
    mesh.positions
        .iter()
        .map(|p| p[axis])
        .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
}

fn sane(mesh: &Mesh) {
    assert!(
        !mesh.indices.is_empty() && mesh.indices.len() % 3 == 0,
        "has triangles"
    );
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
    assert!(
        mesh.positions.iter().flatten().all(|c| c.is_finite()),
        "finite"
    );
}

#[test]
fn no_detail_adds_nothing() {
    let b = build(&params(30.0, 0.0)).unwrap();
    assert!(b.metal.indices.is_empty(), "no fire escapes");
    assert!(b.awning.indices.is_empty(), "no awnings");
    assert!(
        (range(&b.walls, 1).1 - 30.0).abs() < 0.05,
        "the walls still reach the height"
    );
    assert!(
        range(&b.walls, 0).1 <= 10.0 + 1e-3,
        "and there is no cornice"
    );
}

#[test]
fn fire_escapes_hang_on_the_street_front_of_a_mid_rise() {
    let b = build(&params(30.0, 1.0)).unwrap();
    sane(&b.metal);
    let (z_lo, z_hi) = range(&b.metal, 2);
    assert!(
        z_hi <= -10.0 + 0.2,
        "outside the facade (front is -z): {z_hi}"
    );
    assert!(
        z_lo >= -10.0 - 2.2,
        "no more than a couple of metres out: {z_lo}"
    );
    let (y_lo, y_hi) = range(&b.metal, 1);
    assert!(y_lo >= 3.0, "above the shop windows: {y_lo}");
    assert!(
        y_hi >= 20.0 && y_hi <= 30.0 + 0.1,
        "up the building, not past the roof: {y_hi}"
    );
    let (x_lo, x_hi) = range(&b.metal, 0);
    assert!(
        x_lo >= -10.0 && x_hi <= 10.0,
        "within the width of the building: {x_lo} {x_hi}"
    );
}

#[test]
fn towers_have_no_fire_escapes() {
    let b = build(&params(120.0, 1.0)).unwrap();
    assert!(b.metal.indices.is_empty(), "a tower is not a tenement");
}

#[test]
fn awnings_shade_the_shop_front() {
    let b = build(&params(30.0, 1.0)).unwrap();
    sane(&b.awning);
    let (z_lo, z_hi) = range(&b.awning, 2);
    assert!(
        z_hi <= -10.0 + 0.05 && z_lo >= -10.0 - 2.4,
        "sticks out of the front only: {z_lo} {z_hi}"
    );
    let (y_lo, y_hi) = range(&b.awning, 1);
    let shop = 3.6 * 1.35;
    assert!(
        y_lo >= 1.8 && y_hi <= shop + 0.6,
        "over the shop windows: {y_lo} {y_hi}"
    );
}

#[test]
fn a_tall_tower_steps_back_more_than_once_and_ends_in_a_spire() {
    let plain = build(&params(130.0, 0.0)).unwrap();
    let grand = build(&params(130.0, 1.0)).unwrap();
    let levels = |m: &Mesh| {
        let mut ys: Vec<i32> = m
            .positions
            .iter()
            .map(|p| (p[1] * 10.0).round() as i32)
            .collect();
        ys.sort_unstable();
        ys.dedup();
        ys.len()
    };
    assert!(
        levels(&grand.roof) > levels(&plain.roof) + 1,
        "more ledges: {} against {}",
        levels(&grand.roof),
        levels(&plain.roof)
    );
    assert!(
        range(&grand.roof, 1).1 >= 130.0 + 14.0,
        "a spire above the roof: {}",
        range(&grand.roof, 1).1
    );
    assert!(
        range(&grand.walls, 1).1 > range(&grand.walls, 1).0,
        "walls exist"
    );
    sane(&grand.walls);
    sane(&grand.roof);
}

#[test]
fn a_mid_rise_gets_a_cornice_that_overhangs_the_wall() {
    let b = build(&params(30.0, 1.0)).unwrap();
    let (_, x_hi) = range(&b.walls, 0);
    assert!(
        x_hi > 10.2 && x_hi <= 11.0,
        "the cornice overhangs a little: {x_hi}"
    );
}

#[test]
fn a_low_house_gets_a_stoop_at_its_front_door() {
    let plain = build(&params(10.0, 0.0)).unwrap();
    let grand = build(&params(10.0, 1.0)).unwrap();
    assert!(
        range(&plain.shop, 2).0 >= -10.0 - 1e-3,
        "no steps without detail"
    );
    let z_lo = range(&grand.shop, 2).0;
    assert!(
        z_lo <= -10.0 - 1.2 && z_lo >= -10.0 - 3.5,
        "steps lead out to the street: {z_lo}"
    );
}

#[test]
fn the_amount_of_detail_scales_what_is_added() {
    let count = |detail: f32| {
        let b = build(&params(30.0, detail)).unwrap();
        b.metal.indices.len() + b.awning.indices.len()
    };
    assert_eq!(count(0.0), 0);
    assert!(count(0.3) > 0, "a little detail adds a little");
    assert!(count(0.3) <= count(1.0), "and more adds more");
}

#[test]
fn building_with_detail_is_deterministic() {
    assert_eq!(
        build(&params(50.0, 0.8)).unwrap(),
        build(&params(50.0, 0.8)).unwrap()
    );
}

#[test]
fn detail_outside_zero_to_one_is_refused_by_name() {
    for bad in [-0.1, 1.5, f32::NAN] {
        match build(&params(30.0, bad)) {
            Err(BuildingError::Range { name, .. }) => assert_eq!(name, "detail"),
            other => panic!("detail {bad} was accepted: {other:?}"),
        }
    }
}
