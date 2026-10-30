//! `sign.text`: a lit sign. Text in the 5 by 7 pixel font becomes a block of small emissive boxes, centred, standing on y=0 and facing +z: the
//! neon and LED signs that make a night street read as cyberpunk. The letters are real geometry, so bloom and the point lights can use them.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(14);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_sign").unwrap(),
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

fn sign(params: &str) -> Result<Evaluation, String> {
    evaluate(&format!("s = sign.text@1({params})\noutput s = s\n"))
}

fn bounds(evaluation: &Evaluation) -> ([f32; 3], [f32; 3]) {
    let mut lo = [f32::MAX; 3];
    let mut hi = [f32::MIN; 3];
    for p in &evaluation.parts[0].positions {
        for axis in 0..3 {
            lo[axis] = lo[axis].min(p[axis]);
            hi[axis] = hi[axis].max(p[axis]);
        }
    }
    (lo, hi)
}

const E: f32 = 1e-4;

#[test]
fn the_sign_is_centred_stands_on_the_floor_and_faces_forward() {
    // Height 0.7 m is seven pixels of 0.1 m; four letters span 4 * 6 - 1 = 23 pixels = 2.3 m.
    let ev = sign("text=\"OPEN\", height=0.7, depth=0.05").unwrap();
    let (lo, hi) = bounds(&ev);
    assert!(
        (lo[0] + 1.15).abs() < E && (hi[0] - 1.15).abs() < E,
        "x {} .. {}",
        lo[0],
        hi[0]
    );
    assert!(
        lo[1].abs() < E && hi[1] <= 0.7 + E,
        "y {} .. {}",
        lo[1],
        hi[1]
    );
    assert!(
        lo[2].abs() < E && (hi[2] - 0.05).abs() < E,
        "z {} .. {}",
        lo[2],
        hi[2]
    );
}

#[test]
fn the_volume_is_the_lit_pixels_times_their_depth() {
    let ev = sign("text=\"OPEN\", height=0.7, depth=0.05").unwrap();
    let lit = ve_world::pixel_text::layout("OPEN", 0.1).rects.len() as f64;
    let expected = lit * 0.1 * 0.1 * 0.05;
    let volume = ev.parts[0].volume;
    assert!(
        (volume - expected).abs() < expected * 1e-3,
        "volume {volume}, expected {expected}"
    );
}

#[test]
fn different_text_is_different_geometry_and_the_same_text_repeats() {
    let a = sign("text=\"NOODLES\"").unwrap();
    let b = sign("text=\"NOODLES\"").unwrap();
    let c = sign("text=\"RAMEN\"").unwrap();
    assert_eq!(a.parts[0].positions, b.parts[0].positions);
    assert_ne!(a.parts[0].positions, c.parts[0].positions);
}

#[test]
fn the_sign_glows_in_its_colour() {
    let ev = sign("text=\"BAR\", color=[1, 0.1, 0.6], glow=3").unwrap();
    let material = ev.parts[0].material.as_ref().unwrap();
    let emissive_peak = material.emissive.iter().copied().fold(0.0_f32, f32::max);
    assert!(
        emissive_peak > 1.0,
        "glow 3 of a bright colour is brighter than white: {emissive_peak}"
    );
    // The glow keeps the colour's hue: red the strongest, green the weakest.
    assert!(
        material.emissive[0] > material.emissive[2] && material.emissive[2] > material.emissive[1]
    );
    let dim = sign("text=\"BAR\", color=[1, 0.1, 0.6], glow=0").unwrap();
    assert!(
        dim.parts[0]
            .material
            .as_ref()
            .unwrap()
            .emissive
            .iter()
            .all(|c| *c < 1e-6),
        "glow 0 is unlit"
    );
}

#[test]
fn whole_number_colours_and_glow_work() {
    assert!(sign("text=\"A\", color=[1, 0, 0], glow=2, height=1, depth=1").is_ok());
}

#[test]
fn text_with_nothing_to_light_is_refused() {
    assert!(sign("text=\"\"").is_err(), "empty");
    assert!(sign("text=\"   \"").is_err(), "only spaces");
    assert!(
        sign(&format!("text=\"{}\"", "W".repeat(49))).is_err(),
        "more than 48 characters"
    );
}

#[test]
fn bad_sizes_are_refused() {
    assert!(sign("text=\"A\", height=0").is_err());
    assert!(sign("text=\"A\", height=11").is_err());
    assert!(sign("text=\"A\", depth=0").is_err());
    assert!(sign("text=\"A\", depth=1.5").is_err());
    assert!(sign("text=\"A\", glow=-1").is_err());
    assert!(sign("text=\"A\", glow=21").is_err());
}

#[test]
fn a_long_sign_stays_within_a_triangle_budget() {
    let ev = sign(&format!("text=\"{}\", height=1", "W".repeat(48))).unwrap();
    assert!(
        ev.parts[0].indices.len() / 3 < 40_000,
        "{} triangles",
        ev.parts[0].indices.len() / 3
    );
}

#[test]
fn a_sign_joins_a_backing_plate_and_can_be_moved() {
    let source = "s = sign.text@1(text=\"OPEN\", height=0.7, depth=0.05)\n\
                  plate0 = solid.box@1(size=[2.8, 1.0, 0.04])\n\
                  plate1 = xform.move@1(plate0, by=[0, 0.35, -0.02])\n\
                  at = xform.move@1(s, by=[10, 3, 0])\n\
                  chk = check.attached@1(s, plate1)\n\
                  output s = at\noutput plate = plate1\n";
    let ev = evaluate(source).unwrap();
    let sign_part = ev.parts.iter().find(|p| p.name == "s").unwrap();
    let min_x = sign_part
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::MAX, f32::min);
    assert!(
        (min_x - (10.0 - 1.15)).abs() < E,
        "moved sign starts at {min_x}"
    );
    assert!(
        !ev.warnings.iter().any(|w| w.code == "attach.floating"),
        "{:?}",
        ev.warnings
    );
}
