//! `terrain.tile`: world-aware ground tiles through the text language (ADR 0056).
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture setup unwraps data that is known to be valid and reports actionable failures"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g12_world").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))
}

fn tile(params: &str) -> Vec<[f32; 3]> {
    let source = format!("t = terrain.tile@1({params})\noutput t = t\n");
    evaluate(&source).unwrap().parts[0].positions.clone()
}

#[test]
fn the_basis_picks_the_noise_under_the_land_and_classic_keeps_existing_worlds() {
    let params = "size=[64, 64], cell=2, seed=3, height=50, scale=40";
    let default = tile(params);
    assert_eq!(
        default,
        tile(&format!("{params}, basis=\"classic\"")),
        "classic is the default, so worlds written before `basis` do not change"
    );
    for basis in ["perlin", "simplex", "value", "cellular"] {
        let land = tile(&format!("{params}, basis=\"{basis}\""));
        assert_ne!(land, default, "{basis} should change the land");
        assert_eq!(
            land,
            tile(&format!("{params}, basis=\"{basis}\"")),
            "{basis} is deterministic"
        );
        let hi = land.iter().fold(f32::MIN, |h, p| h.max(p[1]));
        assert!(hi > 5.0 && hi <= 50.0 + 1e-3, "{basis}: tallest {hi}");
    }
    let refused = evaluate(&format!(
        "t = terrain.tile@1({params}, basis=\"bogus\")\noutput t = t\n"
    ));
    assert!(refused.is_err(), "an unknown basis is refused, not ignored");
}

#[test]
fn the_default_tile_is_a_grid_of_land_with_uvs() {
    let result = evaluate("t = terrain.tile@1()\noutput t = t\n").unwrap();
    let part = &result.parts[0];
    assert_eq!(
        part.indices.len() / 3,
        2 * 128 * 128,
        "two triangles per cell of a 256 m tile at 2 m"
    );
    assert_eq!(
        part.uvs.len(),
        part.indices.len(),
        "one UV per triangle corner"
    );
    let (lo, hi) = part
        .positions
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), p| (l.min(p[1]), h.max(p[1])));
    assert!(
        lo >= -1e-3 && hi <= 120.0 + 1e-3 && hi - lo > 20.0,
        "land between 0 and 120 m: {lo}..{hi}"
    );
}

#[test]
fn neighbouring_tiles_meet_exactly_and_the_seed_decides_the_land() {
    let left = tile("size=[64, 64], cell=2, tile_x=0, seed=3");
    let right = tile("size=[64, 64], cell=2, tile_x=1, seed=3");
    let edge = |points: &[[f32; 3]], x: f32| {
        let mut row: Vec<[f32; 3]> = points
            .iter()
            .copied()
            .filter(|p| (p[0] - x).abs() < 1e-4)
            .collect();
        row.sort_by(|a, b| a[2].total_cmp(&b[2]));
        row
    };
    let (a, b) = (edge(&left, 64.0), edge(&right, 64.0));
    assert_eq!(a.len(), 33);
    assert_eq!(a, b, "the shared edge is the same points");
    assert_ne!(left, tile("size=[64, 64], cell=2, tile_x=0, seed=4"));
    assert_eq!(
        left,
        tile("size=[64, 64], cell=2, tile_x=0, seed=3"),
        "same seed, same land"
    );
}

#[test]
fn neighbouring_tiles_shade_alike_along_their_shared_edge() {
    let normals_of = |tile_x: i32| {
        let source = format!(
            "t = terrain.tile@1(size=[64, 64], cell=2, tile_x={tile_x}, seed=3)\nm = solid.material@1(t, color=[0.4, 0.5, 0.3])\noutput t = m\n"
        );
        let part = evaluate(&source).unwrap().parts.remove(0);
        assert_eq!(
            part.normals.len(),
            part.positions.len(),
            "a tile carries one normal per vertex"
        );
        assert!(part.normals.iter().all(|n| {
            ((n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() - 1.0).abs() < 1e-3 && n[1] > 0.0
        }));
        let mut edge: Vec<([f32; 3], [f32; 3])> = part
            .positions
            .iter()
            .copied()
            .zip(part.normals.iter().copied())
            .filter(|(p, _)| (p[0] - 64.0).abs() < 1e-4)
            .collect();
        edge.sort_by(|a, b| a.0[2].total_cmp(&b.0[2]));
        edge
    };
    // The right edge of tile 0 and the left edge of tile 1 are the same world line: x = 64.
    let (left, right) = (normals_of(0), {
        let source = "t = terrain.tile@1(size=[64, 64], cell=2, tile_x=1, seed=3)\nm = solid.material@1(t, color=[0.4, 0.5, 0.3])\noutput t = m\n";
        let part = evaluate(source).unwrap().parts.remove(0);
        let mut edge: Vec<([f32; 3], [f32; 3])> = part
            .positions
            .iter()
            .copied()
            .zip(part.normals.iter().copied())
            .filter(|(p, _)| (p[0] - 64.0).abs() < 1e-4)
            .collect();
        edge.sort_by(|a, b| a.0[2].total_cmp(&b.0[2]));
        edge
    });
    assert_eq!(left.len(), right.len());
    for ((_, a), (_, b)) in left.iter().zip(&right) {
        assert!(
            (0..3).all(|k| (a[k] - b[k]).abs() < 1e-5),
            "{a:?} against {b:?}"
        );
    }
}

#[test]
fn terrain_material_carries_the_look_with_the_surface() {
    let source = "t = terrain.tile@1(size=[64, 64], cell=4)\nm = terrain.material@1(t, rock_slope=45, snow_line=100, snow_blend=10, sand_line=2, grass=[0.1, 0.5, 0.1])\noutput m = m\n";
    let part = evaluate(source).unwrap().parts.remove(0);
    let terrain = part
        .material
        .as_ref()
        .unwrap()
        .finish
        .unwrap()
        .terrain
        .unwrap();
    assert!(
        (terrain.rock[0] - 0.5).abs() < 1e-6,
        "45 degrees is half of a right angle: {}",
        terrain.rock[0]
    );
    assert!((terrain.snow[0] - 100.0).abs() < 1e-4 && (terrain.sand[0] - 2.0).abs() < 1e-4);
    assert!((terrain.colors[0][1] - 0.5).abs() < 1e-6);
    let error = evaluate(
        "g = poly.grid@1(width=1, height=1, columns=2, rows=2)\nm = terrain.material@1(g, rock_slope=45)\noutput m = m\n",
    );
    assert!(
        error.is_ok(),
        "any polygon surface can wear it, not only a tile"
    );
}

/// Where the copies of the first part stand: the translation of each placement matrix.
fn planted(source: &str) -> Vec<[f32; 3]> {
    let result = evaluate(source).unwrap();
    let transforms = result.parts[0]
        .transforms
        .as_ref()
        .expect("a placed set has transforms");
    transforms.iter().map(|m| [m[12], m[13], m[14]]).collect()
}

fn forest(tile_x: i32, params: &str) -> String {
    format!(
        "t = terrain.tile@1(size=[128, 128], cell=2, tile_x={tile_x}, seed=4, height=60, scale=160)\nb = solid.box@1(size=[1, 3, 1])\nf = scatter.place@1(b, t, {params})\noutput f = f\n"
    )
}

#[test]
fn plants_stand_on_the_ground_inside_their_tile_with_their_spacing() {
    let points = planted(&forest(0, "radius=8, seed=2"));
    assert!((60..260).contains(&points.len()), "{} plants", points.len());
    assert!(points.iter().all(|p| (0.0..128.0).contains(&p[0])
        && (0.0..128.0).contains(&p[2])
        && (-1e-3..=60.001).contains(&p[1])));
    let closest = points
        .iter()
        .enumerate()
        .flat_map(|(i, a)| {
            points[i + 1..]
                .iter()
                .map(move |b| ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt())
        })
        .fold(f32::MAX, f32::min);
    assert!(closest >= 8.0 * 0.999, "plants {closest} m apart");
    assert_eq!(
        points,
        planted(&forest(0, "radius=8, seed=2")),
        "same seed, same plants"
    );
    assert_ne!(points, planted(&forest(0, "radius=8, seed=3")));
}

#[test]
fn neighbouring_tiles_plant_without_gaps_or_duplicates() {
    let (left, right) = (
        planted(&forest(0, "radius=8, seed=2")),
        planted(&forest(1, "radius=8, seed=2")),
    );
    let all: Vec<[f32; 3]> = left.iter().chain(&right).copied().collect();
    let closest = all
        .iter()
        .enumerate()
        .flat_map(|(i, a)| {
            all[i + 1..]
                .iter()
                .map(move |b| ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt())
        })
        .fold(f32::MAX, f32::min);
    assert!(
        closest >= 8.0 * 0.999,
        "across the tile edge plants are still {closest} m apart"
    );
    assert!(left.iter().all(|p| p[0] < 128.0) && right.iter().all(|p| p[0] >= 128.0));
}

#[test]
fn a_mask_layer_a_height_window_and_a_slope_limit_decide_where_plants_grow() {
    let all = planted(&forest(0, "radius=6, seed=1")).len();
    let flat_only = "t = terrain.tile@1(size=[128, 128], cell=2, seed=4, height=60, scale=160)\nm = terrain.mask@1(t, mask=\"slope\", name=\"flat\", from=0, to=0.25, invert=true)\nb = solid.box@1(size=[1, 3, 1])\nf = scatter.place@1(b, m, radius=6, seed=1, layer=\"flat\")\noutput f = f\n";
    // The height window sits at the tile's own median ground height, so it holds ground whatever the noise made.
    let mut heights: Vec<f32> = tile("size=[128, 128], cell=2, seed=4, height=60, scale=160")
        .iter()
        .map(|p| p[1])
        .collect();
    heights.sort_by(f32::total_cmp);
    let median = heights[heights.len() / 2];
    let low_ground = planted(&forest(
        0,
        &format!("radius=6, seed=1, max_height={median}"),
    ));
    assert!(
        planted(flat_only).len() < all,
        "a mask layer thins the plants"
    );
    assert!(
        planted(&forest(0, "radius=6, seed=1, max_slope=24")).len() < all,
        "a slope limit thins the plants"
    );
    assert!(
        !low_ground.is_empty()
            && low_ground.len() < all
            && low_ground.iter().all(|p| p[1] <= median + 2.0),
        "a height window keeps low ground only (to within one cell of slope)"
    );
    let missing = evaluate(&forest(0, "layer=\"nope\"")).unwrap_err();
    assert!(
        missing.contains("scatter.unknown_layer") && missing.contains("terrain.mask"),
        "{missing}"
    );
    // No suitable ground is a warning, not an error: one barren tile must not stop a whole world.
    let nothing = evaluate(&forest(0, "min_height=500")).unwrap();
    assert!(
        nothing.warnings.iter().any(|w| w.code == "scatter.nothing"),
        "{:?}",
        nothing.warnings
    );
    assert!(
        nothing
            .parts
            .iter()
            .all(|p| p.transforms.as_ref().is_none_or(Vec::is_empty))
    );
    let not_a_tile = evaluate("g = poly.grid@1(width=1, height=1, columns=2, rows=2)\nb = solid.box@1(size=[1, 1, 1])\nf = scatter.place@1(b, g)\noutput f = f\n").unwrap_err();
    assert!(not_a_tile.contains("terrain.needs_tile"), "{not_a_tile}");
}

const RIVER: &str = "path=\"6,40; 120,66; 250,50\", width=6, depth=2.5, bank_width=5";

fn river_source(tile_x: i32, tail: &str) -> String {
    format!(
        "t = terrain.tile@1(size=[128, 128], cell=2, tile_x={tile_x}, seed=4, height=60, scale=160)\nr = river.carve@1(t, {RIVER})\n{tail}"
    )
}

#[test]
fn a_river_is_cut_into_the_tile_and_bad_paths_are_explained() {
    let plain = tile("size=[128, 128], cell=2, tile_x=0, seed=4, height=60, scale=160");
    let carved = evaluate(&river_source(0, "output r = r\n")).unwrap().parts[0]
        .positions
        .clone();
    assert_eq!(plain.len(), carved.len());
    let lowered = plain
        .iter()
        .zip(&carved)
        .filter(|(a, b)| a[1] - b[1] > 1.5)
        .count();
    assert!(lowered > 150, "{lowered} cells cut by more than 1.5 m");
    // Smoothing the valley floor (`flatten`) can lift a small hollow beside the channel, but never by much.
    assert!(
        plain.iter().zip(&carved).all(|(a, b)| b[1] <= a[1] + 1.0),
        "a river does not heap up the ground"
    );
    let one_point = evaluate("t = terrain.tile@1(size=[128, 128], cell=2)\nr = river.carve@1(t, path=\"10,10\")\noutput r = r\n").unwrap_err();
    assert!(one_point.contains("river.bad_path"), "{one_point}");
    let malformed = evaluate("t = terrain.tile@1(size=[128, 128], cell=2)\nr = river.carve@1(t, path=\"10,10; abc\")\noutput r = r\n").unwrap_err();
    assert!(
        malformed.contains("river.bad_path") && malformed.contains("x,z"),
        "{malformed}"
    );
    let outside = evaluate("t = terrain.tile@1(size=[128, 128], cell=2)\nr = river.carve@1(t, path=\"900,900; 1000,900\")\noutput r = r\n").unwrap_err();
    assert!(outside.contains("river.bad_path"), "{outside}");
}

#[test]
fn river_water_is_a_sheet_with_flow_depth_and_foam_per_vertex() {
    let result = evaluate(&river_source(0, "w = river.water@1(r)\noutput w = w\n")).unwrap();
    let part = &result.parts[0];
    assert!(
        part.positions.len() > 100 && part.indices.len() >= 3 * 100,
        "{} vertices",
        part.positions.len()
    );
    assert_eq!(
        part.colors.len(),
        part.positions.len(),
        "one colour per vertex"
    );
    assert!(
        part.colors
            .iter()
            .all(|c| c.iter().all(|v| (0.0..=1.0).contains(v)))
    );
    assert!(part.colors.iter().any(|c| c[2] > 0.2), "some deep water");
    assert!(part.colors.iter().any(|c| c[3] > 0.8), "foam at the edges");
    assert!(
        part.indices
            .iter()
            .all(|i| (*i as usize) < part.positions.len())
    );
    let material = part.material.as_ref().unwrap();
    assert!(
        material.opacity < 1.0 && material.finish.unwrap().water.is_some(),
        "translucent with a water look"
    );
    let missing = evaluate(
        "t = terrain.tile@1(size=[128, 128], cell=2)\nw = river.water@1(t)\noutput w = w\n",
    )
    .unwrap_err();
    assert!(
        missing.contains("river.needs_carve") && missing.contains("river.carve"),
        "{missing}"
    );
}

#[test]
fn a_river_carried_over_two_tiles_meets_at_the_edge() {
    let sheet = |tile_x: i32| {
        evaluate(&river_source(
            tile_x,
            "w = river.water@1(r)\noutput w = w\n",
        ))
        .unwrap()
        .parts
        .remove(0)
        .positions
    };
    let (left, right) = (sheet(0), sheet(1));
    let edge = |points: &[[f32; 3]]| {
        let mut row: Vec<[f32; 3]> = points
            .iter()
            .copied()
            .filter(|p| (p[0] - 128.0).abs() < 1e-3)
            .collect();
        row.sort_by(|a, b| a[2].total_cmp(&b[2]));
        row
    };
    let (a, b) = (edge(&left), edge(&right));
    assert!(
        !a.is_empty() && !b.is_empty(),
        "the water reaches the shared edge from both sides"
    );
    for p in &a {
        if let Some(q) = b.iter().find(|q| (q[2] - p[2]).abs() < 1e-3) {
            assert!(
                (p[1] - q[1]).abs() < 0.05,
                "water height {} against {} at z = {}",
                p[1],
                q[1],
                p[2]
            );
        }
    }
}

fn triangles(source: &str) -> Vec<usize> {
    evaluate(source)
        .unwrap()
        .parts
        .iter()
        .map(|p| p.indices.len() / 3)
        .collect()
}

#[test]
fn a_tree_has_bark_and_cut_out_leaves_standing_at_the_origin() {
    let result =
        evaluate("t = flora.tree@1(kind=\"oak\", height=8, seed=2)\noutput t = t\n").unwrap();
    assert_eq!(
        result.parts.len(),
        2,
        "bark and leaves are two parts of one output"
    );
    let (bark, leaves) = (&result.parts[0], &result.parts[1]);
    let bark_material = bark.material.as_ref().unwrap();
    assert!(
        bark_material.finish.is_some_and(|f| f.relief > 0.5),
        "bark has relief"
    );
    let leaf_material = leaves.material.as_ref().unwrap();
    assert!(
        leaf_material
            .texture_program
            .as_deref()
            .is_some_and(|p| p.contains("expr(")),
        "a drawn leaf shape"
    );
    assert!(
        leaf_material.foliage.is_some_and(|f| f.alpha_cutoff > 0.0),
        "cut out by the leaf shape"
    );
    assert_eq!(
        leaves.uvs.len(),
        leaves.indices.len(),
        "leaf cards carry their UVs"
    );
    let all: Vec<[f32; 3]> = result
        .parts
        .iter()
        .flat_map(|p| p.positions.iter().copied())
        .collect();
    let top = all.iter().fold(f32::MIN, |m, p| m.max(p[1]));
    let bottom = all.iter().fold(f32::MAX, |m, p| m.min(p[1]));
    assert!(
        (6.5..10.5).contains(&top) && bottom > -0.2,
        "an 8 m oak: {bottom} to {top}"
    );
    assert!(bark.indices.len() / 3 > 200 && leaves.indices.len() / 3 > 100);
}

#[test]
fn species_differ_and_levels_of_detail_halve_the_triangles() {
    let count = |kind: &str| {
        triangles(&format!(
            "t = flora.tree@1(kind=\"{kind}\", height=8, seed=2)\noutput t = t\n"
        ))
        .iter()
        .sum::<usize>()
    };
    let sizes: Vec<usize> = ["oak", "pine", "birch", "willow", "shrub", "palm"]
        .iter()
        .map(|k| count(k))
        .collect();
    let mut unique = sizes.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 6, "each species is its own shape: {sizes:?}");
    let level = |lod: u32| {
        triangles(&format!(
            "t = flora.tree@1(kind=\"oak\", height=8, seed=2, lod={lod})\noutput t = t\n"
        ))
        .iter()
        .sum::<usize>()
    };
    let (full, one, two) = (level(0), level(1), level(2));
    assert!(
        one * 2 <= full && two * 2 <= one,
        "{full} then {one} then {two}"
    );
    assert!(
        evaluate("t = flora.tree@1(kind=\"baobab\")\noutput t = t\n").is_err(),
        "an unknown species is refused"
    );
}

#[test]
fn trees_plant_into_a_forest_with_scatter_place() {
    let source = "t = terrain.tile@1(size=[128, 128], cell=2, seed=4, height=40, scale=160)\ntree = flora.tree@1(kind=\"pine\", height=10, seed=1)\nforest = scatter.place@1(tree, t, radius=9, seed=3, max_slope=26, sink=0.1)\noutput forest = forest\n";
    let result = evaluate(source).unwrap();
    assert_eq!(
        result.parts.len(),
        2,
        "one placed set per material of the tree"
    );
    let planted = |p: &ve_modeling::Part| p.transforms.as_ref().map_or(0, Vec::len);
    assert!(
        planted(&result.parts[0]) > 40 && planted(&result.parts[0]) == planted(&result.parts[1]),
        "bark and leaves are planted together"
    );
}

#[test]
fn an_island_falls_to_sea_level_beyond_its_radius() {
    let far = tile("size=[64, 64], cell=4, tile_x=5, tile_z=5, island_radius=100, height=50");
    assert!(
        far.iter().all(|p| p[1].abs() < 1e-3),
        "320 m from the centre is beyond a 100 m island"
    );
}

#[test]
fn a_tile_over_the_size_limit_is_refused_with_the_fix() {
    let error =
        evaluate("t = terrain.tile@1(size=[4096, 4096], cell=0.25)\noutput t = t\n").unwrap_err();
    assert!(
        error.contains("terrain.too_large") && error.contains("cell"),
        "{error}"
    );
}

#[test]
fn the_tile_is_an_editable_polygon_mesh() {
    let result = evaluate(
        "t = terrain.tile@1(size=[64, 64], cell=4)\ns = poly.subdivide@1(t, levels=1)\noutput s = s\n",
    )
    .unwrap();
    assert_eq!(
        result.parts[0].indices.len() / 3,
        4 * 2 * 16 * 16,
        "one level turns each quad into four"
    );
}

const WEATHER: &str = "river_erosion=0.8, river_iterations=20, talus=33, thermal_iterations=10, rain=0.02, hydraulic_iterations=10, pull=0.3, coarse_cell=16, world_size=1024";

fn eroded(tile_x: i32) -> Vec<[f32; 3]> {
    let source = format!(
        "t = terrain.tile@1(size=[64, 64], cell=2, tile_x={tile_x}, seed=5, height=90, scale=120)\ne = terrain.erode@1(t, {WEATHER})\noutput e = e\n"
    );
    evaluate(&source).unwrap().parts[0].positions.clone()
}

#[test]
fn a_mask_needs_a_tile_and_leaves_the_mesh_alone() {
    let error = evaluate("g = poly.grid@1(width=1, height=1, columns=2, rows=2)\nm = terrain.mask@1(g)\noutput m = m\n").unwrap_err();
    assert!(error.contains("terrain.needs_tile"), "{error}");
    let plain = tile("size=[64, 64], cell=2, seed=5");
    let source = "t = terrain.tile@1(size=[64, 64], cell=2, seed=5)\nm = terrain.mask@1(t, mask=\"occlusion\", name=\"hollows\", from=0.1, to=0.6, smooth=true)\noutput m = m\n";
    assert_eq!(evaluate(source).unwrap().parts[0].positions, plain);
}

#[test]
fn erosion_reshapes_the_ground_deterministically() {
    let plain = tile("size=[64, 64], cell=2, tile_x=0, seed=5, height=90, scale=120");
    let (a, b) = (eroded(0), eroded(0));
    assert_eq!(a, b, "same settings, same land");
    assert_ne!(a, plain);
    assert!(a.iter().all(|p| p[1].is_finite() && p[1] >= 0.0));
    let (before, after): (f32, f32) = (
        plain.iter().map(|p| p[1]).sum(),
        a.iter().map(|p| p[1]).sum(),
    );
    assert!(
        after < before,
        "weathering lowers the ground on average: {before} -> {after}"
    );
}

#[test]
fn eroded_neighbours_meet_without_a_seam() {
    let (left, right) = (eroded(0), eroded(1));
    let edge = |points: &[[f32; 3]], x: f32| {
        let mut row: Vec<[f32; 3]> = points
            .iter()
            .copied()
            .filter(|p| (p[0] - x).abs() < 1e-4)
            .collect();
        row.sort_by(|a, b| a[2].total_cmp(&b[2]));
        row
    };
    let (a, b) = (edge(&left, 64.0), edge(&right, 64.0));
    assert_eq!(a.len(), b.len());
    let worst = a
        .iter()
        .zip(&b)
        .map(|(p, q)| (p[1] - q[1]).abs())
        .fold(0.0_f32, f32::max);
    assert!(worst < 0.5, "the shared edge differs by up to {worst} m");
}

#[test]
fn terraces_cut_the_ground_into_steps() {
    let steps = tile("size=[64, 64], cell=2, terraces=4, terrace_sharpness=1");
    let mut levels: Vec<i64> = steps
        .iter()
        .map(|p| (p[1] * 100.0).round() as i64)
        .collect();
    levels.sort_unstable();
    levels.dedup();
    assert!(levels.len() <= 5, "{} distinct heights", levels.len());
}
