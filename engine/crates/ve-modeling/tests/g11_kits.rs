//! G11: the shipped kit library parses, evaluates cleanly, matches its documented footprint,
//! stands on the floor, stays closed and deterministic, and every exposed parameter does what it says.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture failures include the model diagnostic rather than hiding it"
)]
use std::collections::BTreeSet;
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, KITS, Kit, Quality};

/// The kits added for G11. The original three are checked too where their shape allows it.
const NEW_KITS: &str = "arch barrel bed bench chair column crate door fence lamp_floor pipe_run \
     railing roof_gable sign_post stairs table wall_window";

fn kit(name: &str) -> &'static Kit {
    KITS.iter()
        .find(|kit| kit.name == name)
        .unwrap_or_else(|| panic!("no kit named {name}"))
}

/// Parses, commits and evaluates one call; any diagnostic (error or warning) fails the test.
fn run(name: &str, args: &str) -> Evaluation {
    let source = format!("{}\nitem = {name}({args})\noutput item", kit(name).source);
    let parsed = parse_text(&builtin_registry(), &source)
        .unwrap_or_else(|errors| panic!("{name} parse errors: {errors:?}"));
    let mut session = Session::with_builtin(1101);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_kits").unwrap(),
        label: None,
        commands: parsed.commands,
    });
    let committed = matches!(outcome, Outcome::Committed { .. });
    assert!(committed, "{name}: {outcome:?}");
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|error| {
            panic!(
                "{name}: {}: {} (node {:?})",
                error.code, error.message, error.node
            )
        });
    assert!(result.warnings.is_empty(), "{name}: {:?}", result.warnings);
    assert!(!result.parts.is_empty(), "{name} produced nothing");
    result
}

fn bounds(result: &Evaluation) -> [[f32; 2]; 3] {
    let mut out = [[f32::INFINITY, f32::NEG_INFINITY]; 3];
    for point in result.parts.iter().flat_map(|part| &part.positions) {
        for (axis, range) in out.iter_mut().enumerate() {
            range[0] = range[0].min(point[axis]);
            range[1] = range[1].max(point[axis]);
        }
    }
    out
}

fn extent(result: &Evaluation, axis: usize) -> f32 {
    let range = bounds(result)[axis];
    range[1] - range[0]
}

fn triangles(result: &Evaluation) -> usize {
    result.parts.iter().map(|part| part.indices.len() / 3).sum()
}

fn total_volume(result: &Evaluation) -> f64 {
    result.parts.iter().map(|part| part.volume).sum()
}

fn positions(result: &Evaluation) -> Vec<[f32; 3]> {
    let position = |part: &ve_modeling::Part| part.positions.clone();
    result.parts.iter().flat_map(position).collect()
}

fn material_colors(result: &Evaluation) -> Vec<[f32; 3]> {
    let color = |part: &ve_modeling::Part| part.material.as_ref().map(|m| m.color);
    result.parts.iter().filter_map(color).collect()
}

fn roughness_bits(result: &Evaluation) -> Vec<u32> {
    let bits = |part: &ve_modeling::Part| part.material.as_ref().map(|m| m.roughness.to_bits());
    result.parts.iter().filter_map(bits).collect()
}

/// Every edge of the part's triangle mesh joins exactly two triangles.
fn closed(part: &ve_modeling::Part) -> bool {
    let mut edges = std::collections::BTreeMap::new();
    for triangle in part.indices.chunks_exact(3) {
        for (a, b) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            *edges.entry((a.min(b), a.max(b))).or_insert(0_usize) += 1;
        }
    }
    !edges.is_empty() && edges.values().all(|count| *count == 2)
}

/// The `W x H x D m` run at the start of every kit's `doc`.
fn documented_size(doc: &str) -> [f32; 3] {
    let words: Vec<&str> = doc.split_whitespace().collect();
    for [w, x1, h, x2, d, unit] in words
        .windows(6)
        .map(|w| [w[0], w[1], w[2], w[3], w[4], w[5]])
    {
        let metres = matches!(unit.trim_end_matches([':', ',', ';']), "m");
        if x1 == "x"
            && x2 == "x"
            && metres
            && let (Ok(width), Ok(height), Ok(depth)) = (w.parse(), h.parse(), d.parse())
        {
            return [width, height, depth];
        }
    }
    panic!("no `W x H x D m` footprint in {doc:?}");
}

fn exposes(source: &str) -> Vec<&str> {
    source
        .lines()
        .filter_map(|line| line.trim().strip_prefix("expose "))
        .filter_map(|rest| rest.split_once('=').map(|(name, _)| name.trim()))
        .collect()
}

#[test]
fn kit_names_are_unique_and_sorted() {
    let names: Vec<&str> = KITS.iter().map(|kit| kit.name).collect();
    let unique: BTreeSet<&str> = names.iter().copied().collect();
    assert_eq!(unique.len(), names.len(), "duplicate kit names");
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "kits are not sorted by name");
}

#[test]
fn every_kit_evaluates_within_its_documented_footprint() {
    for kit in KITS {
        let expected = documented_size(kit.doc);
        let found = bounds(&run(kit.name, ""));
        for axis in 0..3 {
            let actual = found[axis][1] - found[axis][0];
            let tolerance = (expected[axis] * 0.02).max(0.002);
            assert!(
                (actual - expected[axis]).abs() <= tolerance,
                "{} axis {axis}: measured {actual}, doc says {} ({:?})",
                kit.name,
                expected[axis],
                kit.doc
            );
        }
    }
}

#[test]
fn new_kits_stand_on_the_floor_with_a_centred_origin() {
    for name in NEW_KITS.split_whitespace() {
        let found = bounds(&run(name, ""));
        assert!(
            found[1][0].abs() < 1.0e-3,
            "{name}: floor at {}",
            found[1][0]
        );
        for axis in [0, 2] {
            let centre = (found[axis][0] + found[axis][1]) / 2.0;
            assert!(centre.abs() < 5.0e-3, "{name}: axis {axis} centre {centre}");
        }
    }
}

#[test]
fn new_kits_are_closed_solids() {
    // `shelf` and `window` ship two material regions; each region is an open patch of one closed
    // surface, so per-part watertightness is not meaningful for them. Every G11 kit and the desk
    // use one material, so the final mesh should be one closed solid.
    for name in NEW_KITS.split_whitespace().chain(["desk"]) {
        let result = run(name, "");
        assert_eq!(
            result.parts.len(),
            1,
            "{name}: expected one material region"
        );
        assert!(closed(&result.parts[0]), "{name} has open mesh edges");
    }
}

#[test]
fn kits_are_deterministic() {
    for kit in KITS {
        let (first, second) = (run(kit.name, ""), run(kit.name, ""));
        assert_eq!(triangles(&first), triangles(&second), "{}", kit.name);
        assert_eq!(total_volume(&first), total_volume(&second), "{}", kit.name);
        assert_eq!(positions(&first), positions(&second), "{}", kit.name);
    }
}

#[test]
fn stairs_step_count_multiplies_the_rise() {
    let (six, eight) = (run("stairs", ""), run("stairs", "steps=8"));
    assert!((extent(&six, 1) - 6.0 * 0.18).abs() < 1.0e-3);
    assert!((extent(&eight, 1) - 8.0 * 0.18).abs() < 1.0e-3);
    assert!(triangles(&eight) > triangles(&six));
}

#[derive(Clone, Copy)]
enum Check {
    AxisUp(usize),
    VolumeUp,
    VolumeDown,
    TrianglesUp,
    Color,
    Roughness,
}

type Case = (&'static str, &'static str, &'static str, Check);

/// One mutation per exposed parameter, except seeds: `exposed_seeds_change_the_layout` covers them.
#[rustfmt::skip]
const CASES: &[Case] = &[
    ("arch", "wall", "[1.9, 1.6, 0.3]", Check::AxisUp(0)),
    ("arch", "portal", "[0.8, 0.9, 0.4]", Check::VolumeDown),
    ("arch", "keystone", "[0.12, 0.22, 0.4]", Check::AxisUp(2)),
    ("barrel", "staves", "20", Check::TrianglesUp),
    ("barrel", "radius", "0.31", Check::AxisUp(0)),
    ("barrel", "hoop", "0.34", Check::AxisUp(0)),
    ("bed", "frame", "[1.4, 0.2, 2.2]", Check::AxisUp(2)),
    ("bed", "mattress", "[1.5, 0.18, 1.9]", Check::AxisUp(0)),
    ("bed", "headboard", "[1.4, 0.9, 0.08]", Check::AxisUp(1)),
    ("bench", "seat", "[1.7, 0.06, 0.45]", Check::AxisUp(0)),
    ("bench", "back", "[1.4, 0.3, 0.05]", Check::AxisUp(1)),
    ("chair", "seat", "[0.55, 0.05, 0.45]", Check::AxisUp(0)),
    ("chair", "back", "[0.45, 0.62, 0.04]", Check::AxisUp(1)),
    ("column", "radius", "0.22", Check::VolumeUp),
    ("column", "top_radius", "0.15", Check::VolumeUp),
    ("column", "segments", "32", Check::TrianglesUp),
    ("crate", "body", "[0.7, 0.6, 0.6]", Check::AxisUp(0)),
    ("crate", "post", "[0.06, 0.66, 0.06]", Check::AxisUp(1)),
    ("crate", "band", "[0.7, 0.06, 0.64]", Check::AxisUp(0)),
    ("door", "swing", "[0, 45, 0]", Check::AxisUp(2)),
    ("fence", "posts", "4", Check::TrianglesUp),
    ("fence", "boards", "12", Check::AxisUp(0)),
    ("fence", "spacing", "[0.85, 0, 0]", Check::AxisUp(0)),
    ("lamp_floor", "base_radius", "0.21", Check::AxisUp(0)),
    ("lamp_floor", "pole_radius", "0.03", Check::VolumeUp),
    ("lamp_floor", "shade_radius", "0.22", Check::AxisUp(0)),
    ("pipe_run", "main_radius", "0.06", Check::VolumeUp),
    ("pipe_run", "branch_radius", "0.06", Check::VolumeUp),
    ("pipe_run", "elbow", "0.08", Check::AxisUp(1)),
    ("railing", "posts", "4", Check::TrianglesUp),
    ("railing", "spacing", "[0.6, 0, 0]", Check::AxisUp(0)),
    ("railing", "span", "[1.2, 0.06, 0.06]", Check::AxisUp(0)),
    ("roof_gable", "section", "[[-1.8, 0], [1.8, 0], [0, 1.05]]", Check::AxisUp(0)),
    ("roof_gable", "length", "4.8", Check::AxisUp(2)),
    ("roof_gable", "ridge", "[0.14, 0.2, 4.5]", Check::AxisUp(1)),
    ("shelf", "wood_color", "[0.9, 0.2, 0.2]", Check::Color),
    ("shelf", "book_color", "[0.9, 0.2, 0.2]", Check::Color),
    ("sign_post", "post", "[0.08, 2.2, 0.08]", Check::AxisUp(1)),
    ("sign_post", "sign", "[1.0, 0.4, 0.04]", Check::AxisUp(0)),
    ("stairs", "steps", "8", Check::TrianglesUp),
    ("stairs", "size", "[1.0, 0.22, 0.42]", Check::AxisUp(1)),
    ("table", "top", "[1.4, 0.05, 0.7]", Check::AxisUp(0)),
    ("table", "radius", "0.05", Check::VolumeUp),
    ("table", "segments", "24", Check::TrianglesUp),
    ("adobe_hut", "plaster_color", "[0.9, 0.2, 0.2]", Check::Color),
    ("adobe_hut", "wood_color", "[0.9, 0.2, 0.2]", Check::Color),
    ("ruin_wall", "wall", "[4.4, 2.6, 0.4]", Check::AxisUp(0)),
    ("tent", "cloth_color", "[0.9, 0.2, 0.2]", Check::Color),
    ("tent", "wood_color", "[0.9, 0.2, 0.2]", Check::Color),
    ("wall_window", "wall", "[2.6, 2.6, 0.2]", Check::AxisUp(0)),
    ("well", "radius", "1.0", Check::AxisUp(0)),
    ("well", "stone_color", "[0.9, 0.2, 0.2]", Check::Color),
    ("well", "wood_color", "[0.9, 0.2, 0.2]", Check::Color),
    ("window", "frame_color", "[0.9, 0.2, 0.2]", Check::Color),
    ("window", "glass_color", "[0.9, 0.2, 0.2]", Check::Color),
];

/// The explicit cases plus the shared `color`/`roughness` parameters every kit of this shape has.
fn cases() -> Vec<Case> {
    let mut cases = CASES.to_vec();
    for kit in KITS {
        let exposed = exposes(kit.source);
        for case in [
            ("color", "[0.9, 0.2, 0.2]", Check::Color),
            ("roughness", "0.2", Check::Roughness),
        ] {
            if exposed.contains(&case.0) {
                cases.push((kit.name, case.0, case.1, case.2));
            }
        }
    }
    cases
}

#[test]
fn every_exposed_parameter_is_tested_and_changes_what_it_should() {
    let cases = cases();
    for kit in KITS {
        let exposed = exposes(kit.source);
        let tested: BTreeSet<&str> = cases
            .iter()
            .filter(|case| case.0 == kit.name)
            .map(|case| case.1)
            .collect();
        for name in &exposed {
            assert!(
                tested.contains(name) || *name == "seed" || *name == "arrangement",
                "{} exposes {name}, but no case tests it",
                kit.name
            );
        }
        for case in cases.iter().filter(|case| case.0 == kit.name) {
            assert!(
                exposed.contains(&case.1),
                "{} does not expose {}",
                kit.name,
                case.1
            );
        }
    }
    for &(kit, param, value, check) in &cases {
        let base = run(kit, "");
        let changed = run(kit, &format!("{param}={value}"));
        match check {
            Check::AxisUp(axis) => {
                let (before, after) = (extent(&base, axis), extent(&changed, axis));
                assert!(
                    after > before + 1.0e-3,
                    "{kit} {param}={value}: axis {axis} grew {before} -> {after}"
                );
            }
            Check::VolumeUp | Check::VolumeDown => {
                let (before, after) = (total_volume(&base), total_volume(&changed));
                let grew = after > before + 1.0e-9;
                let shrank = after < before - 1.0e-9;
                assert!(
                    matches!(check, Check::VolumeUp) && grew
                        || matches!(check, Check::VolumeDown) && shrank,
                    "{kit} {param}={value}: volume {before} -> {after}"
                );
            }
            Check::TrianglesUp => {
                let (before, after) = (triangles(&base), triangles(&changed));
                assert!(
                    after > before,
                    "{kit} {param}={value}: triangles {before} -> {after}"
                );
            }
            Check::Color => {
                assert_ne!(
                    material_colors(&base),
                    material_colors(&changed),
                    "{kit} {param}={value} did not change a material colour"
                );
                assert_eq!(triangles(&base), triangles(&changed), "{kit}");
            }
            Check::Roughness => {
                assert_ne!(
                    roughness_bits(&base),
                    roughness_bits(&changed),
                    "{kit} {param}={value} did not change roughness"
                );
                assert_eq!(triangles(&base), triangles(&changed), "{kit}");
            }
        }
    }
}

#[test]
fn exposed_seeds_change_the_layout_without_changing_the_volume() {
    for kit in KITS {
        for name in exposes(kit.source) {
            if name != "seed" && name != "arrangement" {
                continue;
            }
            let (first, second) = (
                run(kit.name, &format!("{name}=4")),
                run(kit.name, &format!("{name}=5")),
            );
            assert_ne!(positions(&first), positions(&second), "{} {name}", kit.name);
            let difference = (total_volume(&first) - total_volume(&second)).abs();
            assert!(
                difference < 1.0e-3,
                "{} {name}: volume differs by {difference}",
                kit.name
            );
        }
    }
}
