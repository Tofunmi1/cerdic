//! Behavioral evidence for UV packing and progressive mesh LOD.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "valid fixtures are unwrapped for concise diagnostic assertions"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(text: &str) -> Result<Evaluation, (String, String)> {
    let batch = parse_text(&builtin_registry(), text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_uv_lod").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message))
}

fn ok(text: &str) -> Evaluation {
    evaluate(text).unwrap_or_else(|(code, message)| panic!("{code}: {message}"))
}

const BOX: &str = "box = solid.box@1(size=[1,1,1])\ntris = poly.from_solid@1(box)\nmesh = poly.merge_coplanar@1(tris)\n";
const UNWRAPPED: &str = "uv = poly.unwrap_uv@1(mesh)\n";
const SPHERE: &str =
    "ball = solid.sphere@1(radius=0.5, segments=48)\nmesh = poly.from_solid@1(ball)\n";

fn packed(extra: &str) -> String {
    format!(
        "{BOX}{UNWRAPPED}packed = poly.pack_uv@1(uv, {extra})\nsolid = poly.to_solid@1(packed)\noutput solid"
    )
}

#[test]
fn packed_uvs_are_in_unit_square() {
    let p = &ok(&packed("margin=0.02")).parts[0];
    let min = p
        .uvs
        .iter()
        .flatten()
        .copied()
        .fold(f32::INFINITY, f32::min);
    let max = p
        .uvs
        .iter()
        .flatten()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(min >= 0.0 && max <= 1.0, "min={min} max={max}");
}
#[test]
fn packing_is_bit_deterministic() {
    let a = &ok(&packed("margin=0.02")).parts[0].uvs;
    let b = &ok(&packed("margin=0.02")).parts[0].uvs;
    let aa: Vec<_> = a.iter().flatten().map(|v| v.to_bits()).collect();
    let bb: Vec<_> = b.iter().flatten().map(|v| v.to_bits()).collect();
    assert_eq!(aa, bb, "UV bits length={} vs {}", aa.len(), bb.len());
}
#[test]
fn packing_without_uvs_suggests_unwrap() {
    let err = evaluate(&format!(
        "{BOX}packed = poly.pack_uv@1(mesh)\nsolid = poly.to_solid@1(packed)\noutput solid"
    ))
    .unwrap_err();
    assert!(err.1.contains("poly.unwrap_uv"), "{}: {}", err.0, err.1);
}

fn lod(level: u32, ratio: f64) -> Evaluation {
    ok(&format!(
        "{SPHERE}reduced = poly.lod@1(mesh, levels=2, level={level}, ratio={ratio})\nsolid = poly.to_solid@1(reduced)\noutput solid"
    ))
}
#[test]
fn lod_triangle_counts_follow_requested_ratios() {
    let a = lod(0, 0.6).parts[0].indices.len() / 3;
    let b = lod(1, 0.6).parts[0].indices.len() / 3;
    let c = lod(2, 0.6).parts[0].indices.len() / 3;
    assert!(
        (b as f64 / a as f64 - 0.6).abs() < 0.1 && (c as f64 / a as f64 - 0.36).abs() < 0.1,
        "input={a} level1={b} level2={c}"
    );
}
#[test]
fn lod_bounding_boxes_stay_close() {
    let a = &lod(0, 0.6).parts[0].positions;
    let amin = (0..3)
        .map(|i| a.iter().map(|p| p[i]).fold(f32::INFINITY, f32::min))
        .collect::<Vec<_>>();
    let amax = (0..3)
        .map(|i| a.iter().map(|p| p[i]).fold(f32::NEG_INFINITY, f32::max))
        .collect::<Vec<_>>();
    for level in 1..=2 {
        let p = &lod(level, 0.6).parts[0].positions;
        for i in 0..3 {
            let lo = p.iter().map(|v| v[i]).fold(f32::INFINITY, f32::min);
            let hi = p.iter().map(|v| v[i]).fold(f32::NEG_INFINITY, f32::max);
            assert!(
                (lo - amin[i]).abs() < 0.02 && (hi - amax[i]).abs() < 0.02,
                "level={level} axis={i} input=[{},{}] output=[{lo},{hi}]",
                amin[i],
                amax[i]
            );
        }
    }
}
#[test]
fn lod_outputs_convert_to_nonzero_solids() {
    for level in 0..=2 {
        let p = &lod(level, 0.6).parts[0];
        let volume = p.volume;
        assert!(
            volume > 0.0,
            "level={level} triangles={} volume={volume}",
            p.indices.len() / 3
        );
    }
}
#[test]
fn lod_repeated_evaluation_is_deterministic() {
    let a = &lod(2, 0.6).parts[0];
    let b = &lod(2, 0.6).parts[0];
    let ac = a.indices.len() / 3;
    let bc = b.indices.len() / 3;
    assert_eq!(
        ac, bc,
        "triangles {ac} vs {bc}; volumes {} vs {}",
        a.volume, b.volume
    );
    assert!(
        (a.volume - b.volume).abs() <= 1.0e-12,
        "triangles={ac} volume {} vs {}",
        a.volume,
        b.volume
    );
}
#[test]
fn lod_uv_seam_protection_refuses_with_suggestion() {
    let e=evaluate(&format!("{SPHERE}reduced = poly.lod@1(mesh, levels=2, level=1, ratio=0.6, protect_uv_seams=true)\nsolid = poly.to_solid@1(reduced)\noutput solid")).unwrap_err();
    assert_eq!(e.0, "poly.lod_protection_unsupported");
    assert!(
        e.1.contains("disable") || e.1.contains("decimator"),
        "{}",
        e.1
    );
}
#[test]
fn lod_refuses_collapse() {
    let e=evaluate(&format!("{SPHERE}reduced = poly.lod@1(mesh, levels=4, level=4, ratio=0.05)\nsolid = poly.to_solid@1(reduced)\noutput solid")).unwrap_err();
    assert_eq!(e.0, "poly.lod_collapse");
    assert!(e.1.contains("ratio"), "{}", e.1);
}
