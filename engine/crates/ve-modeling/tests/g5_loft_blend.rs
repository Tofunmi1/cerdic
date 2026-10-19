//! Smooth blending between loft sections (`steps` above 1).
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn evaluate(text: &str) -> Result<ve_modeling::Evaluation, String> {
    evaluate_at(text, Quality::Final)
}

fn evaluate_at(text: &str, quality: Quality) -> Result<ve_modeling::Evaluation, String> {
    let batch = parse_text(&builtin_registry(), text).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_loft_blend").map_err(|e| format!("{e:?}"))?,
        label: None,
        commands: batch.commands,
    });
    if !matches!(outcome, Outcome::Committed { .. }) {
        return Err(format!("not committed: {outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), quality)
        .map_err(|e| format!("{}: {}", e.code, e.message))
}

fn vase(steps: u32) -> String {
    format!(
        "base = profile.circle@1(radius=0.03, segments=48)\n\
         belly = profile.circle@1(radius=0.11, segments=48)\n\
         neck = profile.circle@1(radius=0.035, segments=48)\n\
         rim = profile.circle@1(radius=0.055, segments=48)\n\
         body = solid.loft@1(base, belly, neck, rim, spacing=0.1, steps={steps})\n\
         output body"
    )
}

#[test]
fn g5_20_one_step_is_the_unchanged_straight_loft() {
    let straight = evaluate(&vase(1)).unwrap();
    let explicit = evaluate(
        "base = profile.circle@1(radius=0.03, segments=48)\n\
         belly = profile.circle@1(radius=0.11, segments=48)\n\
         neck = profile.circle@1(radius=0.035, segments=48)\n\
         rim = profile.circle@1(radius=0.055, segments=48)\n\
         body = solid.loft@1(base, belly, neck, rim, spacing=0.1)\n\
         output body",
    )
    .unwrap();
    assert!((straight.parts[0].volume - explicit.parts[0].volume).abs() < 1.0e-15);
    assert_eq!(straight.parts[0].positions, explicit.parts[0].positions);
}

#[test]
fn g5_21_blended_loft_is_smooth_closed_and_keeps_its_height() {
    let straight = evaluate(&vase(1)).unwrap();
    let smooth = evaluate(&vase(6)).unwrap();
    let (a, b) = (&straight.parts[0], &smooth.parts[0]);
    assert!(
        b.indices.len() > 4 * a.indices.len(),
        "more rows, more triangles"
    );
    let height = |part: &ve_modeling::Part| {
        part.positions
            .iter()
            .map(|p| f64::from(p[1]))
            .fold(f64::MIN, f64::max)
    };
    assert!((height(a) - height(b)).abs() < 1.0e-6, "same total height");
    let widest = b
        .positions
        .iter()
        .map(|p| f64::from(p[0]).hypot(f64::from(p[2])))
        .fold(0.0_f64, f64::max);
    assert!(
        (0.108..0.125).contains(&widest),
        "the belly passes through its section and overshoots only slightly: {widest}"
    );
    assert!(
        (b.volume - a.volume).abs() < 0.25 * a.volume,
        "blended {} versus straight {}",
        b.volume,
        a.volume
    );
    assert!(b.volume.is_finite() && b.volume > 0.0);
}

#[test]
fn g5_18_blended_loft_cost_is_estimated_before_building() {
    use std::fmt::Write;
    let mut text = String::new();
    for index in 0..64 {
        writeln!(
            text,
            "s{index} = profile.circle@1(radius=0.1, segments=256)"
        )
        .unwrap();
    }
    let names = (0..64)
        .map(|i| format!("s{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    write!(
        text,
        "body = solid.loft@1({names}, spacing=0.01, steps=8)\noutput body"
    )
    .unwrap();
    // 63 spans x 8 rows x 256 sides x 2 is about 258,000 triangles, over the 200,000 Preview budget;
    // without the blend rows the same loft is 32,000.
    let error = evaluate_at(&text, Quality::Preview).unwrap_err();
    assert!(error.contains("budget.triangles"), "{error}");
    let straight = text.replace("steps=8", "steps=1");
    assert!(evaluate_at(&straight, Quality::Preview).is_ok());
}
