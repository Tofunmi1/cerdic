//! Collection and validation of document-authored script nodes.
use crate::{Fail, fail};
use serde::{Deserialize, Serialize};
use ve_graph::{Document, Node, Value};

/// A JavaScript behaviour saved in the authored document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScriptDef {
    /// Unique script name used by the page.
    pub name: String,
    /// Body of the async script function.
    pub source: String,
    /// Whether the page should run this script.
    pub enabled: bool,
    /// Authored node id.
    pub node: String,
}

/// Validates and collects scripts in the document's node order.
pub(crate) fn collect(doc: &Document) -> Result<Vec<ScriptDef>, Fail> {
    let mut scripts = Vec::new();
    for node in doc
        .nodes
        .iter()
        .filter(|node| node.op.family().as_str() == "script.js")
    {
        let source = match node.params.get("source") {
            Some(Value::Str(source)) => source.clone(),
            _ => return Err(at(fail("param.type", "script source must be text"), node)),
        };
        if source.trim().is_empty() || source.len() > 65_536 {
            return Err(at(
                fail(
                    "param.range",
                    "script source must be 1 to 65,536 bytes and not blank; change the source",
                ),
                node,
            ));
        }
        let requested = match node.params.get("name") {
            Some(Value::Str(name)) => name.as_str(),
            None => "",
            _ => return Err(at(fail("param.type", "script name must be text"), node)),
        };
        let name = if requested.is_empty() {
            node.label.clone().unwrap_or_else(|| node.id.to_string())
        } else {
            requested.to_owned()
        };
        if !valid_name(&name) {
            return Err(at(
                fail(
                    "script.name",
                    format!(
                        "script name {name:?} must be lowercase letters, digits and underscores, starting with a letter, at most 32 characters; change the name"
                    ),
                ),
                node,
            ));
        }
        if scripts.iter().any(|script: &ScriptDef| script.name == name) {
            return Err(at(
                fail(
                    "script.name",
                    format!("duplicate script name {name:?}; change the repeated name"),
                ),
                node,
            ));
        }
        let enabled = match node.params.get("enabled") {
            Some(Value::Bool(enabled)) => *enabled,
            None => true,
            _ => {
                return Err(at(
                    fail("param.type", "script enabled must be true or false"),
                    node,
                ));
            }
        };
        scripts.push(ScriptDef {
            name,
            source,
            enabled,
            node: node.id.to_string(),
        });
    }
    Ok(scripts)
}

fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && name.len() <= 32
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn at(mut error: Fail, node: &Node) -> Fail {
    error.node = Some(node.id.clone());
    if let Some(label) = &node.label {
        error.message = format!("{} (at {label})", error.message);
    }
    error
}
