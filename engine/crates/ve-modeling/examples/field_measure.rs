//! Release measurements for the production field adapter; run as `field_measure sphere|creature`.
#![expect(
    clippy::print_stdout,
    reason = "prints one machine-readable measurement line"
)]

use std::time::Instant;
use ve_graph::{Outcome, Session};
use ve_modeling::{Evaluator, Quality};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let name = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "creature".to_owned());
    let source = match name.as_str() {
        "sphere" => {
            "field = field.sphere@1(radius=0.5)\n\
            solid = field.to_solid@1(field, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.02)\n\
            output solid"
        }
        "creature" => include_str!("../tests/gallery/field_creature.txt"),
        _ => return Err(format!("unknown measurement case {name:?}").into()),
    };
    let parsed = ve_graph::parse_text(&ve_graph::builtin_registry(), source)
        .map_err(|errors| format!("fixture parse failed: {errors:?}"))?;
    let mut session = Session::with_builtin(9022);
    let batch = serde_json::json!({
        "doc": session.document().id,
        "base_revision": 0,
        "transaction": "t_field_measure",
        "commands": parsed.commands,
    });
    if !matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ) {
        return Err("field measurement fixture was rejected".into());
    }
    let started = Instant::now();
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    let triangles = result
        .parts
        .iter()
        .map(|part| part.indices.len() / 3)
        .sum::<usize>();
    println!(
        "{}",
        serde_json::json!({
            "case": name,
            "quality": "final",
            "elapsed_ms": started.elapsed().as_secs_f64() * 1000.0,
            "triangles": triangles,
            "volume_m3": result.parts.iter().map(|part| part.volume).sum::<f64>(),
        })
    );
    Ok(())
}
