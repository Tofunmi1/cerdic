//! The production evaluator runs the same primitive/boolean graph natively and in WASI.
#![expect(clippy::print_stdout, reason = "parity corpus output")]

use ve_graph::{Outcome, Session};
use ve_modeling::{Evaluator, Quality};

fn text_case(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let parsed = ve_graph::parse_text(&ve_graph::builtin_registry(), source)
        .map_err(|error| format!("{error:?}"))?;
    let mut session = Session::with_builtin(1);
    let batch = serde_json::json!({"doc":session.document().id,"base_revision":0,"transaction":"t_instances","commands":parsed.commands});
    if !matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ) {
        return Err("instance parity fixture rejected".into());
    }
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .map_err(|error| error.message)?;
    for part in result.parts {
        println!(
            "{}",
            serde_json::json!({"node":part.node,"volume":part.volume,
            "positions":part.positions,"indices":part.indices,"transforms":part.transforms,"instance_indices":part.instance_indices})
        );
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    text_case(
        "b = solid.box(size=[1,1,1])\ns = inst.repeat(b, count=3, step=[2,0,0])\nm = inst.mirror(s, axis=\"x\")\noutput m",
    )?;
    text_case(
        "b = solid.box(size=[2,4,6])\na = xform.align(b, from=[1,2,3], to=[-3,2,1], pivot=[2,1,0])\noutput a",
    )?;
    let mut session = Session::with_builtin(1);
    let batch = serde_json::json!({"doc":session.document().id,"base_revision":0,
        "transaction":"t_parity","commands":[
        {"add_node":{"op":"solid.box@1","params":{"size":[1,1,1]},"alias":"$a"}},
        {"add_node":{"op":"solid.box@1","params":{"size":[1,1,1]},"alias":"$b"}},
        {"add_node":{"op":"xform.move@1","params":{"by":[0.5,0,0]},"alias":"$move"}},
        {"add_node":{"op":"solid.difference@1","alias":"$cut"}},
        {"connect":{"from":{"node":"$b","port":"out"},"to":{"node":"$move","port":"in"}}},
        {"connect":{"from":{"node":"$a","port":"out"},"to":{"node":"$cut","port":"in"}}},
        {"connect":{"from":{"node":"$move","port":"out"},"to":{"node":"$cut","port":"in"}}},
        {"set_output":{"name":"cut","from":{"node":"$cut","port":"out"}}}
    ]});
    if !matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ) {
        return Err("parity fixture rejected".into());
    }
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    for part in result.parts {
        if (part.volume - 0.5).abs() > 1e-6 {
            return Err("analytic boolean volume mismatch".into());
        }
        println!(
            "{}",
            serde_json::json!({"node":part.node,"volume":part.volume,
            "positions":part.positions,"indices":part.indices})
        );
    }
    Ok(())
}
