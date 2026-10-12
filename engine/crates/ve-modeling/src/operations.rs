//! Operators consume immutable mesh values. Native solids exist only while one node executes.
use crate::{Fail, appearance, cache::Value, fail, profile_ops, shapes};
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
        return Ok(Value::nothing());
    }
    if node.flags.bypass {
        let input = inputs
            .first()
            .ok_or_else(|| fail("graph.missing_input", "bypass requires an input"))?;
        return Ok(Value {
            mesh: input.mesh.clone(),
            volume: input.volume,
            loops: input.loops.clone(),
            material: input.material.clone(),
        });
    }
    let family = node.op.family().as_str();
    if family == "solid.material" {
        return appearance::material(node, inputs);
    }
    if family.starts_with("profile.") || family == "check.min_wall" {
        return profile_ops::run(node, inputs);
    }
    let first_empty = inputs.first().is_some_and(|input| input.mesh.is_empty());
    if ((family == "solid.difference" || family.starts_with("xform.")) && first_empty)
        || (family == "solid.intersect" && inputs.iter().any(|input| input.mesh.is_empty()))
        || (family == "solid.union" && inputs.iter().all(|input| input.mesh.is_empty()))
    {
        return Ok(Value::nothing());
    }
    let nonempty: Vec<_> = inputs
        .iter()
        .filter(|input| !input.mesh.is_empty())
        .map(Rc::clone)
        .collect();
    let built = if matches!(family, "solid.extrude" | "solid.revolve") {
        profile_ops::to_solid(node, inputs)
    } else {
        solid(node, &nonempty, context)
    };
    let result = built.map_err(|error| kernel_fail(node, &error))?;
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
    let mut value = Value::solid(mesh, result.volume());
    // Moving, turning, scaling or combining keeps the first input's surface.
    if family.starts_with("xform.")
        || matches!(
            family,
            "solid.union" | "solid.difference" | "solid.intersect"
        )
    {
        value.material = inputs.first().and_then(|input| input.material.clone());
    }
    Ok(value)
}

/// A kernel refusal as a diagnostic naming the node; cancellation keeps its own code.
pub(crate) fn kernel_fail(node: &Node, error: &KernelError) -> Fail {
    Box::new(
        ve_graph::Diagnostic::error(
            if *error == KernelError::Cancelled {
                "eval.cancelled"
            } else {
                "kernel.invalid_input"
            },
            error.to_string(),
        )
        .at_node(&node.id),
    )
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
        "solid.cone" | "solid.torus" | "solid.capsule" => shapes::primitive(node),
        "xform.mirror" | "xform.array" | "xform.radial" | "xform.repeat" => {
            let input = inputs
                .first()
                .ok_or(KernelError::InvalidArgument("missing input"))?;
            shapes::pattern(node, input, context)
        }
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

pub(crate) fn vector(node: &Node, name: &str) -> Result<[f64; 3], KernelError> {
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
