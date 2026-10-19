//! `solid.foliage`: makes a surface behave like leaves, grass or a fence of thin cards. It is cut away where the albedo map's
//! alpha is under a threshold (so a leaf shape drawn into a texture becomes a leaf-shaped silhouette, in the picture, in
//! its shadow and in the exported glTF) and it moves in the wind (the whole shape bends, the surface itself trembles).
//!
//! Colours, roughness and texture programs are kept: put this after `solid.material` and `solid.texture`.
use crate::appearance::{textured_surface_default, with_surfaces};
use crate::result::Foliage;
use crate::{Fail, cache::Value, fail};
use std::rc::Rc;
use ve_graph::Node;

#[expect(clippy::cast_possible_truncation, reason = "the factors are 0 to 1")]
fn factor(node: &Node, name: &str, default: f32) -> f32 {
    node.params
        .get(name)
        .and_then(ve_graph::Value::as_f64)
        .map_or(default, |v| v as f32)
        .clamp(0.0, 1.0)
}

pub(crate) fn foliage(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "solid.foliage needs a solid"))?;
    let setting = Foliage {
        alpha_cutoff: factor(node, "alpha_cutoff", 0.5),
        sway: factor(node, "sway", 0.0),
        flutter: factor(node, "flutter", 0.0),
    };
    // All three at zero is the ordinary surface again.
    let setting = (setting.alpha_cutoff > 0.0 || setting.sway > 0.0 || setting.flutter > 0.0)
        .then_some(setting);
    Ok(with_surfaces(input, |existing| {
        let mut material = textured_surface_default(existing);
        material.foliage = setting;
        material
    }))
}
