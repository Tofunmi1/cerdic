//! The facade grammar produces reusable, material-separated close-view assets.
#![expect(
    clippy::unwrap_used,
    reason = "the fixture is deliberately valid and test failures should be direct"
)]

use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|error| format!("{error:?}"))?
        .commands;
    let mut session = Session::with_builtin(73);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_building_asset").unwrap(),
        label: None,
        commands,
    });
    if !matches!(outcome, Outcome::Committed { .. }) {
        return Err(format!("{outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

fn part_program(part: &str, detail: u8) -> String {
    format!(
        "b = building.asset@1(width=24, depth=18, floors=8, bays=6, floor_height=3.6, detail={detail}, style=\"art_deco\", part=\"{part}\", seed=17)\noutput b = b\n"
    )
}

#[test]
fn each_facade_material_group_builds_geometry_with_corner_uvs() {
    for style in [
        "art_deco",
        "classical",
        "tenement",
        "brownstone",
        "precast",
        "curtainwall",
        "industrial",
        "brutalist",
        "modernist",
        "mediterranean",
        "victorian",
        "warehouse",
    ] {
        for part in ["masonry", "glass", "trim", "roof", "doors"] {
            let source = part_program(part, 3).replace("art_deco", style);
            let result = evaluate(&source).unwrap();
            let mesh = &result.parts[0];
            assert!(!mesh.indices.is_empty(), "{style} {part} mesh is empty");
            assert_eq!(
                mesh.uvs.len(),
                mesh.indices.len(),
                "{style} {part} UV corners"
            );
            assert!(
                mesh.positions
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite())
            );
            assert!(mesh.uvs.iter().flatten().all(|value| value.is_finite()));
        }
    }
}

#[test]
fn facade_grammar_is_deterministic_and_detail_changes_trim() {
    let source = part_program("trim", 3);
    let first = evaluate(&source).unwrap();
    let second = evaluate(&source).unwrap();
    assert_eq!(first.parts[0].positions, second.parts[0].positions);
    assert_eq!(first.parts[0].indices, second.parts[0].indices);
    let clean = evaluate(&part_program("trim", 0)).unwrap();
    assert!(first.parts[0].indices.len() > clean.parts[0].indices.len());
}

#[test]
fn facade_dimensions_and_style_are_bounded_by_the_operator_contract() {
    for invalid in [
        "b = building.asset@1(width=7)\noutput b = b\n",
        "b = building.asset@1(floors=41)\noutput b = b\n",
        "b = building.asset@1(style=\"gothic\")\noutput b = b\n",
    ] {
        assert!(
            parse_text(&builtin_registry(), invalid).is_err(),
            "{invalid}"
        );
    }
    let result = evaluate(&part_program("roof", 2)).unwrap();
    let top = result.parts[0]
        .positions
        .iter()
        .map(|point| point[1])
        .fold(f32::MIN, f32::max);
    assert!(top > 28.0, "eight storeys should exceed 28 m: {top}");
}

#[test]
fn footprint_dimensions_create_long_slabs_and_narrow_towers() {
    for (width, depth) in [(32.0, 12.0), (12.0, 32.0)] {
        let source = format!(
            "b = building.asset@1(width={width}, depth={depth}, floors=2, bays=3, style=\"classical\", part=\"roof\")\noutput b = b\n"
        );
        let result = evaluate(&source).unwrap();
        let positions = &result.parts[0].positions;
        let min_x = positions.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
        let max_x = positions.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        let min_z = positions.iter().map(|p| p[2]).fold(f32::MAX, f32::min);
        let max_z = positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
        assert!(((max_x - min_x) - width).abs() < 0.01);
        assert!(((max_z - min_z) - depth).abs() < 0.01);
    }
}

#[test]
fn wall_and_glass_triangles_face_outward_on_every_facade() {
    for style in [
        "art_deco",
        "classical",
        "tenement",
        "brownstone",
        "precast",
        "curtainwall",
        "industrial",
        "brutalist",
        "modernist",
        "mediterranean",
        "victorian",
        "warehouse",
    ] {
        for part in ["masonry", "glass"] {
            let source = format!(
                "b = building.asset@1(width=24, depth=18, floors=4, bays=5, detail=1, style=\"{style}\", part=\"{part}\")\noutput b = b\n"
            );
            let result = evaluate(&source).unwrap();
            let mesh = &result.parts[0];
            for triangle in mesh.indices.chunks_exact(3) {
                let [a, b, c] = [
                    mesh.positions[triangle[0] as usize],
                    mesh.positions[triangle[1] as usize],
                    mesh.positions[triangle[2] as usize],
                ];
                let normal = [
                    (b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1]),
                    (b[2] - a[2]) * (c[0] - a[0]) - (b[0] - a[0]) * (c[2] - a[2]),
                    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]),
                ];
                if normal[0].abs() + normal[2].abs() < 1e-6 {
                    continue;
                }
                let centre = [(a[0] + b[0] + c[0]) / 3.0, (a[2] + b[2] + c[2]) / 3.0];
                assert!(
                    normal[0] * centre[0] + normal[2] * centre[1] > 0.0,
                    "{style} {part} has an inward-facing facade triangle at {centre:?}"
                );
            }
        }
    }
}

#[test]
fn residential_grammar_adds_projecting_balconies() {
    let source = "b = building.asset@1(width=24, depth=18, floors=5, bays=6, style=\"victorian\", detail=0, part=\"trim\")\noutput b = b\n";
    let result = evaluate(source).unwrap();
    let frontmost_z = result.parts[0]
        .positions
        .iter()
        .map(|point| point[2])
        .fold(f32::INFINITY, f32::min);
    assert!(
        frontmost_z < -9.5,
        "balconies should project from the facade: {frontmost_z}"
    );
}
