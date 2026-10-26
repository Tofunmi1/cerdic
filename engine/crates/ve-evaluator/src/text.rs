//! Stateless text operations over the page's document snapshot.
use std::collections::BTreeMap;

use serde_json::json;
use ve_graph::{Document, Output, PortRef, builtin_registry, parse_text_in, to_text_compact};

fn document(source: &str) -> Result<Document, String> {
    let mut doc: Document = serde_json::from_str(source).map_err(|error| {
        json!({"error":{"code":"parse.syntax","message":error.to_string()}}).to_string()
    })?;
    // JSON.stringify turns 1.0 into 1. Restore parameter types so an integral radian value
    // cannot be printed as a bare integer (which text would read as degrees).
    let registry = builtin_registry();
    let mut exposed = BTreeMap::new();
    for group in &doc.groups {
        for param in &group.params {
            if let Some(ty) = doc
                .node(&param.node)
                .and_then(|node| registry.get(&node.op))
                .and_then(|spec| spec.param(&param.param))
                .map(|spec| spec.ty)
            {
                exposed.insert((group.id.clone(), param.name.clone()), ty);
            }
        }
    }
    for node in &mut doc.nodes {
        if let Some(spec) = registry.get(&node.op) {
            for (name, value) in &mut node.params {
                if let Some(param) = spec.param(name) {
                    *value = param.ty.check(value).map_err(|error| {
                        json!({"error":{"code":error.code,"message":error.message,"node":node.id}}).to_string()
                    })?;
                }
            }
        }
        if let Some(instance) = &mut node.instance {
            for (name, value) in &mut instance.overrides {
                if let Some(ty) = exposed.get(&(instance.group.clone(), name.clone())) {
                    *value = ty.check(value).map_err(|error| {
                        json!({"error":{"code":error.code,"message":error.message,"node":node.id}}).to_string()
                    })?;
                }
            }
        }
    }
    Ok(doc)
}

pub(crate) fn compact(source: &str, nodes: bool) -> String {
    let mut doc = match document(source) {
        Ok(doc) => doc,
        Err(error) => return error,
    };
    if !nodes {
        return match to_text_compact(&builtin_registry(), &doc) {
            Ok(text) => json!({"text":text}),
            Err(error) => json!({"error":error}),
        }
        .to_string();
    }
    // Give every top-level node an output so the printer keeps each node's own line.
    // The output also tells us the printer's chosen name, including disambiguated labels.
    doc.outputs = doc
        .nodes
        .iter()
        .filter(|node| node.group.is_none())
        .enumerate()
        .map(|(index, node)| Output {
            name: format!("read_{index}"),
            from: PortRef {
                node: node.id.clone(),
                port: "out".to_owned(),
            },
        })
        .collect();
    match to_text_compact(&builtin_registry(), &doc) {
        Ok(text) => {
            let statements: BTreeMap<_, _> = text
                .lines()
                .filter_map(|line| line.split_once(" = ").map(|(name, _)| (name, line)))
                .collect();
            let lines: BTreeMap<_, _> = doc
                .outputs
                .iter()
                .filter_map(|output| {
                    let prefix = format!("output {} = ", output.name);
                    let shorthand = format!("output {}", output.name);
                    let name = text.lines().find_map(|line| {
                        line.strip_prefix(&prefix)
                            .or_else(|| (line == shorthand).then_some(output.name.as_str()))
                    })?;
                    Some((output.from.node.clone(), *statements.get(name)?))
                })
                .collect();
            json!({"lines":lines}).to_string()
        }
        Err(error) => json!({"error":error}).to_string(),
    }
}

pub(crate) fn parse(source: &str, text: &str) -> String {
    let doc = match document(source) {
        Ok(doc) => doc,
        Err(error) => return error,
    };
    match parse_text_in(&builtin_registry(), &doc, text) {
        Ok(batch) => json!({"commands":batch.commands,"spans":batch.spans}),
        Err(diagnostics) => json!({"diagnostics":diagnostics}),
    }
    .to_string()
}

#[cfg(test)]
mod tests;
