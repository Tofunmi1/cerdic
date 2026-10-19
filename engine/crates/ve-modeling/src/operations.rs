//! Operators consume immutable mesh values. Native solids exist only while one node executes.
use crate::regions::{self, Labels};
use crate::{Fail, Quality, appearance, cache::Value, fail, instances, profile_ops, shapes};
use std::rc::Rc;
use ve_graph::{Node, Value as Param};
use ve_kernel::{ExecutionContext, KernelError, Mesh, Solid};

pub(crate) fn execute(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
    quality: Quality,
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
        return Ok((**input).clone());
    }
    if let Some(result) = execute_special(node, inputs, context, quality) {
        return result;
    }
    let family = node.op.family().as_str();
    let first_empty = inputs.first().is_some_and(|input| input.mesh.is_empty());
    if ((family == "solid.difference" || family.starts_with("xform.")) && first_empty)
        || (family == "solid.intersect" && inputs.iter().any(|input| input.mesh.is_empty()))
        || (family == "solid.union" && inputs.iter().all(|input| input.mesh.is_empty()))
    {
        return inherited(node, inputs, Value::nothing());
    }
    let nonempty: Vec<_> = inputs
        .iter()
        .filter(|input| !input.mesh.is_empty())
        .map(Rc::clone)
        .collect();
    // Inputs of two or more different materials are labelled so the result keeps each one's surface.
    let mut labels = Labels::default();
    let tracking = tracks_materials(family) && regions::needed(&nonempty);
    let built = if matches!(family, "solid.extrude" | "solid.revolve") {
        profile_ops::to_solid(node, inputs)
    } else {
        solid(node, &nonempty, context, tracking.then_some(&mut labels))
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
    let mut value = inherited(node, inputs, Value::solid(mesh, result.volume()))?;
    if tracking && !result.is_empty() {
        let found = labels
            .regions(&result, value.material.as_ref())
            .map_err(|e| fail("kernel.failed", e.to_string()))?;
        regions::apply(&mut value, found);
    }
    Ok(value)
}

fn execute_special(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
    quality: Quality,
) -> Option<Result<Value, Fail>> {
    let family = node.op.family().as_str();
    if family == "frame.add" {
        let Some(input) = inputs.first() else {
            return Some(Err(fail("graph.missing_input", "frame source")));
        };
        return Some(crate::frames::add(node, input));
    }
    if family.starts_with("light.") {
        return Some(crate::lights::execute(node));
    }
    if let Some(result) = execute_scene_environment(node) {
        return Some(result);
    }
    if family == "xform.attach" {
        return Some(crate::frames::attach(node, inputs));
    }
    if family.starts_with("field.") {
        return Some(
            crate::field_ops::execute(family, node, inputs, context, quality)
                .map_err(|error| at_node(error, node)),
        );
    }
    if let Some(result) = crate::curves::run(node, inputs, context) {
        return Some(result.map_err(|error| at_node(error, node)));
    }
    if let Some(result) = crate::deform::run(node, inputs, context) {
        return Some(result.map_err(|error| at_node(error, node)));
    }
    if let Some(result) = crate::terrain::run(node, inputs, context) {
        return Some(result.map_err(|error| at_node(error, node)));
    }
    if let Some(result) = crate::terrain::water::run(node, inputs, context) {
        return Some(result.map_err(|error| at_node(error, node)));
    }
    if let Some(result) = crate::poly_ops::run(node, inputs, context) {
        return Some(result.map_err(|error| at_node(error, node)));
    }
    if let Some(result) = crate::hulls::run(node, inputs) {
        return Some(result.map_err(|error| at_node(error, node)));
    }
    if family == "solid.material" {
        return Some(appearance::material(node, inputs));
    }
    if family == "anim.track" {
        return Some(crate::anim::track(node, inputs));
    }
    if family == "phys.body" {
        return Some(crate::physics::body(node, inputs));
    }
    if family == "phys.world" {
        return Some(crate::physics::world(node, inputs));
    }
    if family == "phys.joint" {
        return Some(crate::physics::joint(node, inputs));
    }
    if let Some(result) = execute_rig(node, inputs) {
        return Some(result);
    }
    if family == "anim.wave" {
        return Some(crate::wave::wave(node, inputs));
    }
    if family == "anim.parent" {
        return Some(crate::hierarchy::parent(node, inputs));
    }
    if family == "behavior.on" {
        return Some(crate::reaction::behavior(node, inputs));
    }
    if family == "solid.texture" {
        return Some(appearance::texture(node, inputs));
    }
    if family == "solid.toon" {
        return Some(appearance::toon(node, inputs));
    }
    if family == "solid.style" {
        return Some(crate::style::style(node, inputs));
    }
    if family.starts_with("inst.") || inputs.first().is_some_and(|v| v.instances.is_some()) {
        return Some(
            instances::execute(node, inputs, context).map_err(|error| at_node(error, node)),
        );
    }
    if family.starts_with("profile.") || family == "check.min_wall" {
        return Some(profile_ops::run(node, inputs));
    }
    None
}

fn execute_scene_environment(node: &Node) -> Option<Result<Value, Fail>> {
    match node.op.family().as_str() {
        "scene.environment" => Some(crate::environment::execute(node)),
        "scene.sky" => Some(crate::environment::sky::execute(node)),
        _ => None,
    }
}

fn execute_rig(node: &Node, inputs: &[Rc<Value>]) -> Option<Result<Value, Fail>> {
    match node.op.family().as_str() {
        "rig.bone" => Some(crate::rig::bone(node, inputs)),
        "rig.ik" => Some(crate::rig::ik(node, inputs)),
        "rig.skin" => Some(crate::rig::skin(node, inputs)),
        _ => None,
    }
}

/// Operations that carry each input's material through to the faces of their result.
fn tracks_materials(family: &str) -> bool {
    matches!(
        family,
        "solid.union"
            | "solid.difference"
            | "solid.intersect"
            | "xform.move"
            | "xform.rotate"
            | "xform.scale"
            | "xform.align"
            | "xform.mirror"
            | "xform.array"
            | "xform.radial"
            | "xform.repeat"
    )
}

fn at_node(mut error: Fail, node: &Node) -> Fail {
    if error.node.is_none() {
        error.node = Some(node.id.clone());
    }
    error
}

pub(crate) fn inherited(
    node: &Node,
    inputs: &[Rc<Value>],
    mut value: Value,
) -> Result<Value, Fail> {
    let family = node.op.family().as_str();
    // Moving, turning, scaling or combining keeps the first input's surface.
    if family.starts_with("xform.")
        || matches!(
            family,
            "solid.union" | "solid.difference" | "solid.intersect"
        )
    {
        value.material = inputs.first().and_then(|input| input.material.clone());
        if let Some(input) = inputs.first() {
            value.frames = crate::frames::propagate(node, &input.frames)?;
        }
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
    mut labels: Option<&mut Labels>,
) -> Result<Solid, KernelError> {
    let kind = node.op.family().as_str();
    let mut convert = |input: &Value| match labels.as_deref_mut() {
        Some(labels) => labels.solid(input),
        None => Solid::from_mesh(&input.mesh),
    };
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
            shapes::pattern(node, input, context, labels)
        }
        _ => {
            if kind == "solid.union" && inputs.len() >= 2 {
                let solids: Vec<_> = inputs
                    .iter()
                    .map(|input| convert(input))
                    .collect::<Result<_, _>>()?;
                let borrowed: Vec<_> = solids.iter().collect();
                return Solid::union_all_with_context(&borrowed, context);
            }
            let input = inputs
                .first()
                .ok_or(KernelError::InvalidArgument("missing input"))?;
            let mut value = convert(input)?;
            match kind {
                "xform.move" => value.translate(vector(node, "by")?),
                "xform.rotate" => value.rotate(vector(node, "angles")?.map(f64::to_degrees)),
                "xform.scale" => value.scale(vector(node, "by")?),
                "xform.align" => value.transform(crate::assembly::alignment(node)?),
                "solid.round" => value.round(number("radius")?, segments()?),
                "solid.chamfer" => value.chamfer(number("width")?),
                _ => {
                    for input in inputs.iter().skip(1) {
                        let next = convert(input)?;
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
