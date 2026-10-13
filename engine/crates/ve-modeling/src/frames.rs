//! Named proper rigid sockets carried by immutable values, never by group interfaces.
use crate::{
    Fail,
    cache::Value,
    fail,
    operations::{kernel_fail, vector},
};
use glam::{DMat4, DQuat, DVec3};
use serde::Serialize;
use std::{collections::BTreeMap, rc::Rc};
use ve_graph::{Diagnostic, Node, Value as Param};
use ve_kernel::Solid;

mod preflight;
mod propagation;
pub(crate) use preflight::estimate;
pub(crate) use propagation::{copies, placed, propagate, reflected};
pub(crate) const MAX_FRAMES: usize = 128;

/// A right-handed rigid socket in the output's coordinate space (prototype space for sets).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Frame {
    /// Position in metres.
    pub position: [f64; 3],
    /// Unit orientation quaternion, ordered X, Y, Z, W.
    pub rotation: [f64; 4],
}

impl Frame {
    pub(crate) fn matrix(&self) -> DMat4 {
        DMat4::from_rotation_translation(
            DQuat::from_array(self.rotation),
            DVec3::from_array(self.position),
        )
    }

    pub(crate) fn from_matrix(matrix: DMat4) -> Result<Self, Fail> {
        let position = matrix.w_axis.truncate();
        let rotation = propagation::orientation(matrix)?;
        if !position.is_finite() || position.abs().max_element() > ve_kernel::MAX_EXTENT {
            return Err(fail(
                "socket.invalid_frame",
                "socket position exceeds coordinate limits",
            ));
        }
        Ok(Self {
            position: position.to_array(),
            rotation: rotation.to_array(),
        })
    }
}

pub(crate) fn text<'a>(node: &'a Node, name: &str) -> Result<&'a str, Fail> {
    match node.params.get(name) {
        Some(Param::Str(value)) => Ok(value),
        _ => Err(fail("param.type", format!("{name} requires text"))),
    }
}

pub(crate) fn add(node: &Node, input: &Value) -> Result<Value, Fail> {
    let name = text(node, "name")?;
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        return Err(Box::new(
            Diagnostic::error(
                "socket.name",
                "socket name needs letters, digits, _, - or .",
            )
            .at_node(&node.id),
        ));
    }
    if input.frames.contains_key(name) {
        return Err(Box::new(
            Diagnostic::error(
                "socket.duplicate",
                format!("socket {name:?} already exists"),
            )
            .at_node(&node.id),
        ));
    }
    let position = vector(node, "position").map_err(|e| kernel_fail(node, &e))?;
    let rotation =
        crate::instances::rotation(vector(node, "rotation").map_err(|e| kernel_fail(node, &e))?);
    let frame =
        Frame::from_matrix(DMat4::from_translation(DVec3::from_array(position)) * rotation)?;
    let mut value = input.clone();
    value.frames.insert(name.to_owned(), frame);
    if value.frames.len() > MAX_FRAMES {
        return Err(crate::budget::exceeded(
            "budget.sockets",
            MAX_FRAMES,
            value.frames.len(),
        ));
    }
    Ok(value)
}

pub(crate) fn attachment(node: &Node, inputs: &[Rc<Value>]) -> Result<DMat4, Fail> {
    let get = |port: &str, param: &str| {
        let name = text(node, param)?;
        let index = usize::from(port == "to");
        let input = inputs
            .get(index)
            .ok_or_else(|| fail("graph.missing_input", port))?;
        input.frames.get(name).map(Frame::matrix).ok_or_else(|| {
            let available = input.frames.keys().cloned().collect::<Vec<_>>().join(", ");
            Box::new(
                Diagnostic::error(
                    "socket.unknown",
                    format!("socket {name:?} is missing on {port}"),
                )
                .at_node(&node.id)
                .at_port(port)
                .with_related(
                    node.inputs
                        .values()
                        .flatten()
                        .map(|r| r.node.clone())
                        .collect(),
                )
                .suggest(format!("Available sockets on {port}: {available}")),
            )
        })
    };
    Ok(get("to", "to_socket")? * get("in", "socket")?.inverse())
}

pub(crate) fn attach(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let matrix = attachment(node, inputs)?;
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "attach source"))?;
    let solid = Solid::from_mesh(&input.mesh)
        .map_err(|e| kernel_fail(node, &e))?
        .transform(crate::assembly::rows(matrix))
        .map_err(|e| kernel_fail(node, &e))?;
    let mut value = Value::solid(
        solid.to_mesh().map_err(|e| kernel_fail(node, &e))?,
        solid.volume(),
    );
    value.material.clone_from(&input.material);
    value.frames = placed(&input.frames, matrix)?;
    Ok(value)
}

pub(crate) type Frames = BTreeMap<String, Frame>;
