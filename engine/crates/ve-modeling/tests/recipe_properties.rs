//! G2: property checks for every recipe (group kit and plant kit) with its default arguments: it evaluates without warnings, has geometry within a triangle budget, measures what its own documentation
//! says (the `A x B x C m` footprint), stands where it says its origin is, and evaluates to the same bits twice. A kit whose text or documentation drifts fails here by name. The golden pictures of the
//! same recipes are `web/tests/g2-recipe-goldens.mjs`.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    reason = "fixture failures include the diagnostic rather than hiding it"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, KITS, Kit, PLANT_KITS, Quality};

fn evaluate(kit: &Kit) -> Result<Evaluation, String> {
    let source = format!("{}\nitem = {}()\noutput item", kit.source, kit.name);
    let parsed = parse_text(&builtin_registry(), &source).map_err(|e| format!("parse: {e:?}"))?;
    let mut session = Session::with_builtin(2201);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_recipe").unwrap(),
        label: None,
        commands: parsed.commands,
    });
    if !matches!(outcome, Outcome::Committed { .. }) {
        return Err(format!("{outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{}: {}", e.code, e.message))
}

/// The first `A x B x C m` in the documentation, as numbers.
fn documented_size(doc: &str) -> Option<[f32; 3]> {
    let words: Vec<&str> = doc.split_whitespace().collect();
    for i in 0..words.len().saturating_sub(5) {
        if words[i + 1] == "x"
            && words[i + 3] == "x"
            && words[i + 5].trim_end_matches([',', ';', ':', '.']) == "m"
        {
            let n = |s: &str| s.trim_end_matches(',').parse::<f32>().ok();
            if let (Some(a), Some(b), Some(c)) = (n(words[i]), n(words[i + 2]), n(words[i + 4])) {
                return Some([a, b, c]);
            }
        }
    }
    None
}

fn bounds(result: &Evaluation) -> Option<([f32; 3], [f32; 3])> {
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    let mut any = false;
    for part in &result.parts {
        for p in &part.positions {
            any = true;
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
    }
    any.then_some((lo, hi))
}

fn triangles(result: &Evaluation) -> usize {
    result.parts.iter().map(|p| p.indices.len() / 3).sum()
}

fn fingerprint(result: &Evaluation) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for part in &result.parts {
        for v in part.positions.iter().flatten() {
            h = (h ^ u64::from(v.to_bits())).wrapping_mul(0x0100_0000_01b3);
        }
        for i in &part.indices {
            h = (h ^ u64::from(*i)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

#[test]
fn every_recipe_evaluates_cleanly_within_its_documented_size_and_budget() {
    let mut problems = Vec::new();
    let mut checked = 0;
    for (kit, plant) in KITS
        .iter()
        .map(|k| (k, false))
        .chain(PLANT_KITS.iter().map(|k| (k, true)))
    {
        let result = match evaluate(kit) {
            Ok(r) => r,
            Err(e) => {
                problems.push(format!("{}: does not evaluate: {e}", kit.name));
                continue;
            }
        };
        checked += 1;
        if !result.warnings.is_empty() {
            problems.push(format!(
                "{}: warnings {:?}",
                kit.name,
                result
                    .warnings
                    .iter()
                    .map(|w| w.code.clone())
                    .collect::<Vec<_>>()
            ));
        }
        let tris = triangles(&result);
        let budget = if plant { 12_000 } else { 40_000 };
        if tris < 12 || tris > budget {
            problems.push(format!(
                "{}: {tris} triangles, expected 12 to {budget}",
                kit.name
            ));
        }
        let Some((lo, hi)) = bounds(&result) else {
            problems.push(format!("{}: no geometry", kit.name));
            continue;
        };
        let size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
        eprintln!(
            "RECIPE {:<12} {tris:>6} tris  {:.2} x {:.2} x {:.2} m  min y {:.2}  documented {:?}",
            kit.name,
            size[0],
            size[1],
            size[2],
            lo[1],
            documented_size(kit.doc)
        );
        if let Some(doc) = documented_size(kit.doc) {
            // Plants are organic ("the footprint is a guide, not a contract"); the rest should be close to their stated size.
            let tolerance = if plant { 0.35 } else { 0.12 };
            for (axis, name) in ["wide", "tall", "deep"].iter().enumerate() {
                let allowed = doc[axis] * tolerance + 0.06;
                if (size[axis] - doc[axis]).abs() > allowed {
                    problems.push(format!(
                        "{}: documented {} m {name} but measures {:.2} m",
                        kit.name, doc[axis], size[axis]
                    ));
                }
            }
        }
        // The origin is at the floor (or the ground, or the foot of the trunk): nothing stands below it, and the recipe does not float above it.
        let on_floor = kit.doc.contains("origin at floor")
            || kit.doc.contains("origin at the ground")
            || kit.doc.contains("origin at the foot");
        if on_floor && (lo[1].abs() > 0.06) {
            problems.push(format!("{}: its documentation says the origin is at the floor but its lowest point is at y {:.3}", kit.name, lo[1]));
        }
        if fingerprint(&result) != fingerprint(&evaluate(kit).unwrap()) {
            problems.push(format!("{}: two evaluations differ", kit.name));
        }
    }
    assert!(checked >= 27, "the sweep covered {checked} recipes");
    assert!(
        problems.is_empty(),
        "{} problem(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
}
