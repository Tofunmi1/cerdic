//! `terrain.heightfield` and `inst.drop`: bounds, volumes, determinism, and copies landing on the ground.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate_at(text: &str, quality: Quality) -> Result<Evaluation, String> {
    let batch = parse_text(&builtin_registry(), text).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_terrain").unwrap(),
        label: None,
        commands: batch.commands,
    });
    if let Outcome::Rejected { .. } = outcome {
        return Err(format!("{outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), quality)
        .map_err(|e| format!("{}: {}", e.code, e.message))
}

fn evaluate(text: &str) -> Result<Evaluation, String> {
    evaluate_at(text, Quality::Final)
}

fn ground(extra: &str) -> Evaluation {
    evaluate(&format!(
        "land = terrain.heightfield@1(size=[10, 10], resolution=32{extra})\noutput land = land\n"
    ))
    .unwrap()
}

fn y_range(e: &Evaluation) -> (f32, f32) {
    e.parts[0]
        .positions
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), p| {
            (lo.min(p[1]), hi.max(p[1]))
        })
}

#[test]
fn flat_ground_is_a_slab_and_hills_stay_inside_their_height() {
    let flat = ground(", height=0, base=0.5");
    assert!(
        (flat.parts[0].volume - 50.0).abs() < 1e-3,
        "a flat slab 10 x 10 x 0.5: {}",
        flat.parts[0].volume
    );
    let hills = ground(", height=3, base=0.5, seed=4");
    let (low, high) = y_range(&hills);
    assert!((low + 0.5).abs() < 1e-5, "the base is at -0.5: {low}");
    assert!(
        high <= 3.0 + 1e-4 && high > 1.0,
        "tallest ground is within the height and the land is not flat: {high}"
    );
    let volume = hills.parts[0].volume;
    assert!(volume > 50.0 && volume < 100.0 * 3.5, "{volume}");
}

#[test]
fn the_same_seed_gives_the_same_land_and_another_seed_a_different_one() {
    let (a, b, c) = (ground(", seed=7"), ground(", seed=7"), ground(", seed=8"));
    assert_eq!(a.parts[0].positions, b.parts[0].positions);
    assert_ne!(a.parts[0].positions, c.parts[0].positions);
    let ridged = ground(", seed=7, style=\"ridged\"");
    assert_ne!(a.parts[0].positions, ridged.parts[0].positions);
}

#[test]
fn edge_falloff_makes_an_island_and_flat_below_makes_lake_beds() {
    let island = ground(", height=4, seed=3, edge_falloff=0.6");
    let edge_heights: Vec<f32> = island.parts[0]
        .positions
        .iter()
        .filter(|p| p[0].abs() > 4.999 || p[2].abs() > 4.999)
        .filter(|p| p[1] > -0.4)
        .map(|p| p[1])
        .collect();
    assert!(
        !edge_heights.is_empty() && edge_heights.iter().all(|y| y.abs() < 1e-4),
        "the edge is at sea level: {edge_heights:?}"
    );
    let beds = ground(", height=4, seed=3, flat_below=0.5");
    let lowest_ground = beds.parts[0]
        .positions
        .iter()
        .filter(|p| p[1] > -0.4)
        .map(|p| p[1])
        .fold(f32::MAX, f32::min);
    assert!(
        lowest_ground >= 2.0 - 1e-4,
        "nothing is below the lake bed: {lowest_ground}"
    );
}

const SCENE: &str = "land = terrain.heightfield@1(size=[10, 10], height=3, resolution=48, seed=2)\n\
    tree = solid.cylinder@1(radius=0.1, height=1, segments=8)\n\
    crowd = inst.scatter@1(tree, count=60, width=12, depth=12, min_spacing=0.2, seed=5)\n";

fn translations(e: &Evaluation) -> Vec<[f32; 3]> {
    let part = e.parts.iter().find(|p| p.name == "trees").unwrap();
    part.transforms
        .as_ref()
        .unwrap()
        .iter()
        .map(|m| [m[12], m[13], m[14]])
        .collect()
}

#[test]
fn dropped_copies_sit_on_the_ground_and_those_off_the_map_are_removed() {
    let plain = evaluate(&format!(
        "{SCENE}placed = inst.drop@1(crowd, land)\noutput trees = placed\noutput land = land\n"
    ))
    .unwrap();
    let spots = translations(&plain);
    assert!(
        !spots.is_empty() && spots.len() < 60,
        "some of the 60 fell outside the 10 m map: {}",
        spots.len()
    );
    assert!(
        spots
            .iter()
            .all(|p| p[0].abs() <= 5.0 + 1e-4 && p[2].abs() <= 5.0 + 1e-4),
        "all are over the ground"
    );
    assert!(
        spots.iter().all(|p| (0.0..=3.0 + 1e-4).contains(&p[1])),
        "all stand at ground height"
    );
    assert!(spots.iter().any(|p| p[1] > 0.3), "and the land is not flat");
    let raised = evaluate(&format!("{SCENE}placed = inst.drop@1(crowd, land, offset=0.25)\noutput trees = placed\noutput land = land\n")).unwrap();
    for (a, b) in spots.iter().zip(translations(&raised)) {
        assert!(
            (b[1] - a[1] - 0.25).abs() < 1e-4,
            "offset lifts every copy by the same amount"
        );
    }
    let kept = evaluate(&format!("{SCENE}placed = inst.drop@1(crowd, land, outside=\"keep\")\noutput trees = placed\noutput land = land\n")).unwrap();
    assert_eq!(translations(&kept).len(), 60);
}

#[test]
fn align_tilts_copies_on_slopes_and_a_miss_everywhere_is_refused() {
    let tilted = evaluate(&format!("{SCENE}placed = inst.drop@1(crowd, land, align=1)\noutput trees = placed\noutput land = land\n")).unwrap();
    let part = tilted.parts.iter().find(|p| p.name == "trees").unwrap();
    let leaning = part
        .transforms
        .as_ref()
        .unwrap()
        .iter()
        .filter(|m| (m[5] - 1.0).abs() > 0.01)
        .count();
    assert!(leaning > 0, "on slopes at least some copies lean");
    let nowhere = "land = terrain.heightfield@1(size=[2, 2], resolution=8)\ntree = solid.box@1(size=[0.1, 0.1, 0.1])\n\
        row = inst.repeat@1(tree, count=3, step=[1, 0, 0])\nfar = xform.move@1(row, by=[50, 0, 50])\n\
        placed = inst.drop@1(far, land)\noutput trees = placed\noutput land = land\n";
    let error = evaluate(nowhere).unwrap_err();
    assert!(error.contains("none of the"), "{error}");
}

/// The top of the slab is wound to face up, so it renders lit and not as a back face.
#[test]
fn the_landscape_surface_faces_outward_and_its_top_faces_up() {
    let land = evaluate("land = terrain.heightfield@1(size=[20, 20], height=3, resolution=32, seed=2)\noutput land = land\n").unwrap();
    let part = &land.parts[0];
    let mut centre = [0.0_f32; 3];
    for p in &part.positions {
        for axis in 0..3 {
            centre[axis] += p[axis] / part.positions.len() as f32;
        }
    }
    let (mut up_top, mut top) = (0, 0);
    for tri in part.indices.chunks(3) {
        let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| part.positions[i as usize]);
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        let mid = [
            (a[0] + b[0] + c[0]) / 3.0 - centre[0],
            (a[1] + b[1] + c[1]) / 3.0 - centre[1],
            (a[2] + b[2] + c[2]) / 3.0 - centre[2],
        ];
        if mid[1] > 0.0 && n[1].abs() > 0.5 * (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() {
            top += 1;
            if n[1] > 0.0 {
                up_top += 1;
            }
        }
    }
    assert!(top > 100, "the top has triangles: {top}");
    assert_eq!(up_top, top, "top triangles face up");
}

/// The drop only reads its surface, so the planner must charge a placed copy the prototype's triangles, not the ground's:
/// twenty small trees on a 33,000-triangle landscape fit the 200,000-triangle preview budget.
#[test]
fn dropping_onto_a_big_landscape_is_not_charged_the_landscape_per_copy() {
    let program = "land = terrain.heightfield@1(size=[60, 60], height=3, resolution=128, seed=2)\n\
        tree = solid.cylinder@1(radius=0.1, height=1, segments=8)\n\
        crowd = inst.scatter@1(tree, count=20, width=50, depth=50, min_spacing=1, seed=5)\n\
        placed = inst.drop@1(crowd, land)\noutput trees = placed\noutput land = land\n";
    let result = evaluate_at(program, Quality::Preview).unwrap();
    assert_eq!(result.parts.len(), 2);
}
