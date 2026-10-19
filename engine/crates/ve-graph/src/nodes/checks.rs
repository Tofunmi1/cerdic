//! Edits checked against budget and range limits: named outputs, materials and the editor layer.

use std::collections::BTreeMap;

use ve_core::Name;

use crate::apply::{Ctx, Fail, fail};
use crate::diag::Diagnostic;
use crate::doc::{Material, OUT_PORT, Output, PortRef};
use crate::id::{MaterialId, NodeId};
use crate::limits::{
    MAX_EDITOR_COORD, MAX_LIST_VALUES, MAX_MATERIAL_ENTRIES, MAX_MATERIALS, MAX_OUTPUTS,
    MAX_STRING_BYTES, count,
};
use crate::value::Value;

use super::existing;

pub(crate) fn set_output(
    ctx: &mut Ctx<'_>,
    name: &str,
    from: Option<&PortRef>,
) -> Result<(), Fail> {
    Name::new(name).map_err(|e| fail("parse.syntax", format!("bad output name: {e}")))?;
    let Some(from) = from else {
        ctx.doc.outputs.retain(|o| o.name != name);
        return Ok(());
    };
    existing(ctx, &from.node)?;
    if from.port != OUT_PORT {
        return Err(Box::new(
            Diagnostic::error(
                "graph.unknown_port",
                format!("no output port {:?}", from.port),
            )
            .at_node(&from.node)
            .suggest(format!("use the output port {OUT_PORT:?}")),
        ));
    }
    let is_new = ctx.doc.outputs.iter().all(|o| o.name != name);
    if is_new && ctx.doc.outputs.len() >= MAX_OUTPUTS {
        return Err(Box::new(
            Diagnostic::error("budget.nodes", "too many named outputs")
                .with_limit(count(MAX_OUTPUTS), count(ctx.doc.outputs.len() + 1))
                .suggest(format!(
                    "remove outputs to stay within {MAX_OUTPUTS} named outputs"
                )),
        ));
    }
    let output = Output {
        name: name.to_owned(),
        from: from.clone(),
    };
    match ctx.doc.outputs.iter_mut().find(|o| o.name == name) {
        Some(slot) => *slot = output,
        None => ctx.doc.outputs.push(output),
    }
    ctx.changed.insert(from.node.clone());
    Ok(())
}

fn plain_value_ok(value: &Value) -> bool {
    match value {
        Value::Num(n) => n.is_finite(),
        Value::Str(s) => s.len() <= MAX_STRING_BYTES,
        Value::List(items) => items.len() <= MAX_LIST_VALUES && items.iter().all(plain_value_ok),
        Value::Bool(_) | Value::Int(_) => true,
    }
}

pub(crate) fn define_material(
    ctx: &mut Ctx<'_>,
    def: &BTreeMap<String, Value>,
) -> Result<(), Fail> {
    if ctx.doc.materials.len() >= MAX_MATERIALS {
        return Err(Box::new(
            Diagnostic::error("budget.nodes", "too many materials")
                .with_limit(count(MAX_MATERIALS), count(ctx.doc.materials.len() + 1))
                .suggest(format!(
                    "remove materials to stay within {MAX_MATERIALS} definitions"
                )),
        ));
    }
    let bad_key = def.keys().any(|k| k.is_empty() || k.len() > 64);
    if def.len() > MAX_MATERIAL_ENTRIES || bad_key || !def.values().all(plain_value_ok) {
        return Err(Box::new(
            Diagnostic::error("param.range", "the material definition is out of bounds")
                .with_limit(count(MAX_MATERIAL_ENTRIES), count(def.len()))
                .suggest(format!(
                    "use at most {MAX_MATERIAL_ENTRIES} entries named 1 to 64 bytes, with finite values"
                )),
        ));
    }
    let id = ctx
        .doc
        .issue_material_id()
        .ok_or_else(|| fail("budget.nodes", "ids are used up"))?;
    ctx.doc.materials.push(Material {
        id,
        def: def.clone(),
    });
    Ok(())
}

pub(crate) fn remove_material(ctx: &mut Ctx<'_>, id: &MaterialId) -> Result<(), Fail> {
    let before = ctx.doc.materials.len();
    ctx.doc.materials.retain(|m| &m.id != id);
    if ctx.doc.materials.len() == before {
        return Err(fail(
            "graph.unknown_material",
            format!("there is no material {id}"),
        ));
    }
    Ok(())
}

pub(crate) fn editor_set_position(
    ctx: &mut Ctx<'_>,
    node: &NodeId,
    x: f64,
    y: f64,
) -> Result<(), Fail> {
    existing(ctx, node)?;
    for v in [x, y] {
        if !v.is_finite() {
            return Err(fail("param.non_finite", "positions must be finite"));
        }
        if v.abs() > MAX_EDITOR_COORD {
            return Err(Box::new(
                Diagnostic::error("param.range", "position is too far from the origin")
                    .with_limit(MAX_EDITOR_COORD, v.abs())
                    .suggest(format!("keep each coordinate within ±{MAX_EDITOR_COORD}")),
            ));
        }
    }
    ctx.doc.editor.positions.insert(node.clone(), [x, y]);
    Ok(())
}

pub(crate) fn editor_set_collapsed(
    ctx: &mut Ctx<'_>,
    node: &NodeId,
    collapsed: bool,
) -> Result<(), Fail> {
    existing(ctx, node)?;
    if collapsed {
        ctx.doc.editor.collapsed.insert(node.clone());
    } else {
        ctx.doc.editor.collapsed.remove(node);
    }
    Ok(())
}
