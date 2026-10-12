//! Schema migrations: stepwise rewrites of a saved document from an older schema to the current one.
//!
//! Steps work on the JSON value, before it is read as a [`crate::Document`], so an old file never
//! has to fit today's types. A failed step leaves the file untouched (migration happens in memory;
//! nothing is written until the application saves).

use serde_json::{Map, Value};

use crate::diag::Diagnostic;
use crate::doc::SCHEMA;

/// One rewrite from schema `from` to `from + 1`.
#[derive(Clone, Copy)]
pub struct MigrationStep {
    /// The schema this step reads.
    pub from: u32,
    /// Rewrites the document value in place; the error is shown to the user.
    pub run: fn(&mut Value) -> Result<(), String>,
}

/// A chain of migration steps.
#[derive(Clone)]
pub struct Migrator {
    steps: Vec<MigrationStep>,
}

impl Migrator {
    /// The steps this build ships.
    #[must_use]
    pub fn builtin() -> Self {
        Self::with_steps(vec![MigrationStep {
            from: 0,
            run: schema0_to_1,
        }])
    }

    /// A migrator with the given steps (tests inject failing ones).
    #[must_use]
    pub fn with_steps(steps: Vec<MigrationStep>) -> Self {
        Self { steps }
    }

    /// Migrates `document` from schema `from` up to the current schema.
    ///
    /// # Errors
    /// A `store.migration_failed` diagnostic if a step is missing or fails.
    pub fn run(&self, document: &mut Value, from: u32) -> Result<(), Box<Diagnostic>> {
        for version in from..SCHEMA {
            let step = self
                .steps
                .iter()
                .find(|s| s.from == version)
                .ok_or_else(|| {
                    Box::new(Diagnostic::error(
                        "store.migration_failed",
                        format!("no migration from schema {version}"),
                    ))
                })?;
            (step.run)(document).map_err(|why| {
                Box::new(
                    Diagnostic::error(
                        "store.migration_failed",
                        format!("migrating from schema {version} failed: {why}"),
                    )
                    .suggest("the previous save was kept; the file was not changed"),
                )
            })?;
        }
        Ok(())
    }
}

fn object(value: &mut Value) -> Result<&mut Map<String, Value>, String> {
    value
        .as_object_mut()
        .ok_or_else(|| "the document is not an object".to_owned())
}

/// Schema 0 was the draft shape written in the G0 documents: a bare document with no ID state,
/// single links allowed where lists are now required, and frames and reroutes in the editor layer.
fn schema0_to_1(document: &mut Value) -> Result<(), String> {
    let doc = object(document)?;
    doc.insert("schema".to_owned(), Value::from(1));
    doc.entry("revision").or_insert_with(|| Value::from(0));
    doc.entry("units").or_insert_with(|| Value::from("m"));
    doc.entry("seed").or_insert_with(|| Value::from(0));
    doc.entry("outputs")
        .or_insert_with(|| Value::Array(Vec::new()));
    doc.entry("materials")
        .or_insert_with(|| Value::Array(Vec::new()));

    let count = |doc: &Map<String, Value>, key: &str| {
        doc.get(key).and_then(Value::as_array).map_or(0, Vec::len)
    };
    let issued = count(doc, "nodes") + count(doc, "materials") + 1;
    doc.entry("ids")
        .or_insert_with(|| serde_json::json!({ "next": issued, "salt": 0 }));

    if let Some(Value::Array(nodes)) = doc.get_mut("nodes") {
        for node in nodes {
            let node = object(node)?;
            node.entry("flags")
                .or_insert_with(|| serde_json::json!({ "muted": false, "bypass": false }));
            if let Some(Value::Object(inputs)) = node.get_mut("inputs") {
                for links in inputs.values_mut() {
                    if links.is_object() {
                        *links = Value::Array(vec![links.take()]);
                    }
                }
            } else {
                node.insert("inputs".to_owned(), Value::Object(Map::new()));
            }
        }
    }
    match doc.get_mut("editor") {
        Some(Value::Object(editor)) => {
            editor.remove("frames");
            editor.remove("reroutes");
            editor
                .entry("positions")
                .or_insert_with(|| Value::Object(Map::new()));
            editor
                .entry("collapsed")
                .or_insert_with(|| Value::Array(Vec::new()));
        }
        _ => {
            doc.insert(
                "editor".to_owned(),
                serde_json::json!({ "positions": {}, "collapsed": [] }),
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_zero_gains_ids_list_links_and_a_clean_editor_layer() {
        let mut doc = serde_json::json!({
            "id": "d_7k2m9q4x",
            "nodes": [
                { "id": "n_a1b2c3d4", "op": "profile.polygon@1", "params": {}, "inputs": {} },
                { "id": "n_f9e8d7c6", "op": "solid.revolve@1", "params": {},
                  "inputs": { "in": { "node": "n_a1b2c3d4", "port": "out" } } }
            ],
            "editor": { "positions": {}, "frames": [1], "reroutes": [2] }
        });
        schema0_to_1(&mut doc).unwrap();
        assert_eq!(doc["schema"], 1);
        assert_eq!(doc["ids"]["next"], 3);
        assert_eq!(doc["nodes"][1]["inputs"]["in"][0]["node"], "n_a1b2c3d4");
        assert!(doc["editor"].get("frames").is_none() && doc["editor"].get("reroutes").is_none());
        assert_eq!(doc["nodes"][0]["flags"]["muted"], false);
    }

    #[test]
    fn a_missing_step_and_a_failing_step_are_diagnosed() {
        let mut doc = serde_json::json!({});
        let none = Migrator::with_steps(Vec::new());
        assert_eq!(
            none.run(&mut doc, 0).unwrap_err().code,
            "store.migration_failed"
        );
        let failing = Migrator::with_steps(vec![MigrationStep {
            from: 0,
            run: |_| Err("boom".into()),
        }]);
        let err = failing.run(&mut doc, 0).unwrap_err();
        assert!(err.message.contains("boom"), "{}", err.message);
        assert!(
            Migrator::builtin().run(&mut doc, SCHEMA).is_ok(),
            "nothing to do at the current schema"
        );
    }
}
