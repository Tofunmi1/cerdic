//! Building shape for the district generator: a mansard roof with dormers instead of a flat one, and lots split into wings (an L, or a U round a courtyard)
//! instead of one box. Both are opt-in (`Roof::Flat` and one wing are what a building was before), so existing districts do not change. Written before the
//! code; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_world::building::{BuildingParams, Mesh, Roof, build, wings};

fn square(half: f32) -> Vec<[f32; 2]> {
    vec![[-half, -half], [half, -half], [half, half], [-half, half]]
}

fn params(height: f32, roof: Roof) -> BuildingParams {
    BuildingParams {
        footprint: square(10.0),
        height,
        front: 0,
        seed: 3,
        floor: 3.6,
        clutter: 0.0,
        detail: 0.0,
        roof,
    }
}

fn range(mesh: &Mesh, axis: usize) -> (f32, f32) {
    mesh.positions
        .iter()
        .map(|p| p[axis])
        .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)))
}

fn area(polygon: &[[f32; 2]]) -> f32 {
    (0..polygon.len())
        .map(|i| {
            let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f32>()
        * 0.5
}

/// Upward-facing triangle normals' y component, one per triangle.
fn normals_y(mesh: &Mesh) -> Vec<f32> {
    mesh.indices
        .chunks_exact(3)
        .map(|t| {
            let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.positions[i as usize]);
            let (u, v) = (
                [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
                [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
            );
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            n[1] / (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9)
        })
        .collect()
}

#[test]
fn a_flat_roof_is_what_a_building_was() {
    let b = build(&params(24.0, Roof::Flat)).unwrap();
    assert!(
        (range(&b.walls, 1).1 - 24.0).abs() < 0.05,
        "the walls reach the height"
    );
    assert!(
        normals_y(&b.roof).iter().all(|n| *n > 0.99),
        "every roof triangle is flat"
    );
}

#[test]
fn a_mansard_roof_ends_the_walls_one_storey_below_the_ridge() {
    let b = build(&params(24.0, Roof::Mansard)).unwrap();
    let wall_top = 24.0 - 3.6;
    let (_, roof_top) = range(&b.roof, 1);
    assert!(
        (roof_top - 24.0).abs() < 0.05,
        "the roof still reaches the height: {roof_top}"
    );
    let steep = normals_y(&b.roof)
        .iter()
        .filter(|n| (0.3..0.8).contains(*n))
        .count();
    assert!(steep >= 8, "the slope has triangles on every side: {steep}");
    let eave = b
        .roof
        .positions
        .iter()
        .filter(|p| (p[1] - wall_top).abs() < 0.05)
        .count();
    assert!(eave >= 4, "the slope starts where the walls end: {eave}");
}

#[test]
fn the_mansard_slopes_in_and_stays_inside_the_lot() {
    let b = build(&params(24.0, Roof::Mansard)).unwrap();
    let top: Vec<_> = b.roof.positions.iter().filter(|p| p[1] > 23.95).collect();
    assert!(!top.is_empty());
    assert!(
        top.iter().all(|p| p[0].abs() < 9.0 && p[2].abs() < 9.0),
        "the flat top is drawn in from the eaves"
    );
    for axis in [0, 2] {
        let (lo, hi) = range(&b.roof, axis);
        assert!(
            lo >= -10.0 - 0.01 && hi <= 10.0 + 0.01,
            "nothing past the lot"
        );
    }
}

#[test]
fn dormers_stand_on_the_slope_with_windows() {
    let b = build(&params(24.0, Roof::Mansard)).unwrap();
    let wall_top = 24.0 - 3.6;
    let (_, walls_top) = range(&b.walls, 1);
    assert!(
        walls_top > wall_top + 0.5,
        "dormer fronts rise above the eave: {walls_top}"
    );
    assert!(walls_top < 24.0, "and stay below the ridge: {walls_top}");
    let dormer_corners = b
        .walls
        .positions
        .iter()
        .filter(|p| p[1] > wall_top + 0.01)
        .count();
    assert!(
        dormer_corners >= 16,
        "at least a few dormers: {dormer_corners}"
    );
}

#[test]
fn a_low_building_keeps_a_flat_roof() {
    let b = build(&params(8.0, Roof::Mansard)).unwrap();
    assert!(
        (range(&b.walls, 1).1 - 8.0).abs() < 0.05,
        "no room for a mansard under three storeys"
    );
    assert!(normals_y(&b.roof).iter().all(|n| *n > 0.99));
}

#[test]
fn a_tower_keeps_its_stepped_crown_not_a_mansard() {
    let b = build(&params(120.0, Roof::Mansard)).unwrap();
    assert!(
        normals_y(&b.roof).iter().all(|n| *n > 0.99),
        "towers stay flat"
    );
}

#[test]
fn no_wings_keeps_the_lot() {
    let lot = square(15.0);
    let w = wings(&lot, 0, 5, 0.0);
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].footprint, lot);
    assert_eq!(w[0].front, 0);
    assert!((w[0].height_factor - 1.0).abs() < 1e-6);
}

#[test]
fn a_big_lot_splits_into_wings_round_a_courtyard() {
    let lot = square(15.0);
    let mut split = 0;
    for seed in 0..40 {
        let w = wings(&lot, 0, seed, 1.0);
        if w.len() > 1 {
            split += 1;
            let covered: f32 = w.iter().map(|x| area(&x.footprint)).sum();
            assert!(
                covered < area(&lot) - 20.0,
                "a courtyard is left open: {covered}"
            );
            assert!(
                covered > area(&lot) * 0.5,
                "most of the lot is still built: {covered}"
            );
            for wing in &w {
                assert!(area(&wing.footprint) > 0.0, "wound counter-clockwise");
                assert!(
                    wing.footprint
                        .iter()
                        .all(|c| c[0].abs() <= 15.001 && c[1].abs() <= 15.001),
                    "inside the lot"
                );
                assert!(wing.front < wing.footprint.len());
                assert!(wing.height_factor > 0.3 && wing.height_factor <= 1.0);
            }
            assert!(
                w.iter().any(|x| (x.height_factor - 1.0).abs() < 1e-6),
                "the street wing keeps the full height"
            );
        }
    }
    assert!(
        split >= 38,
        "nearly every seed splits a big lot at share 1: {split}"
    );
}

#[test]
fn wings_are_deterministic() {
    let lot = square(15.0);
    assert_eq!(wings(&lot, 0, 9, 1.0), wings(&lot, 0, 9, 1.0));
}

#[test]
fn a_small_or_skewed_lot_stays_whole() {
    assert_eq!(
        wings(&square(5.0), 0, 1, 1.0).len(),
        1,
        "too small for a courtyard"
    );
    let skew = vec![[-15.0, -15.0], [15.0, -15.0], [22.0, 15.0], [-8.0, 15.0]];
    assert_eq!(wings(&skew, 0, 1, 1.0).len(), 1, "not a rectangle");
}

#[test]
fn every_wing_builds() {
    let lot = square(15.0);
    for seed in 0..10 {
        for wing in wings(&lot, 0, seed, 1.0) {
            let mut p = params(20.0 * wing.height_factor, Roof::Mansard);
            p.footprint = wing.footprint;
            p.front = wing.front;
            build(&p).unwrap();
        }
    }
}
