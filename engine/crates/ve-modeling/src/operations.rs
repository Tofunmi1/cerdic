//! Operators consume immutable mesh values. Native solids exist only while one node executes.
use crate::{Fail, cache::Value, fail};
use std::rc::Rc;
use ve_graph::{Node, Value as Param};
use ve_kernel::{ExecutionContext, KernelError, Mesh, Solid};

pub(crate) fn execute(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    if context.is_cancelled() {
        return Err(fail("eval.cancelled", "evaluation was cancelled"));
    }
    if node.flags.muted {
        return Ok(Value {
            mesh: Mesh::default(),
            volume: 0.0,
        });
    }
    if node.flags.bypass {
        let input = inputs
            .first()
            .ok_or_else(|| fail("graph.missing_input", "bypass requires an input"))?;
        return Ok(Value {
            mesh: input.mesh.clone(),
            volume: input.volume,
        });
    }
    let family = node.op.family().as_str();
    let first_empty = inputs.first().is_some_and(|input| input.mesh.is_empty());
    if ((family == "solid.difference" || family.starts_with("xform.")) && first_empty)
        || (family == "solid.intersect" && inputs.iter().any(|input| input.mesh.is_empty()))
        || (family == "solid.union" && inputs.iter().all(|input| input.mesh.is_empty()))
    {
        return Ok(Value {
            mesh: Mesh::default(),
            volume: 0.0,
        });
    }
    let nonempty: Vec<_> = inputs
        .iter()
        .filter(|input| !input.mesh.is_empty())
        .map(Rc::clone)
        .collect();
    let result = solid(node, &nonempty, context).map_err(|error| {
        Box::new(
            ve_graph::Diagnostic::error(
                if error == KernelError::Cancelled {
                    "eval.cancelled"
                } else {
                    "kernel.invalid_input"
                },
                error.to_string(),
            )
            .at_node(&node.id),
        )
    })?;
    if context.is_cancelled() {
        return Err(fail("eval.cancelled", "cancelled result discarded"));
    }
    let mesh = if result.is_empty() {
        Mesh::default()
    } else {
        result
            .to_mesh()
            .map_err(|e| fail("kernel.failed", e.to_string()))?
    };
    Ok(Value {
        mesh,
        volume: result.volume(),
    })
}

fn solid(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Solid, KernelError> {
    let kind = node.op.family().as_str();
    let number = |name| {
        node.params
            .get(name)
            .and_then(Param::as_f64)
            .ok_or(KernelError::InvalidArgument("missing number"))
    };
    let segments = || -> Result<u32, KernelError> {
        let value = number("segments")?;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "preflight enforces integral bounded segments"
        )]
        Ok(value as u32)
    };
    match kind {
        "solid.box" => Solid::cube(vector(node, "size")?, true),
        "solid.sphere" => Solid::sphere(number("radius")?, segments()?),
        "solid.cylinder" => Solid::cylinder(
            number("height")?,
            number("radius")?,
            number("radius")?,
            segments()?,
            true,
        ),
        _ => {
            if kind == "solid.union" && inputs.len() >= 2 {
                let solids: Vec<_> = inputs
                    .iter()
                    .map(|input| Solid::from_mesh(&input.mesh))
                    .collect::<Result<_, _>>()?;
                let borrowed: Vec<_> = solids.iter().collect();
                return Solid::union_all_with_context(&borrowed, context);
            }
            let input = inputs
                .first()
                .ok_or(KernelError::InvalidArgument("missing input"))?;
            let mut value = Solid::from_mesh(&input.mesh)?;
            match kind {
                "xform.move" => value.translate(vector(node, "by")?),
                "xform.rotate" => value.rotate(vector(node, "angles")?.map(f64::to_degrees)),
                "xform.scale" => value.scale(vector(node, "by")?),
                _ => {
                    for input in inputs.iter().skip(1) {
                        let next = Solid::from_mesh(&input.mesh)?;
                        value = match kind {
                            "solid.union" => value.union_with_context(&next, context)?,
                            "solid.difference" => value.difference_with_context(&next, context)?,
                            "solid.intersect" => value.intersect_with_context(&next, context)?,
                            _ => return Err(KernelError::InvalidArgument("unknown operation")),
                        };
                    }
                    Ok(value)
                }
            }
        }
    }
}

fn vector(node: &Node, name: &str) -> Result<[f64; 3], KernelError> {
    let Some(Param::List(values)) = node.params.get(name) else {
        return Err(KernelError::InvalidArgument("missing vector"));
    };
    let mut out = [0.0; 3];
    for (index, value) in out.iter_mut().enumerate() {
        *value = values
            .get(index)
            .and_then(Param::as_f64)
            .ok_or(KernelError::InvalidArgument("vector component"))?;
    }
    Ok(out)
}
