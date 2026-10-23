//! Patterns (ADR 0060): functions of position built by `field.expr` (the field-expression language of `ve_world::expr`) and
//! `field.mask_from` (a terrain layer). A node that takes a pattern input samples it where it needs a value; the pattern knows
//! nothing about what the value is for.
use std::rc::Rc;

use ve_graph::{Node, Value as Param};
use ve_world::expr::{Bindings, Diagnostic, Expr, Sample, Type, Value as Sampled};

use crate::world_terrain::Tile;
use crate::{Fail, cache::Value, fail_suggest};

/// A function of a point on a surface: its position and the surface normal there.
pub(crate) struct Pattern {
    kind: Kind,
}

enum Kind {
    /// A compiled expression and the patterns linked to its named inputs (`a`, `b`, ...), sampled at the same point.
    Expr {
        expr: Expr,
        inputs: Vec<Rc<Pattern>>,
    },
    /// A named layer of a terrain tile, zero outside the tile.
    Layer { tile: Rc<Tile>, layer: String },
}

impl Pattern {
    /// The type of the value this pattern gives.
    pub(crate) fn output_type(&self) -> Type {
        match &self.kind {
            Kind::Expr { expr, .. } => expr.output_type(),
            Kind::Layer { .. } => Type::Float,
        }
    }

    /// The value at `position` on a surface whose normal there is `normal`.
    pub(crate) fn sample(&self, position: [f32; 3], normal: [f32; 3]) -> Sampled {
        match &self.kind {
            Kind::Expr { expr, inputs } => {
                let bound: Vec<Sampled> = inputs
                    .iter()
                    .map(|input| input.sample(position, normal))
                    .collect();
                expr.eval(&Sample {
                    p: position,
                    n: normal,
                    bound: &bound,
                    ..Sample::default()
                })
            }
            Kind::Layer { tile, layer } => {
                let grid = &tile.grid;
                let (origin, cell) = (grid.origin(), grid.cell());
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "tile sizes are at most 2048 cells"
                )]
                let far = [
                    origin[0] + (grid.width() - 1) as f32 * cell,
                    origin[1] + (grid.depth() - 1) as f32 * cell,
                ];
                let inside = position[0] >= origin[0]
                    && position[0] <= far[0]
                    && position[2] >= origin[1]
                    && position[2] <= far[1];
                let value = if inside {
                    grid.sample_layer(layer, position[0], position[2])
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
                Sampled::Float(value)
            }
        }
    }

    /// The first component of [`Pattern::sample`], for the nodes that need one number (a density, a weight).
    pub(crate) fn float(&self, position: [f32; 3], normal: [f32; 3]) -> f32 {
        self.sample(position, normal)
            .floats()
            .first()
            .copied()
            .unwrap_or(0.0)
    }
}

pub(crate) fn run(node: &Node, inputs: &[Rc<Value>]) -> Option<Result<Value, Fail>> {
    match node.op.family().as_str() {
        "field.expr" => Some(expression(node, inputs)),
        "field.mask_from" => Some(mask_from(node, inputs)),
        _ => None,
    }
}

fn text<'a>(node: &'a Node, name: &str) -> &'a str {
    match node.params.get(name) {
        Some(Param::Str(value)) => value.as_str(),
        _ => "",
    }
}

fn pattern_value(pattern: Pattern) -> Value {
    let mut value = Value::nothing();
    value.pattern = Some(Rc::new(pattern));
    value
}

/// The diagnostics of the expression language as one failure that names the first problem with its place and fix.
fn expression_failure(node: &Node, diagnostics: &[Diagnostic]) -> Fail {
    let shown: Vec<String> = diagnostics
        .iter()
        .take(3)
        .map(|d| format!("line {}, column {}: {}", d.line, d.column, d.message))
        .collect();
    let suggestion = diagnostics
        .first()
        .map_or_else(String::new, |d| d.suggestion.clone());
    fail_suggest(
        "expr.error",
        format!("{} has an expression error: {}", node.id, shown.join("; ")),
        suggestion,
    )
}

fn expression(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    // `inputs` follow the order of the node's linked ports, which is alphabetical: a, b, c, d.
    let mut fields = Vec::new();
    let mut linked = Vec::new();
    for (port, value) in node.inputs.keys().zip(inputs) {
        let pattern = value.pattern.clone().ok_or_else(|| {
            fail_suggest(
                "pattern.not_a_pattern",
                format!(
                    "{} gets {port} from a node that does not produce a pattern",
                    node.id
                ),
                "link a field.expr or field.mask_from node, or leave the input empty",
            )
        })?;
        fields.push((port.clone(), pattern.output_type()));
        linked.push(pattern);
    }
    let expr = Expr::compile(text(node, "code"), &Bindings { fields })
        .map_err(|found| expression_failure(node, &found))?;
    Ok(pattern_value(Pattern {
        kind: Kind::Expr {
            expr,
            inputs: linked,
        },
    }))
}

fn mask_from(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let tile = inputs.first().and_then(|value| value.grid.clone()).ok_or_else(|| {
        fail_suggest(
            "terrain.needs_tile",
            "field.mask_from needs the output of terrain.tile (or another terrain node, such as terrain.mask)",
            "connect a terrain node whose layers you want to read",
        )
    })?;
    let layer = text(node, "layer").to_owned();
    if tile.grid.layer(&layer).is_none() {
        return Err(fail_suggest(
            "terrain.unknown_layer",
            format!(
                "the tile has no layer called {layer:?} (it has {:?})",
                tile.grid.layer_names()
            ),
            "make it first with terrain.mask, before this node, and use the same name",
        ));
    }
    Ok(pattern_value(Pattern {
        kind: Kind::Layer { tile, layer },
    }))
}
