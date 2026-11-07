//! Settings a limb chain carries besides its bones, and the `rig.aim` node: the analytic two-bone solver and the aim solver (see [`crate::ik_limb`]).
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use ve_graph::{Node, Value as Param};

use super::{IkChain, number, triple};
use crate::{Fail, cache::Value, fail};

/// How a `solver="two_bone"` chain bends: the pole direction (world; zero keeps the current side), the soft-landing amount and the twist.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TwoBoneSettings {
    /// The world direction the middle joint bends towards.
    pub pole: [f32; 3],
    /// 0 to 1: how much of the chain's length it eases in over near full stretch.
    pub soften: f32,
    /// Extra swing of the middle joint around the root-to-target line, in radians.
    pub twist: f32,
}

/// How a `rig.aim` bone looks at its target: the bone's own direction (head to tail at rest) points at the target and `up` leans towards `pole`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AimSettings {
    /// The bone's up direction at rest (world axes at rest), for example the top of a head.
    pub up: [f32; 3],
    /// The world direction `up` leans towards while looking. Zero keeps the current up.
    pub pole: [f32; 3],
    /// Extra roll around the line of sight, in radians.
    pub twist: f32,
    /// 0 to 1: how much of the turn to apply.
    pub weight: f32,
}

/// `rig.aim`: makes the input shape a TARGET that one bone part turns to look at (a head, an eye, a turret).
pub(crate) fn aim(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs.first().ok_or_else(|| {
        fail(
            "graph.missing_input",
            "rig.aim needs a shape to be the target",
        )
    })?;
    let Some(Param::Str(bone)) = node.params.get("bone") else {
        return Err(fail(
            "param.type",
            "rig.aim needs bone, the output name of the bone part",
        ));
    };
    let bone = bone.trim();
    if bone.is_empty() {
        return Err(fail("param.range", "rig.aim bone must name a part"));
    }
    if input.ik.is_some() {
        return Err(fail(
            "rig.duplicate",
            "a part can carry only one of rig.ik, rig.aim or rig.secondary",
        ));
    }
    let up = match triple(node, "up") {
        [0.0, 0.0, 0.0] => [0.0, 1.0, 0.0],
        given => given,
    };
    let mut value = (**input).clone();
    value.ik = Some(IkChain {
        bones: vec![bone.to_owned()],
        iterations: 1,
        segments: Vec::new(),
        secondary: None,
        gait: None,
        two_bone: None,
        aim: Some(AimSettings {
            up,
            pole: triple(node, "pole"),
            twist: number(node, "twist", 0.0).clamp(-6.3, 6.3),
            weight: number(node, "weight", 1.0).clamp(0.0, 1.0),
        }),
        limit_degrees: 180.0,
    });
    Ok(value)
}
