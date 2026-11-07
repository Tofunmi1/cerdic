//! The XPBD solver behind cloth, rope, hair and soft bodies: what hangs, drapes, slides, inflates and blows, and that it does so the same
//! way every time. Written before the body; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_world::cloth::{Collider, SdfGrid, Solver, V3, balloon, cloth_from_mesh, rope};

/// A square sheet of `n` by `n` points, `spacing` apart, in the XZ plane at height `y`, centred on the origin, as triangles.
fn sheet(n: usize, spacing: f32, y: f32) -> (Vec<V3>, Vec<[u32; 3]>) {
    let half = (n - 1) as f32 * spacing * 0.5;
    let mut points = Vec::new();
    for k in 0..n {
        for i in 0..n {
            points.push([i as f32 * spacing - half, y, k as f32 * spacing - half]);
        }
    }
    let mut triangles = Vec::new();
    for k in 0..n - 1 {
        for i in 0..n - 1 {
            let (a, b, c, d) = (
                (k * n + i) as u32,
                (k * n + i + 1) as u32,
                ((k + 1) * n + i + 1) as u32,
                ((k + 1) * n + i) as u32,
            );
            triangles.extend([[a, b, c], [a, c, d]]);
        }
    }
    (points, triangles)
}

fn run(solver: &mut Solver, seconds: f32) {
    for _ in 0..(seconds * 60.0) as usize {
        assert!(solver.step(1.0 / 60.0), "the simulation stayed finite");
    }
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

fn length(a: V3, b: V3) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn a_rope_pinned_at_one_end_hangs_straight_down_at_its_own_length() {
    let points: Vec<V3> = (0..21).map(|k| [k as f32 * 0.05, 2.0, 0.0]).collect();
    let mut solver = rope(&points, 0.1, 0.0, 0.01).unwrap();
    solver.pin(0);
    run(&mut solver, 4.0);
    let end = solver.pos[20];
    assert!(
        near(end[0], 0.0, 0.05),
        "it swung down under its pin: x = {}",
        end[0]
    );
    assert!(
        near(end[1], 2.0 - 1.0, 0.03),
        "one metre of rope hangs one metre down: y = {}",
        end[1]
    );
    assert!(
        solver.max_stretch() < 1.02,
        "a rigid rope hardly stretches under its own weight: {}",
        solver.max_stretch()
    );
}

#[test]
fn a_soft_rope_stretches_more_than_a_stiff_one() {
    let points: Vec<V3> = (0..11).map(|k| [k as f32 * 0.1, 2.0, 0.0]).collect();
    let stretch = |compliance: f32| {
        let mut solver = rope(&points, 0.5, compliance, 0.01).unwrap();
        solver.pin(0);
        run(&mut solver, 3.0);
        solver.max_stretch()
    };
    assert!(
        stretch(1e-3) > stretch(0.0) + 0.02,
        "compliance makes the rope give"
    );
}

#[test]
fn cloth_pinned_along_one_edge_hangs_without_stretching() {
    let (points, triangles) = sheet(12, 0.1, 2.0);
    let mut solver = cloth_from_mesh(&points, &triangles, 0.2, 0.0, 0.05).unwrap();
    for i in 0..12 {
        solver.pin(i);
    }
    // A sheet swung from flat takes a few swings to settle like a pendulum.
    run(&mut solver, 9.0);
    let lowest = solver
        .pos
        .iter()
        .map(|p| p[1])
        .fold(f32::INFINITY, f32::min);
    assert!(
        lowest < 2.0 - 0.95,
        "the sheet hangs down about its height of 1.1 m: lowest {lowest}"
    );
    assert!(
        solver.max_stretch() < 1.06,
        "and stays near its own size: {}",
        solver.max_stretch()
    );
}

#[test]
fn cloth_dropped_on_a_sphere_wraps_it_and_never_passes_through() {
    let (points, triangles) = sheet(24, 0.05, 1.2);
    let mut solver = cloth_from_mesh(&points, &triangles, 0.2, 0.0, 0.02).unwrap();
    solver.colliders.push(Collider::Sphere {
        center: [0.0, 0.8, 0.0],
        radius: 0.35,
    });
    run(&mut solver, 3.0);
    let thickness = solver.settings.thickness;
    let inside = solver
        .pos
        .iter()
        .filter(|p| length(**p, [0.0, 0.8, 0.0]) < 0.35 + thickness - 0.004)
        .count();
    assert_eq!(inside, 0, "no point is inside the sphere");
    let centre = solver.pos[12 * 24 + 12];
    let edge = solver.pos[0];
    assert!(
        centre[1] > 0.8 + 0.3,
        "the middle rests on top: {}",
        centre[1]
    );
    assert!(
        edge[1] < centre[1] - 0.2,
        "the corners hang lower: {} against {}",
        edge[1],
        centre[1]
    );
}

#[test]
fn a_capsule_holds_cloth_like_a_sphere_does() {
    let (points, triangles) = sheet(20, 0.06, 1.0);
    let mut solver = cloth_from_mesh(&points, &triangles, 0.2, 0.0, 0.02).unwrap();
    solver.colliders.push(Collider::Capsule {
        a: [-0.4, 0.6, 0.0],
        b: [0.4, 0.6, 0.0],
        radius: 0.12,
    });
    run(&mut solver, 3.0);
    let thickness = solver.settings.thickness;
    for p in &solver.pos {
        let (d, _) = solver.colliders[0].distance(*p);
        assert!(d >= thickness - 0.004, "outside the capsule: {d}");
    }
    let top = solver.pos[10 * 20 + 10][1];
    assert!(top > 0.6 + 0.1, "it rests over the bar: {top}");
}

#[test]
fn a_floor_stops_cloth_and_friction_stops_it_sliding_down_a_slope() {
    let slope = |friction: f32| {
        let (points, triangles) = sheet(10, 0.06, 0.2);
        let mut solver = cloth_from_mesh(&points, &triangles, 0.2, 0.0, 0.02).unwrap();
        solver.settings.friction = friction;
        // A plane tilted 25 degrees about the z axis (rising toward +x).
        let n = [-0.4226, 0.9063, 0.0];
        solver.colliders.push(Collider::Plane {
            point: [0.0, 0.0, 0.0],
            normal: n,
        });
        run(&mut solver, 3.0);
        let mean_x = solver.pos.iter().map(|p| p[0]).sum::<f32>() / solver.pos.len() as f32;
        (
            mean_x,
            solver
                .pos
                .iter()
                .map(|p| p[0] * n[0] + p[1] * n[1] + p[2] * n[2])
                .fold(f32::INFINITY, f32::min),
        )
    };
    let (slid, lowest) = slope(0.0);
    let (held, _) = slope(0.9);
    assert!(
        lowest >= 0.005 - 0.003,
        "nothing goes through the floor: {lowest}"
    );
    assert!(
        slid < held - 0.05,
        "without friction it slides further down the slope (smaller x) than with it: {slid} against {held}"
    );
}

fn icosphere(levels: u32, radius: f32) -> (Vec<V3>, Vec<[u32; 3]>) {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let raw = [
        [-1.0, t, 0.0],
        [1.0, t, 0.0],
        [-1.0, -t, 0.0],
        [1.0, -t, 0.0],
        [0.0, -1.0, t],
        [0.0, 1.0, t],
        [0.0, -1.0, -t],
        [0.0, 1.0, -t],
        [t, 0.0, -1.0],
        [t, 0.0, 1.0],
        [-t, 0.0, -1.0],
        [-t, 0.0, 1.0],
    ];
    let norm = |v: V3| {
        let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        [v[0] / l * radius, v[1] / l * radius, v[2] / l * radius]
    };
    let mut vertices: Vec<V3> = raw.iter().map(|v| norm(*v)).collect();
    let mut faces: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..levels {
        let mut cache = std::collections::BTreeMap::new();
        let mut next = Vec::new();
        let mut mid = |a: u32, b: u32, vertices: &mut Vec<V3>| {
            *cache.entry((a.min(b), a.max(b))).or_insert_with(|| {
                let (p, q) = (vertices[a as usize], vertices[b as usize]);
                vertices.push(norm([
                    (p[0] + q[0]) / 2.0,
                    (p[1] + q[1]) / 2.0,
                    (p[2] + q[2]) / 2.0,
                ]));
                vertices.len() as u32 - 1
            })
        };
        for f in &faces {
            let (a, b, c) = (
                mid(f[0], f[1], &mut vertices),
                mid(f[1], f[2], &mut vertices),
                mid(f[2], f[0], &mut vertices),
            );
            next.extend([[f[0], a, c], [f[1], b, a], [f[2], c, b], [a, b, c]]);
        }
        faces = next;
    }
    (vertices, faces)
}

#[test]
fn a_balloon_at_pressure_one_keeps_its_volume_and_more_pressure_inflates_it() {
    let (points, triangles) = icosphere(2, 0.5);
    let measure = |pressure: f32| {
        let mut solver = balloon(&points, &triangles, 0.5, pressure, 0.0, 0.0).unwrap();
        solver.settings.gravity = [0.0; 3];
        let before = solver.volume_of(&triangles);
        run(&mut solver, 1.5);
        solver.volume_of(&triangles) / before
    };
    assert!(near(measure(1.0), 1.0, 0.02), "{}", measure(1.0));
    assert!(
        measure(1.4) > 1.25,
        "pressure 1.4 inflates it: {}",
        measure(1.4)
    );
}

#[test]
fn a_balloon_dropped_on_a_floor_squashes_but_keeps_its_volume() {
    let (points, triangles) = icosphere(2, 0.4);
    let lifted: Vec<V3> = points.iter().map(|p| [p[0], p[1] + 1.0, p[2]]).collect();
    let mut solver = balloon(&lifted, &triangles, 0.5, 1.0, 1e-5, 1e-6).unwrap();
    solver.colliders.push(Collider::Plane {
        point: [0.0, 0.0, 0.0],
        normal: [0.0, 1.0, 0.0],
    });
    let before = solver.volume_of(&triangles);
    run(&mut solver, 2.5);
    let after = solver.volume_of(&triangles);
    assert!(
        near(after / before, 1.0, 0.08),
        "volume holds: {}",
        after / before
    );
    let lowest = solver
        .pos
        .iter()
        .map(|p| p[1])
        .fold(f32::INFINITY, f32::min);
    let height = solver
        .pos
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max)
        - lowest;
    assert!(
        lowest >= 0.005 - 0.003,
        "it rests on the floor, not in it: {lowest}"
    );
    assert!(
        near(height, 0.8, 0.04),
        "and stays a ball (a stiff one barely squashes): {height}"
    );
}

#[test]
fn wind_pushes_hanging_cloth_downwind_and_no_drag_means_no_push() {
    let hang = |drag: f32| {
        let (points, triangles) = sheet(10, 0.08, 2.0);
        let mut solver = cloth_from_mesh(&points, &triangles, 0.2, 0.0, 0.05).unwrap();
        for i in 0..10 {
            solver.pin(i);
        }
        solver.settings.wind = [6.0, 0.0, 0.0];
        solver.settings.drag = drag;
        run(&mut solver, 2.0);
        solver.pos.iter().map(|p| p[0]).sum::<f32>() / solver.pos.len() as f32
    };
    assert!(
        hang(1.0) > hang(0.0) + 0.05,
        "wind blows it along +x: {} against {}",
        hang(1.0),
        hang(0.0)
    );
    assert!(
        near(hang(0.0), 0.0, 0.01),
        "with no drag the wind does nothing: {}",
        hang(0.0)
    );
}

#[test]
fn a_signed_distance_grid_collides_like_the_shape_it_samples() {
    // A sphere of radius 0.35 as a grid of distances.
    let (n, cell) = (40_usize, 0.05_f32);
    let origin = [-1.0, -0.2, -1.0];
    let mut values = Vec::new();
    for k in 0..n {
        for j in 0..n {
            for i in 0..n {
                let p = [
                    origin[0] + i as f32 * cell,
                    origin[1] + j as f32 * cell,
                    origin[2] + k as f32 * cell,
                ];
                values.push(length(p, [0.0, 0.8, 0.0]) - 0.35);
            }
        }
    }
    let grid = SdfGrid {
        origin,
        cell,
        dims: [n, n, n],
        values,
    };
    let drop = |collider: Collider| {
        let (points, triangles) = sheet(20, 0.06, 1.3);
        let mut solver = cloth_from_mesh(&points, &triangles, 0.2, 0.0, 0.02).unwrap();
        solver.colliders.push(collider);
        run(&mut solver, 2.5);
        solver.pos[10 * 20 + 10][1]
    };
    let analytic = drop(Collider::Sphere {
        center: [0.0, 0.8, 0.0],
        radius: 0.35,
    });
    let gridded = drop(Collider::Sdf(grid));
    assert!(
        near(analytic, gridded, 0.02),
        "the grid and the sphere agree on where the cloth rests: {analytic} and {gridded}"
    );
}

#[test]
fn extending_a_clamped_distance_grid_gives_far_and_deep_points_a_true_distance_and_a_way_out() {
    let (n, cell, trunc) = (48_usize, 0.05_f32, 0.1_f32);
    let origin = [-1.2, -1.2, -1.2];
    let centre = [0.0, 0.0, 0.0];
    let mut values = Vec::new();
    for k in 0..n {
        for j in 0..n {
            for i in 0..n {
                let p = [
                    origin[0] + i as f32 * cell,
                    origin[1] + j as f32 * cell,
                    origin[2] + k as f32 * cell,
                ];
                values.push((length(p, centre) - 0.6).clamp(-trunc, trunc));
            }
        }
    }
    let mut grid = SdfGrid {
        origin,
        cell,
        dims: [n, n, n],
        values,
    };
    let clamped = Collider::Sdf(grid.clone());
    // Deep inside, the clamped grid is flat: no direction to push along (it answers "up" for everything).
    let (_, flat_normal) = clamped.distance([0.3, 0.0, 0.0]);
    assert!(
        near(flat_normal[1], 1.0, 1e-4),
        "no real direction in the clamped region: {flat_normal:?}"
    );
    grid.fill_far(trunc);
    let extended = Collider::Sdf(grid);
    // Far outside and deep inside now read the true distance, within a few percent of the cell size, and point radially.
    let (outside, out_normal) = extended.distance([0.0, 0.0, 1.0]);
    assert!(
        near(outside, 0.4, 0.04),
        "1 m from the centre is 0.4 m outside the 0.6 m sphere: {outside}"
    );
    assert!(
        out_normal[2] > 0.9,
        "and the way out is straight away from the centre: {out_normal:?}"
    );
    let (inside, in_normal) = extended.distance([0.3, 0.0, 0.0]);
    assert!(
        near(inside, -0.3, 0.04),
        "0.3 m from the centre is 0.3 m inside: {inside}"
    );
    assert!(
        in_normal[0] > 0.9,
        "and out is along +x, toward the surface: {in_normal:?}"
    );
}

#[test]
fn the_same_simulation_twice_gives_the_same_bits() {
    let once = || {
        let (points, triangles) = sheet(14, 0.06, 1.0);
        let mut solver = cloth_from_mesh(&points, &triangles, 0.2, 1e-6, 0.02).unwrap();
        solver.colliders.push(Collider::Sphere {
            center: [0.0, 0.5, 0.0],
            radius: 0.25,
        });
        solver.settings.wind = [2.0, 0.0, 1.0];
        solver.settings.drag = 0.5;
        run(&mut solver, 1.5);
        solver
            .pos
            .iter()
            .flatten()
            .map(|v| v.to_bits())
            .collect::<Vec<u32>>()
    };
    assert_eq!(once(), once());
}

#[test]
fn a_moved_pin_drags_the_rope_after_it() {
    let points: Vec<V3> = (0..11).map(|k| [0.0, 2.0 - k as f32 * 0.1, 0.0]).collect();
    let mut solver = rope(&points, 0.1, 0.0, 0.01).unwrap();
    solver.pin(0);
    for frame in 0..120 {
        solver.move_pin(0, [frame as f32 * 0.01, 2.0, 0.0]);
        assert!(solver.step(1.0 / 60.0));
    }
    let end = solver.pos[10];
    assert!(
        end[0] > 0.6,
        "the end followed the root along +x: {}",
        end[0]
    );
    assert!(
        end[0] < solver.pos[0][0],
        "and trails behind it: {} against {}",
        end[0],
        solver.pos[0][0]
    );
}

#[test]
fn bad_input_gives_no_solver_instead_of_a_crash() {
    assert!(
        cloth_from_mesh(&[[0.0; 3]; 3], &[[0, 1, 9]], 0.2, 0.0, 0.0).is_none(),
        "a triangle naming a missing point"
    );
    assert!(
        rope(&[[0.0; 3]], 0.1, 0.0, 0.0).is_none(),
        "a rope needs two points"
    );
}
