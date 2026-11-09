//! Plant meshing, LOD chains and impostors (ADR 0056). Written before the bodies; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "fixtures unwrap valid data, name vector parts a, b, c, and compare exact wind weights of 0 and 1"
)]
use ve_world::flora::{Preset, Skeleton, Tuft, colonize, grow_tree, grow_tuft, preset};
use ve_world::lod::{
    LeafCards, LodError, Meshing, PlantMesh, Surface, build_lods, impostor_spec, mesh_plant,
    mesh_tuft,
};

const NEVER: &dyn Fn() -> bool = &|| false;

fn oak() -> Skeleton {
    match preset("oak", 8.0, 3).unwrap() {
        Preset::Tree(tree) => grow_tree(&tree, NEVER).unwrap(),
        Preset::Colonize(setup) => colonize(&setup, NEVER).unwrap(),
    }
}

fn settings() -> Meshing {
    Meshing {
        radial_segments: 8,
        max_segment_length: 0.6,
        bark_uv_scale: [0.5, 1.0],
        leaf_cards: LeafCards::Quads {
            aspect: 0.6,
            bend_deg: 20.0,
        },
        fork_blend: 0.5,
        smooth: 0,
        flare: 0.0,
    }
}

fn triangles(mesh: &PlantMesh, kind: Option<Surface>) -> usize {
    mesh.material_ranges
        .iter()
        .filter(|r| kind.is_none_or(|k| r.kind == k))
        .map(|r| r.index_count as usize / 3)
        .sum()
}

fn area(mesh: &PlantMesh, kind: Surface) -> f32 {
    let mut total = 0.0;
    for range in mesh.material_ranges.iter().filter(|r| r.kind == kind) {
        let from = range.first_index as usize;
        for tri in mesh.indices[from..from + range.index_count as usize].chunks_exact(3) {
            let p = |k: usize| mesh.vertices[tri[k] as usize].position;
            let (a, b, c) = (p(0), p(1), p(2));
            let (u, v) = (
                [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
                [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
            );
            let cross = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            total += 0.5 * cross.iter().map(|x| x * x).sum::<f32>().sqrt();
        }
    }
    total
}

fn extent(mesh: &PlantMesh) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for vertex in &mesh.vertices {
        for axis in 0..3 {
            lo[axis] = lo[axis].min(vertex.position[axis]);
            hi[axis] = hi[axis].max(vertex.position[axis]);
        }
    }
    (lo, hi)
}

// ---------- the plant mesh ----------

#[test]
fn an_oak_meshes_to_a_valid_closed_description_with_bark_and_leaves() {
    let skeleton = oak();
    let mesh = mesh_plant(&skeleton, &settings(), NEVER).unwrap();
    assert!(mesh.vertices.len() > 500 && mesh.indices.len().is_multiple_of(3));
    assert!(
        mesh.indices
            .iter()
            .all(|i| (*i as usize) < mesh.vertices.len()),
        "indices in range"
    );
    // The ranges cover the indices exactly, in order, bark and leaves both present.
    let mut cursor = 0;
    for range in &mesh.material_ranges {
        assert_eq!(range.first_index, cursor, "ranges are contiguous");
        assert!(range.index_count % 3 == 0 && range.index_count > 0);
        cursor += range.index_count;
    }
    assert_eq!(
        cursor as usize,
        mesh.indices.len(),
        "ranges cover every index"
    );
    assert!(
        triangles(&mesh, Some(Surface::Bark)) > 200 && triangles(&mesh, Some(Surface::Leaf)) > 0
    );
    // Triangles have area, normals and tangents are unit, UVs are finite.
    for tri in mesh.indices.chunks_exact(3) {
        let p = |k: usize| mesh.vertices[tri[k] as usize].position;
        let (a, b, c) = (p(0), p(1), p(2));
        let (u, v) = (
            [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
            [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
        );
        let cross = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        assert!(
            cross.iter().map(|x| x * x).sum::<f32>() > 1e-14,
            "a degenerate triangle"
        );
    }
    for vertex in &mesh.vertices {
        let n = vertex.normal.iter().map(|x| x * x).sum::<f32>().sqrt();
        let t = vertex.tangent[..3]
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        assert!(
            (n - 1.0).abs() < 1e-3 && (t - 1.0).abs() < 1e-3,
            "unit normal and tangent"
        );
        assert!(vertex.tangent[3].abs() == 1.0 && vertex.uv.iter().all(|x| x.is_finite()));
    }
}

#[test]
fn the_mesh_covers_the_skeleton_and_stays_near_it() {
    let skeleton = oak();
    let mesh = mesh_plant(&skeleton, &settings(), NEVER).unwrap();
    let (lo, hi) = extent(&mesh);
    assert!(
        hi[1] >= skeleton.height() * 0.95 && lo[1] > -0.1,
        "{lo:?} {hi:?} against height {}",
        skeleton.height()
    );
    let reach = skeleton
        .position
        .iter()
        .fold(0.0_f32, |m, p| m.max(p[0].abs()).max(p[2].abs()));
    let spread = hi[0]
        .abs()
        .max(lo[0].abs())
        .max(hi[2].abs())
        .max(lo[2].abs());
    assert!(
        spread < reach + 1.5 && spread > reach * 0.7,
        "horizontal reach {spread} against the skeleton's {reach}"
    );
}

#[test]
fn more_sides_make_more_vertices_and_leaf_modes_make_their_triangles() {
    let skeleton = oak();
    let mut coarse = settings();
    coarse.radial_segments = 3;
    let mut fine = settings();
    fine.radial_segments = 12;
    let (a, b) = (
        mesh_plant(&skeleton, &coarse, NEVER).unwrap(),
        mesh_plant(&skeleton, &fine, NEVER).unwrap(),
    );
    assert!(
        b.vertices.len() > a.vertices.len()
            && triangles(&b, Some(Surface::Bark)) > triangles(&a, Some(Surface::Bark))
    );
    let leaves = skeleton.leaves.len();
    let count = |cards: LeafCards| {
        let mut s = settings();
        s.leaf_cards = cards;
        triangles(
            &mesh_plant(&skeleton, &s, NEVER).unwrap(),
            Some(Surface::Leaf),
        )
    };
    assert_eq!(
        count(LeafCards::Quads {
            aspect: 0.5,
            bend_deg: 0.0
        }),
        leaves * 2
    );
    assert_eq!(count(LeafCards::Crossed { aspect: 0.5 }), leaves * 4);
    assert_eq!(
        count(LeafCards::Clumps {
            leaves: 3,
            spread: 40.0,
            aspect: 0.5
        }),
        leaves * 6
    );
}

#[test]
fn wind_data_has_three_scales_in_the_right_places() {
    let skeleton = oak();
    let mesh = mesh_plant(&skeleton, &settings(), NEVER).unwrap();
    let mut is_leaf = vec![false; mesh.vertices.len()];
    for range in mesh
        .material_ranges
        .iter()
        .filter(|r| r.kind == Surface::Leaf)
    {
        let from = range.first_index as usize;
        for index in &mesh.indices[from..from + range.index_count as usize] {
            is_leaf[*index as usize] = true;
        }
    }
    for (vertex, leaf) in mesh.vertices.iter().zip(&is_leaf) {
        let w = vertex.wind;
        for value in [w.phase, w.trunk, w.branch, w.leaf, w.edge] {
            assert!((0.0..=1.0).contains(&value), "wind weights in 0..=1: {w:?}");
        }
        assert!(w.pivot.iter().all(|v| v.is_finite()));
        if *leaf {
            assert!(w.leaf > 0.0, "leaf vertices flutter");
        } else {
            assert!(w.leaf == 0.0 && w.edge == 0.0, "bark does not flutter");
        }
    }
    let mean = |pick: &dyn Fn(&ve_world::lod::Vertex) -> bool| {
        let selected: Vec<f32> = mesh
            .vertices
            .iter()
            .filter(|v| pick(v))
            .map(|v| v.wind.trunk)
            .collect();
        selected.iter().sum::<f32>() / selected.len().max(1) as f32
    };
    assert!(
        mean(&|v| v.position[1] > 5.0) > mean(&|v| v.position[1] < 1.0),
        "the top sways more than the base"
    );
    assert!(
        mesh.vertices.iter().any(|v| v.wind.branch > 0.2),
        "branches have their own sway"
    );
}

#[test]
fn meshing_is_deterministic_cancellable_and_refuses_bad_settings() {
    let skeleton = oak();
    assert_eq!(
        mesh_plant(&skeleton, &settings(), NEVER).unwrap(),
        mesh_plant(&skeleton, &settings(), NEVER).unwrap()
    );
    assert!(matches!(
        mesh_plant(&skeleton, &settings(), &|| true),
        Err(LodError::Cancelled)
    ));
    let mut sides = settings();
    sides.radial_segments = 2;
    assert!(matches!(
        mesh_plant(&skeleton, &sides, NEVER),
        Err(LodError::BadParameter {
            name: "radial_segments",
            ..
        })
    ));
    sides.radial_segments = 17;
    assert!(matches!(
        mesh_plant(&skeleton, &sides, NEVER),
        Err(LodError::BadParameter {
            name: "radial_segments",
            ..
        })
    ));
    let mut blend = settings();
    blend.fork_blend = 1.5;
    assert!(matches!(
        mesh_plant(&skeleton, &blend, NEVER),
        Err(LodError::BadParameter {
            name: "fork_blend",
            ..
        })
    ));
    let mut segment = settings();
    segment.max_segment_length = 0.0;
    assert!(matches!(
        mesh_plant(&skeleton, &segment, NEVER),
        Err(LodError::BadParameter { .. })
    ));
}

// ---------- LOD chains ----------

#[test]
fn each_level_has_at_most_half_the_triangles_and_keeps_the_leaf_area_and_extent() {
    let skeleton = oak();
    let full = mesh_plant(&skeleton, &settings(), NEVER).unwrap();
    let chain = build_lods(&full, &skeleton, 4, NEVER).unwrap();
    assert_eq!(chain.levels.len(), 4);
    assert_eq!(chain.levels[0], full, "level 0 is the full mesh");
    assert_eq!(chain.switch_distance_per_size.len(), 3);
    assert!(
        chain
            .switch_distance_per_size
            .windows(2)
            .all(|w| w[1] > w[0])
            && chain.switch_distance_per_size[0] > 0.0
    );
    let (lo0, hi0) = extent(&full);
    let leaf_area = area(&full, Surface::Leaf);
    for level in 1..4 {
        let (now, before) = (
            triangles(&chain.levels[level], None),
            triangles(&chain.levels[level - 1], None),
        );
        assert!(
            now * 2 <= before,
            "level {level}: {now} triangles after {before}"
        );
        let leaf = area(&chain.levels[level], Surface::Leaf);
        assert!(
            (leaf - leaf_area).abs() / leaf_area < 0.10,
            "level {level} leaf area {leaf} against {leaf_area}"
        );
        let (lo, hi) = extent(&chain.levels[level]);
        for axis in 0..3 {
            let (full_size, size) = (hi0[axis] - lo0[axis], hi[axis] - lo[axis]);
            assert!(
                (size - full_size).abs() <= full_size * 0.10 + 0.05,
                "level {level} axis {axis}: {size} against {full_size}"
            );
        }
        assert!(
            chain.levels[level]
                .indices
                .iter()
                .all(|i| (*i as usize) < chain.levels[level].vertices.len())
        );
    }
    assert!(matches!(
        build_lods(&full, &skeleton, 1, NEVER),
        Err(LodError::BadParameter { name: "levels", .. })
    ));
    assert!(matches!(
        build_lods(&full, &skeleton, 6, NEVER),
        Err(LodError::BadParameter { name: "levels", .. })
    ));
    assert!(matches!(
        build_lods(&full, &skeleton, 3, &|| true),
        Err(LodError::Cancelled)
    ));
}

// ---------- impostors ----------

#[test]
fn impostor_views_surround_the_plant_and_the_bounds_contain_it() {
    let skeleton = oak();
    let mesh = mesh_plant(&skeleton, &settings(), NEVER).unwrap();
    let spec = impostor_spec(&mesh, 8, true);
    assert_eq!(spec.views.len(), 9, "eight around and one from above");
    let around: Vec<f32> = spec.views[..8]
        .iter()
        .map(|v| v.direction[2].atan2(v.direction[0]))
        .collect();
    let mut sorted = around.clone();
    sorted.sort_by(f32::total_cmp);
    let step = std::f32::consts::TAU / 8.0;
    for pair in sorted.windows(2) {
        assert!(
            (pair[1] - pair[0] - step).abs() < 1e-3,
            "evenly spaced in azimuth: {sorted:?}"
        );
    }
    for view in &spec.views {
        let (d, u) = (view.direction, view.up);
        assert!(
            (d.iter().map(|x| x * x).sum::<f32>().sqrt() - 1.0).abs() < 1e-4
                && (u.iter().map(|x| x * x).sum::<f32>().sqrt() - 1.0).abs() < 1e-4
        );
        assert!(view.half_width > 0.0 && view.half_height > 0.0);
    }
    assert!(
        spec.views[8].direction[1].abs() > 0.99,
        "the extra view looks straight down or up"
    );
    let c = spec.bounds_centre;
    assert!(mesh.vertices.iter().all(|v| {
        ((v.position[0] - c[0]).powi(2)
            + (v.position[1] - c[1]).powi(2)
            + (v.position[2] - c[2]).powi(2))
        .sqrt()
            <= spec.bounds_radius + 1e-3
    }));
    assert!(
        spec.bounds_radius < 12.0,
        "tight bounds: {}",
        spec.bounds_radius
    );
    assert!(u64::from(spec.atlas_grid[0]) * u64::from(spec.atlas_grid[1]) >= 9);
    assert_eq!(impostor_spec(&mesh, 4, false).views.len(), 4);
}

// ---------- grass ----------

#[test]
fn a_tuft_meshes_to_tapered_blades_with_wind_from_root_to_tip() {
    let params = Tuft {
        seed: 2,
        blades: 12,
        height: [0.15, 0.4],
        width: 0.012,
        bend_deg: 35.0,
        spread: 0.05,
    };
    let skeleton = grow_tuft(&params, NEVER).unwrap();
    let mesh = mesh_tuft(&skeleton, 4).unwrap();
    assert!(
        triangles(&mesh, Some(Surface::Leaf)) >= 12 * 4,
        "{} triangles",
        triangles(&mesh, None)
    );
    assert_eq!(
        triangles(&mesh, Some(Surface::Bark)),
        0,
        "grass is all leaf"
    );
    assert!(
        mesh.indices
            .iter()
            .all(|i| (*i as usize) < mesh.vertices.len())
    );
    let low = mesh
        .vertices
        .iter()
        .min_by(|a, b| a.position[1].total_cmp(&b.position[1]))
        .unwrap();
    let high = mesh
        .vertices
        .iter()
        .max_by(|a, b| a.position[1].total_cmp(&b.position[1]))
        .unwrap();
    assert!(
        low.wind.leaf < 0.1 && high.wind.leaf > 0.9,
        "root {} tip {}",
        low.wind.leaf,
        high.wind.leaf
    );
    assert!(matches!(
        mesh_tuft(&skeleton, 1),
        Err(LodError::BadParameter { .. })
    ));
    assert!(matches!(
        mesh_tuft(&skeleton, 9),
        Err(LodError::BadParameter { .. })
    ));
}

#[test]
#[ignore = "timing: run with `cargo test -p ve-world --release -- --ignored`"]
fn an_oak_meshes_in_well_under_two_hundred_milliseconds() {
    let skeleton = oak();
    let start = std::time::Instant::now();
    let full = mesh_plant(&skeleton, &settings(), NEVER).unwrap();
    let meshing = start.elapsed().as_secs_f32();
    let start = std::time::Instant::now();
    build_lods(&full, &skeleton, 4, NEVER).unwrap();
    let lods = start.elapsed().as_secs_f32();
    assert!(
        meshing < 0.2 && lods < 0.2,
        "meshing {meshing} s, lods {lods} s"
    );
}
