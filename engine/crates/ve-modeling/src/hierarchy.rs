//! `anim.parent`: a part attached to another part moves with it (ADR 0028). The node records the parent's
//! output name; after evaluation each name is resolved against the parts, and a name that matches nothing,
//! or a chain that loops or runs too deep, is dropped with a warning so the viewer and the exporter only
//! ever see sound hierarchies.
use std::{collections::HashMap, rc::Rc};

use ve_graph::{Node, Value as Param};

use crate::{EvaluationWarning, Fail, Part, cache::Value, fail};

/// Longest chain of parents a part may have.
pub const MAX_DEPTH: usize = 8;

/// Records the node's parent name on the input.
pub(crate) fn parent(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "anim.parent needs a shape"))?;
    let Some(Param::Str(name)) = node.params.get("parent") else {
        return Err(fail("param.type", "anim.parent needs a parent name"));
    };
    let name = name.trim();
    if name.is_empty() {
        return Err(fail(
            "param.range",
            "anim.parent needs the output name of the part to follow",
        ));
    }
    let mut value = (**input).clone();
    value.parent = Some(name.to_owned());
    Ok(value)
}

fn warning(part: &Part, code: &str, message: String) -> EvaluationWarning {
    EvaluationWarning {
        code: code.to_owned(),
        node: part.node.to_string(),
        message,
    }
}

/// Drops parents that name no part, loop, or run deeper than [`MAX_DEPTH`], and says so.
pub(crate) fn resolve(parts: &mut [Part]) -> Vec<EvaluationWarning> {
    let mut warnings = Vec::new();
    let index: HashMap<String, usize> = parts
        .iter()
        .enumerate()
        .filter(|(_, part)| part.transforms.is_none())
        .map(|(i, part)| (part.name.clone(), i))
        .collect();
    for part in parts.iter_mut() {
        if let Some(name) = part.parent.clone()
            && !index.contains_key(&name)
        {
            warnings.push(warning(
                part,
                "anim.parent_missing",
                format!(
                    "{} follows \"{name}\", but no output part has that name; it stays where it is",
                    part.name
                ),
            ));
            part.parent = None;
        }
    }
    let parents: Vec<Option<usize>> = parts
        .iter()
        .map(|part| {
            part.parent
                .as_ref()
                .and_then(|name| index.get(name).copied())
        })
        .collect();
    let mut dropped = Vec::new();
    for start in 0..parts.len() {
        let (mut at, mut depth) = (start, 0);
        let mut seen = vec![start];
        while let Some(next) = parents[at] {
            depth += 1;
            at = next;
            if seen.contains(&at) || depth > MAX_DEPTH {
                dropped.push((start, seen.contains(&at)));
                break;
            }
            seen.push(at);
        }
    }
    for (i, looped) in dropped {
        let (code, why) = if looped {
            ("anim.parent_cycle", "its parents loop".to_owned())
        } else {
            (
                "anim.parent_too_deep",
                format!("its chain of parents is longer than {MAX_DEPTH}"),
            )
        };
        warnings.push(warning(
            &parts[i],
            code,
            format!("{} cannot follow its parent: {why}", parts[i].name),
        ));
        parts[i].parent = None;
    }
    warnings
}
