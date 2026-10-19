
/// Adds a joint to the body on the input.
pub(crate) fn joint(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "phys.joint needs a shape"))?;
    let Some(body) = &input.body else {
        return Err(fail(
            "phys.joint_needs_body",
            "put phys.joint after phys.body on the same part: the joint connects that body",
        ));
    };
    if body.joints.len() >= MAX_JOINTS_PER_PART {
        return Err(fail(
            "param.range",
            format!("a part carries at most {MAX_JOINTS_PER_PART} joints"),
        ));
    }
    let kind = choice(
        node,
        "kind",
        &[
            ("fixed", JointKind::Fixed),
            ("hinge", JointKind::Hinge),
            ("slider", JointKind::Slider),
            ("ball", JointKind::Ball),
            ("spring", JointKind::Spring),
        ],
        JointKind::Hinge,
    );
    let to = match node.params.get("to") {
        Some(Param::Str(name)) => name.trim().to_owned(),
        _ => String::new(),
    };
    let anchor = triple(node, "anchor", [0.0; 3]);
    let axis = triple(node, "axis", [0.0, 1.0, 0.0]);
    if matches!(kind, JointKind::Hinge | JointKind::Slider) && axis.iter().all(|v| v.abs() < 1.0e-9)
    {
        return Err(fail(
            "param.range",
            "a hinge or slider needs a non-zero axis",
        ));
    }
    let limited = matches!(node.params.get("limited"), Some(Param::Bool(true)));
    let (low, high) = (number(node, "low", 0.0), number(node, "high", 0.0));
    if limited && low > high {
        return Err(fail("param.range", "joint low must not be above high"));
    }
    let mut value = (**input).clone();
    if let Some(body) = &mut value.body {
        body.joints.push(PhysJoint {
            to,
            kind,
            anchor,
            anchor_b: triple(node, "anchor_b", anchor),
            axis,
            limited,
            low,
            high,
            stiffness: number(node, "stiffness", 20.0),
            damping: number(node, "damping", 1.0),
            motor_speed: number(node, "motor_speed", 0.0),
            motor_force: number(node, "motor_force", 0.0),
            rest_length: number(node, "rest_length", -1.0),
        });
    }
    Ok(value)
}

/// Drops bodies that cannot work (on parts that move on their own, or beyond the scene's cap), each with a warning, so the
/// viewer only ever sees usable ones.
pub(crate) fn check(parts: &mut [Part]) -> Vec<EvaluationWarning> {
    let mut warnings = Vec::new();
    let mut kept = 0;
    for part in parts.iter_mut() {
        if part.body.is_none() {
            continue;
        }
        let reason = if part.transforms.is_some() {
            Some("an instance set cannot be a physics body; make each copy its own part")
        } else if !part.animation.is_empty()
            || part.parent.is_some()
            || part.skin.is_some()
            || part.bone.is_some()
            || part.ik.is_some()
        {
            Some(
                "a part with its own tracks, parent, bones or skin moves by animation; use phys.body only on parts that are not animated (a kinematic body is moved by hand or by play)",
            )
        } else if part.positions.is_empty() {
            Some("it has no geometry")
        } else if kept >= MAX_BODIES {
            Some("a scene may have at most 256 physics bodies")
        } else {
            None
        };
        match reason {
            Some(text) => {
                warnings.push(EvaluationWarning {
                    code: "phys.body_dropped".to_owned(),
                    node: part.node.to_string(),
                    message: format!("{} is not simulated: {text}", part.name),
                });
                part.body = None;
            }
            None => kept += 1,
        }
    }
    // One set of scene settings: the first body that carries them keeps them.
    let mut settled = false;
    for part in parts.iter_mut() {
        let (name, node) = (part.name.clone(), part.node.to_string());
        let Some(body) = &mut part.body else {
            continue;
        };
        if body.world.is_some() && settled {
            body.world = None;
            warnings.push(EvaluationWarning {
                code: "phys.world_duplicate".to_owned(),
                node,
                message: format!(
                    "{name} has phys.world, but the scene already has one; only the first is used"
                ),
            });
        }
        settled |= body.world.is_some();
    }
    // A joint can only connect to another part that is itself a body (or to the world).
    let bodies: std::collections::HashSet<String> = parts
        .iter()
        .filter(|part| part.body.is_some())
        .map(|part| part.name.clone())
        .collect();
    for part in parts.iter_mut() {
        let (name, node) = (part.name.clone(), part.node.to_string());
        let Some(body) = &mut part.body else {
            continue;
        };
        body.joints.retain(|joint| {
            let reason = if joint.to == name {
                Some("it names its own part")
            } else if !joint.to.is_empty() && !bodies.contains(&joint.to) {
                Some("no output part with a phys.body has that name")
            } else {
                None
            };
            if let Some(reason) = reason {
                warnings.push(EvaluationWarning {
                    code: "phys.joint_dropped".to_owned(),
                    node: node.clone(),
                    message: format!(
                        "{name} has a joint to \"{}\" that is ignored: {reason}",
                        joint.to
                    ),
                });
            }
            reason.is_none()
        });
    }
    warnings
}
