//! Lets a `poly.script` call any registered operator as a function: `op('poly.grid@1', { width: 2, height: 2 })`.
//!
//! The call builds a one-node document through the same typed command every other author uses (so parameter names, types, ranges
//! and defaults are checked exactly as usual), runs the operator, and hands the result back as plain data: a mesh as
//! `{ points, faces }`, a curve as `{ points, closed }`, a number as `{ number }`.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::{Value as Json, json};
use ve_graph::{Command, CommandBatch, Outcome, PortRef, Session, TransactionId, Value as Param};
use ve_kernel::ExecutionContext;

use super::convert::value_from_poly;
use super::script::mesh_from;
use crate::cache::{Curve3, Value};
use crate::script::Host;

/// The most operator calls one script may make.
const MAX_CALLS: u32 = 128;

/// The JavaScript side: `op(id, params, inputs)` returns the operator's result as data.
pub(super) const HELPER: &str = "function op(id, params, inputs) { return JSON.parse(__host(String(id), JSON.stringify(params || {}), JSON.stringify(inputs || {}))); }";

/// The host function behind `op`, with its own call counter.
pub(super) fn host() -> Host {
    let calls = Rc::new(Cell::new(0_u32));
    Rc::new(move |id, params, inputs| {
        calls.set(calls.get() + 1);
        if calls.get() > MAX_CALLS {
            return Err(format!(
                "op() was called more than {MAX_CALLS} times in one script; build bigger pieces per call"
            ));
        }
        call(id, params, inputs)
    })
}

fn call(id: &str, params: &str, inputs: &str) -> Result<String, String> {
    let family = id.split('@').next().unwrap_or(id);
    if family == "poly.script" || family.starts_with("script.") {
        return Err(format!(
            "op() cannot call {family}: a script cannot start another script"
        ));
    }
    let params: BTreeMap<String, Param> = serde_json::from_str(params).map_err(|e| {
        format!("op('{id}') parameters must be an object of numbers, text, booleans and lists: {e}")
    })?;
    let inputs: BTreeMap<String, Json> = serde_json::from_str(inputs).map_err(|e| {
        format!("op('{id}') inputs must be an object of named meshes, curves or numbers: {e}")
    })?;
    let mut session = Session::with_builtin(1);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_script_op")
            .map_err(|_| "internal: transaction id".to_owned())?,
        label: None,
        commands: vec![Command::AddNode {
            op: id.to_owned(),
            params,
            label: None,
            alias: None,
            group: None,
        }],
    });
    if let Outcome::Rejected { diagnostics } = &applied {
        let said: Vec<String> = diagnostics
            .iter()
            .take(3)
            .map(|d| match &d.suggestion {
                Some(fix) => format!("{} ({fix})", d.message),
                None => d.message.clone(),
            })
            .collect();
        return Err(format!("op('{id}') was refused: {}", said.join("; ")));
    }
    let mut node = session
        .document()
        .nodes
        .first()
        .cloned()
        .ok_or_else(|| format!("op('{id}') made no node"))?;
    let context = ExecutionContext::new().map_err(|e| e.to_string())?;
    let mut values: Vec<Rc<Value>> = Vec::new();
    // Ports are read in name order, the order the evaluator hands inputs to an operator.
    // A list under a port feeds a variadic port (`solid.union` and friends) one value per element.
    for (port, json) in &inputs {
        let each: Vec<&Json> = match json {
            Json::Array(items) => items.iter().collect(),
            single => vec![single],
        };
        let links = each
            .iter()
            .map(|_| PortRef {
                node: node.id.clone(),
                port: "out".to_owned(),
            })
            .collect();
        node.inputs.insert(port.clone(), links);
        for item in each {
            values.push(Rc::new(input_value(&node, id, port, item, &context)?));
        }
    }
    let result = crate::operations::execute(
        &node,
        &values,
        &context,
        crate::Quality::Final,
        &BTreeMap::new(),
    )
    .map_err(|d| format!("op('{id}') failed: {}", describe(&d)))?;
    Ok(output_json(&result).to_string())
}

fn describe(failure: &crate::Fail) -> String {
    match &failure.suggestion {
        Some(fix) => format!("{} ({fix})", failure.message),
        None => failure.message.clone(),
    }
}

/// One input as the operator receives it.
fn input_value(
    node: &ve_graph::Node,
    id: &str,
    port: &str,
    json: &Json,
    context: &ExecutionContext,
) -> Result<Value, String> {
    if let Some(number) = json.as_f64() {
        return Ok(Value::number(number));
    }
    let wrong = || {
        format!(
            "op('{id}') input '{port}' must be a mesh {{ points, faces }}, a curve {{ points, closed }} or a number"
        )
    };
    let object = json.as_object().ok_or_else(wrong)?;
    let has_faces = object
        .get("faces")
        .and_then(Json::as_array)
        .is_some_and(|f| !f.is_empty());
    if has_faces {
        let mesh = mesh_from(node, &json.to_string()).map_err(|d| describe(&d))?;
        return value_from_poly(node, mesh, None, context).map_err(|d| describe(&d));
    }
    let points = object
        .get("points")
        .and_then(Json::as_array)
        .ok_or_else(wrong)?;
    let mut out = Vec::with_capacity(points.len());
    for point in points {
        let p: Vec<f64> = point
            .as_array()
            .map(|a| a.iter().filter_map(Json::as_f64).collect())
            .unwrap_or_default();
        if p.len() != 3 || p.iter().any(|v| !v.is_finite()) {
            return Err(format!(
                "op('{id}') input '{port}' has a point that is not three finite numbers"
            ));
        }
        out.push([p[0], p[1], p[2]]);
    }
    let closed = object
        .get("closed")
        .and_then(Json::as_bool)
        .unwrap_or(false);
    Ok(Value::curve(Curve3 {
        points: out,
        closed,
    }))
}

/// A result as data for the script.
fn output_json(value: &Value) -> Json {
    if let Some(poly) = &value.poly {
        let mut faces = Vec::with_capacity(poly.face_sizes().len());
        let mut at = 0;
        for &size in poly.face_sizes() {
            let end = at + size as usize;
            faces.push(&poly.corners()[at..end]);
            at = end;
        }
        return json!({ "points": poly.positions(), "faces": faces });
    }
    if let Some(curve) = &value.curve {
        return json!({ "points": curve.points, "closed": curve.closed });
    }
    if let Some(number) = value.number {
        return json!({ "number": number });
    }
    let triangles: Vec<&[u32]> = value.mesh.indices.chunks_exact(3).collect();
    json!({ "points": value.mesh.positions, "faces": triangles })
}
