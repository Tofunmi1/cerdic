//! The saved-file envelope: format tag, schema, generation, checksum, operator manifest and document.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::diag::Diagnostic;
use crate::doc::{Document, SCHEMA};
use crate::id::NodeId;
use crate::limits::{MAX_DOCUMENT_BYTES, MAX_NODES, count};
use crate::spec::OperatorRegistry;
use crate::store::Loaded;
use crate::store::migrate::Migrator;

/// The `format` tag of a saved document.
pub(crate) const FORMAT: &str = "ve-doc";

#[derive(Serialize, Deserialize)]
struct Envelope {
    format: String,
    schema: u32,
    generation: u64,
    saved_by: String,
    checksum: String,
    manifest: Vec<(String, u32)>,
    document: Value,
}

/// A 128-bit non-cryptographic digest, enough to catch truncation and accidental damage.
fn checksum(value: &Value) -> String {
    let text = serde_json::to_string(value).unwrap_or_default();
    let fold = |offset: u64| {
        text.bytes().fold(offset, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01B3)
        })
    };
    format!(
        "{:016x}{:016x}",
        fold(0xCBF2_9CE4_8422_2325),
        fold(0x8422_2325_CBF2_9CE4)
    )
}

/// The text to store for `doc` at `generation`.
pub(crate) fn encode(doc: &Document, generation: u64) -> Result<String, Box<Diagnostic>> {
    let document = serde_json::to_value(doc).map_err(|e| {
        Box::new(Diagnostic::error(
            "store.corrupt",
            format!("cannot encode: {e}"),
        ))
    })?;
    let manifest: BTreeSet<(String, u32)> = doc
        .nodes
        .iter()
        .map(|n| (n.op.family().to_string(), n.op.version()))
        .collect();
    let envelope = Envelope {
        format: FORMAT.to_owned(),
        schema: SCHEMA,
        generation,
        saved_by: format!("ve-graph {}", env!("CARGO_PKG_VERSION")),
        checksum: checksum(&document),
        manifest: manifest.into_iter().collect(),
        document,
    };
    serde_json::to_string(&envelope).map_err(|e| {
        Box::new(Diagnostic::error(
            "store.corrupt",
            format!("cannot encode: {e}"),
        ))
    })
}

fn corrupt(why: impl Into<String>) -> Box<Diagnostic> {
    Box::new(Diagnostic::error("store.corrupt", why).suggest("the previous save, if any, was kept"))
}

/// What a stored text claims to be, without fully loading it.
pub(crate) enum Peek {
    /// An intact envelope of this generation (schema at or below the current one).
    Valid { generation: u64 },
    /// A document file from a newer schema. Recognised before the checksum is checked, because a
    /// newer version may checksum differently; it must never be mistaken for damage and replaced.
    Newer,
    /// Anything else (truncated, damaged, not ours).
    Invalid,
}

/// Reads just enough of `text` to say whether it is an intact save.
pub(crate) fn peek(text: &str) -> Peek {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Peek::Invalid;
    };
    let from_newer = value.get("format").and_then(Value::as_str) == Some(FORMAT)
        && value
            .get("schema")
            .and_then(Value::as_u64)
            .is_some_and(|s| s > u64::from(SCHEMA));
    if from_newer {
        return Peek::Newer;
    }
    let (Some(schema), Some(generation), Some(sum), Some(document)) = (
        value.get("schema").and_then(Value::as_u64),
        value.get("generation").and_then(Value::as_u64),
        value.get("checksum").and_then(Value::as_str),
        value.get("document"),
    ) else {
        return Peek::Invalid;
    };
    if schema <= u64::from(SCHEMA) && checksum(document) == sum {
        Peek::Valid { generation }
    } else {
        Peek::Invalid
    }
}

/// Reads a stored text: checks it, migrates it, and validates the document.
pub(crate) fn decode(
    text: &str,
    migrator: &Migrator,
    registry: &OperatorRegistry,
) -> Result<Loaded, Box<Diagnostic>> {
    let value: Value = serde_json::from_str(text).map_err(|e| {
        corrupt(format!(
            "the file is not valid JSON (truncated or damaged): {e}"
        ))
    })?;
    let (mut document, schema, generation) = match value.get("format").and_then(Value::as_str) {
        Some(FORMAT) => envelope_parts(&value)?,
        Some(other) => return Err(corrupt(format!("unknown file format {other:?}"))),
        None if value.get("nodes").is_some() => (value, 0, 0),
        None => return Err(corrupt("this is not a document file")),
    };
    migrator.run(&mut document, schema)?;
    let doc: Document = serde_json::from_value(document)
        .map_err(|e| corrupt(format!("the document does not fit the schema: {e}")))?;
    check_document(&doc)?;
    let unknown: Vec<NodeId> = doc
        .nodes
        .iter()
        .filter(|n| registry.get(&n.op).is_none())
        .map(|n| n.id.clone())
        .collect();
    let mut issues = Vec::new();
    if !unknown.is_empty() {
        issues.push(
            Diagnostic::error(
                "store.unknown_op",
                format!("{} node(s) use operators this build does not have", unknown.len()),
            )
            .with_related(unknown)
            .suggest("the file is kept and shown read-only; open it with a newer build to edit those nodes"),
        );
    }
    Ok(Loaded {
        read_only: !issues.is_empty(),
        document: doc,
        generation,
        issues,
        migrated_from: (schema < SCHEMA).then_some(schema),
    })
}

fn envelope_parts(value: &Value) -> Result<(Value, u32, u64), Box<Diagnostic>> {
    let schema = value
        .get("schema")
        .and_then(Value::as_u64)
        .ok_or_else(|| corrupt("the file has no schema version"))?;
    if schema > u64::from(SCHEMA) {
        return Err(Box::new(
            Diagnostic::error(
                "store.newer_version",
                format!("the file uses schema {schema}; this build reads up to {SCHEMA}"),
            )
            .suggest("open it with a newer build; the file was not changed"),
        ));
    }
    let document = value
        .get("document")
        .ok_or_else(|| corrupt("the file has no document"))?;
    let sum = value
        .get("checksum")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if checksum(document) != sum {
        return Err(corrupt("the checksum does not match: the file was damaged"));
    }
    let generation = value.get("generation").and_then(Value::as_u64).unwrap_or(0);
    Ok((
        document.clone(),
        u32::try_from(schema).unwrap_or(SCHEMA),
        generation,
    ))
}

/// Structural checks a file must pass before it is treated as a document.
fn check_document(doc: &Document) -> Result<(), Box<Diagnostic>> {
    let size = doc.canonical_json().len();
    if size > MAX_DOCUMENT_BYTES {
        return Err(Box::new(
            Diagnostic::error(
                "budget.source",
                "the saved document is larger than the limit",
            )
            .with_limit(count(MAX_DOCUMENT_BYTES), count(size)),
        ));
    }
    if doc.nodes.len() > MAX_NODES {
        return Err(Box::new(
            Diagnostic::error("budget.nodes", "the saved document has too many nodes")
                .with_limit(count(MAX_NODES), count(doc.nodes.len())),
        ));
    }
    let ids: BTreeSet<&NodeId> = doc.nodes.iter().map(|n| &n.id).collect();
    if ids.len() != doc.nodes.len() {
        return Err(corrupt("two nodes share an ID"));
    }
    let dangling = doc
        .nodes
        .iter()
        .flat_map(|n| n.inputs.values().flatten())
        .map(|r| &r.node)
        .chain(doc.outputs.iter().map(|o| &o.from.node))
        .chain(doc.editor.positions.keys())
        .chain(doc.editor.collapsed.iter())
        .find(|id| !ids.contains(id));
    match dangling {
        Some(id) => Err(corrupt(format!(
            "something refers to the missing node {id}"
        ))),
        None => Ok(()),
    }
}
