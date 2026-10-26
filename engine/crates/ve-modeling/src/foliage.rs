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
    let defaults = Foliage::default();
    let ground_color = match node.params.get("ground_color") {
        Some(ve_graph::Value::List(items)) if items.len() == 3 => std::array::from_fn(|i| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "colour channels are bounded by the operator schema"
            )]
            {
                items[i]
                    .as_f64()
                    .map_or(defaults.ground_color[i], |v| v as f32)
                    .clamp(0.0, 1.0)
            }
        }),
        _ => defaults.ground_color,
    };
    let setting = Foliage {
        alpha_cutoff: factor(node, "alpha_cutoff", 0.5),
        sway: factor(node, "sway", 0.0),
        flutter: factor(node, "flutter", 0.0),
        grass: matches!(node.params.get("grass"), Some(ve_graph::Value::Bool(true))),
        ground_color,
        ground_blend: factor(node, "ground_blend", 0.0),
        fade_near: number(node, "fade_near", 25.0),
        fade_far: number(node, "fade_far", 70.0),
        translucency: factor(node, "translucency", 0.0),
        exposure_boost: number(node, "exposure_boost", 1.0),
    };
    // All three at zero is the ordinary surface again.
    let setting = (setting.grass
        || setting.alpha_cutoff > 0.0
        || setting.sway > 0.0
        || setting.flutter > 0.0)
        .then_some(setting);
    Ok(with_surfaces(input, |existing| {
        let mut material = textured_surface_default(existing);
        material.foliage = setting;
        material
    }))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "finite schema-bounded parameters fit f32"
)]
fn number(node: &Node, name: &str, default: f32) -> f32 {
    node.params
        .get(name)
        .and_then(ve_graph::Value::as_f64)
        .map_or(default, |v| v as f32)
}
