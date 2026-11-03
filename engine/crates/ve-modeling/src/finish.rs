//! `solid.finish`: layers on top of the base surface that the physically based renderer draws: a glossy clear coat (car paint, wet skin,
//! glazed ceramic), highlights stretched along the surface (brushed metal, silk, hair) and light passing through a thin body (leaves,
//! wax, ears). Colours, roughness, textures and the other surface settings are kept: put it after `solid.material` and `solid.texture`.
use crate::appearance::{textured_surface_default, with_surfaces};
use crate::{Fail, cache::Value, fail};
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use ve_graph::Node;

/// The layers a `solid.finish` node puts on a surface, each 0 to 1 unless noted.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finish {
    /// Strength of a glossy clear layer.
    pub clearcoat: f32,
    /// Roughness of that layer.
    pub clearcoat_roughness: f32,
    /// How much highlights stretch along the surface.
    pub anisotropy: f32,
    /// Share of diffuse light that passes through.
    pub translucency: f32,
    /// Steepness of procedural pixel-shader relief.
    pub relief: f32,
    /// Relief features per metre.
    pub relief_scale: f32,
    /// 0 smooth, 1 ridged, 2 scales.
    pub relief_style: f32,
    /// Wet look.
    pub wetness: f32,
    /// Angle-dependent colour shift.
    pub iridescence: f32,
    /// Grazing-angle softness.
    pub sheen: f32,
    /// Subsurface scattering.
    pub subsurface: f32,
    /// Sunlight passing through a thin body, glowing on the side that faces the viewer when the sun is behind it.
    #[serde(default)]
    pub thickness: f32,
    /// A terrain surface from a `terrain.material` node; `None` for every other surface.
    #[serde(default)]
    pub terrain: Option<crate::terrain_look::TerrainLook>,
    /// A water surface from a `river.water` node; `None` for every other surface.
    #[serde(default)]
    pub water: Option<crate::water_look::WaterLook>,
}

impl Finish {
    /// The finish with every layer off: the ordinary surface (a relief scale of 30 is only the renderer's accepted default).
    pub(crate) const fn plain() -> Self {
        Self {
            clearcoat: 0.0,
            clearcoat_roughness: 0.1,
            anisotropy: 0.0,
            translucency: 0.0,
            relief: 0.0,
            relief_scale: 30.0,
            relief_style: 0.0,
            wetness: 0.0,
            iridescence: 0.0,
            sheen: 0.0,
            subsurface: 0.0,
            thickness: 0.0,
            terrain: None,
            water: None,
        }
    }
}

#[expect(clippy::cast_possible_truncation, reason = "the factors are 0 to 1")]
fn factor(node: &Node, name: &str, default: f32) -> f32 {
    node.params
        .get(name)
        .and_then(ve_graph::Value::as_f64)
        .map_or(default, |v| v as f32)
        .clamp(0.0, 1.0)
}

pub(crate) fn finish(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "solid.finish needs a solid"))?;
    let setting = Finish {
        clearcoat: factor(node, "clearcoat", 0.0),
        clearcoat_roughness: factor(node, "clearcoat_roughness", 0.1),
        anisotropy: factor(node, "anisotropy", 0.0),
        translucency: factor(node, "translucency", 0.0),
        relief: factor(node, "relief", 0.0),
        wetness: factor(node, "wetness", 0.0),
        iridescence: factor(node, "iridescence", 0.0),
        sheen: factor(node, "sheen", 0.0),
        subsurface: factor(node, "subsurface", 0.0),
        thickness: factor(node, "thickness", 0.0),
        relief_scale: node
            .params
            .get("relief_scale")
            .and_then(ve_graph::Value::as_f64)
            .map_or(30.0, |v| {
                #[expect(clippy::cast_possible_truncation, reason = "bounded relief scale")]
                let scale = v as f32;
                scale
            })
            .clamp(0.1, 500.0),
        relief_style: match node.params.get("relief_style") {
            Some(ve_graph::Value::Str(style)) if style == "ridged" => 1.0,
            Some(ve_graph::Value::Str(style)) if style == "scales" => 2.0,
            _ => 0.0,
        },
        terrain: None,
        water: None,
    };
    // No layer at all is the plain surface again.
    let setting = (setting.clearcoat > 0.0
        || setting.anisotropy > 0.0
        || setting.translucency > 0.0
        || setting.relief > 0.0
        || setting.wetness > 0.0
        || setting.iridescence > 0.0
        || setting.sheen > 0.0
        || setting.subsurface > 0.0
        || setting.thickness > 0.0)
        .then_some(setting);
    Ok(with_surfaces(input, |existing| {
        let mut material = textured_surface_default(existing);
        // A terrain or water look already on the shape stays: this node adds layers on top of it.
        let kept = existing.and_then(|m| m.finish);
        let (terrain, water) = (kept.and_then(|f| f.terrain), kept.and_then(|f| f.water));
        material.finish = match (setting, terrain.is_some() || water.is_some()) {
            (Some(layers), _) => Some(Finish {
                terrain,
                water,
                ..layers
            }),
            (None, true) => Some(Finish {
                terrain,
                water,
                ..Finish::plain()
            }),
            (None, false) => None,
        };
        material
    }))
}
