//! `curve.expr`: a space curve written as a program of one number, `t`, from 0 to 1.

use ve_graph::{Node, Value as Param};
use ve_world::expr::{Bindings, Sample, Type, Value as Sampled};

use crate::Fail;
use crate::cache::{Curve3, Value};
use crate::poly_ops::compile;

fn fail(node: &Node, code: &str, message: String, suggestion: &str) -> Fail {
    Box::new(
        ve_graph::Diagnostic::error(code, message)
            .at_node(&node.id)
            .suggest(suggestion),
    )
}

pub(crate) fn build(node: &Node) -> Result<Value, Fail> {
    let samples = match node.params.get("samples") {
        Some(Param::Int(n)) => usize::try_from(*n).unwrap_or(0),
        _ => 0,
    };
    let closed = matches!(node.params.get("closed"), Some(Param::Bool(true)));
    let bindings = Bindings {
        fields: vec![
            ("t".to_owned(), Type::Float),
            ("i".to_owned(), Type::Float),
            ("count".to_owned(), Type::Float),
        ],
    };
    let expr = compile(node, &bindings, Type::Vec3)?;
    // An open curve ends at t = 1; a closed one stops one step short so the last point is not the first again.
    let steps = if closed {
        samples
    } else {
        samples.saturating_sub(1)
    }
    .max(1);
    let mut points: Vec<[f64; 3]> = Vec::with_capacity(samples);
    for k in 0..samples {
        #[expect(
            clippy::cast_precision_loss,
            reason = "sample numbers are at most 4096"
        )]
        let (t, i, n) = (k as f32 / steps as f32, k as f32, samples as f32);
        let bound = [Sampled::Float(t), Sampled::Float(i), Sampled::Float(n)];
        let Sampled::Vec3(p) = expr.eval(&Sample {
            p: [t, 0.0, 0.0],
            bound: &bound,
            ..Sample::default()
        }) else {
            return Err(fail(
                node,
                "expr.error",
                "the program must give a vec3".to_owned(),
                "make out a vec3",
            ));
        };
        points.push(p.map(f64::from));
    }
    if points.windows(2).any(|pair| {
        let d = [
            pair[0][0] - pair[1][0],
            pair[0][1] - pair[1][1],
            pair[0][2] - pair[1][2],
        ];
        d[0] * d[0] + d[1] * d[1] + d[2] * d[2] <= 1.0e-18
    }) {
        return Err(fail(
            node,
            "geometry.degenerate_curve",
            "the program gives the same point for two neighbouring samples".to_owned(),
            "make the curve move with t, or use fewer samples",
        ));
    }
    Ok(Value::curve(Curve3 { points, closed }))
}
