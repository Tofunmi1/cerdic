//! A labelled mock provider: deterministic, offline, understands two requests.

use crate::{Patch, Provider, ProviderError, Request};
use serde_json::json;
use ve_graph::{Command, Document, NodeRef, Value, builtin_registry, parse_text};

/// The frozen vase fixture, written in the text form.
pub const VASE_TEXT: &str = "\
outer = profile.polygon@1(points=[[0, 0], [0.06, 0], [0.11, 0.12], [0.07, 0.26], [0.05, 0.3], [0, 0.3]])
inner = profile.polygon@1(points=[[0, 0.01], [0.056, 0.01], [0.104, 0.12], [0.064, 0.26], [0.04, 0.31], [0, 0.31]])
wall = check.min_wall@1(outer, inner, min=3mm)
body = solid.revolve@1(outer, angle=360deg, segments=64)
cavity = solid.revolve@1(inner, angle=360deg, segments=64)
vase = solid.difference@1(body, cavity)
output vase
";

/// Answers "make a hollow vase" and "widen its opening"; refuses everything else.
#[derive(Clone, Copy, Debug, Default)]
pub struct MockProvider;

fn labelled<'a>(doc: &'a Document, label: &str) -> Option<&'a ve_graph::Node> {
    doc.nodes.iter().find(|n| n.label.as_deref() == Some(label))
}

/// Moves the last off-axis point of a profile outward by `by` metres.
fn widened(doc: &Document, label: &str, by: f64) -> Option<Command> {
    let node = labelled(doc, label)?;
    let Value::List(points) = node.params.get("points")? else {
        return None;
    };
    let mut points = points.clone();
    let index = points.len().checked_sub(2)?;
    let Value::List(point) = points.get(index)?.clone() else {
        return None;
    };
    let (Some(x), Some(y)) = (point.first()?.as_f64(), point.get(1).cloned()) else {
        return None;
    };
    *points.get_mut(index)? = Value::List(vec![Value::Num(x + by), y]);
    Some(Command::SetParam {
        node: NodeRef::Id(node.id.clone()),
        param: "points".to_owned(),
        value: Value::List(points),
    })
}

fn reply(request: &Request, summary: &str, commands: &[Command]) -> Result<String, ProviderError> {
    let patch = Patch {
        summary: summary.to_owned(),
        base_revision: request.revision,
        commands: commands.to_vec(),
    };
    serde_json::to_string(&json!(patch)).map_err(|e| ProviderError::Failed(e.to_string()))
}

impl Provider for MockProvider {
    fn name(&self) -> &'static str {
        "mock (offline, scripted)"
    }

    fn propose(&mut self, request: &Request) -> Result<String, ProviderError> {
        let prompt = request.prompt.to_lowercase();
        if prompt.contains("vase") && prompt.contains("make") {
            let batch = parse_text(&builtin_registry(), VASE_TEXT)
                .map_err(|e| ProviderError::Failed(format!("{e:?}")))?;
            return reply(request, "make a hollow vase", &batch.commands);
        }
        if prompt.contains("widen") && prompt.contains("opening") {
            let commands: Option<Vec<_>> = [
                widened(&request.document, "outer", 0.02),
                widened(&request.document, "inner", 0.024),
            ]
            .into_iter()
            .collect();
            return match commands {
                Some(commands) => reply(request, "widen the opening", &commands),
                None => Err(ProviderError::Unsupported(
                    "there is no outer and inner profile to widen".to_owned(),
                )),
            };
        }
        Err(ProviderError::Unsupported(
            "the mock understands only: make a hollow vase, widen its opening".to_owned(),
        ))
    }
}
