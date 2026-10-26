//! `terrain.material`: the look of a landscape (grass, rock, snow, sand) drawn in the pixel shader from the ground's slope and height
//! (ADR 0057). It sets no texture: the colours and thresholds travel with the surface and the viewer blends them per pixel.
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use ve_graph::{Node, Value as Param};

use crate::appearance::{textured_surface_default, with_surfaces};
use crate::finish::Finish;
use crate::{Fail, cache::Value, fail};

/// The layers of a terrain surface. Colours are sRGB; heights are world metres; slopes run from 0 (flat) to 1 (vertical).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerrainLook {
    /// Colours of grass, rock, snow and sand.
    pub colors: [[f32; 3]; 4],
    /// Roughness of grass, rock, snow and sand.
    pub roughness: [f32; 4],
    /// Slope at which rock takes over, and the width of that edge.
    pub rock: [f32; 2],
    /// Height of the snow line and the width of its edge.
    pub snow: [f32; 2],
    /// Height of the sand line and the width of its edge.
    pub sand: [f32; 2],
    /// Large-scale variation, 0 to 1.
    pub variation: f32,
    /// Bump strength 0 to 1, and its features per metre.
    pub relief: [f32; 2],
}

pub(crate) fn run(node: &Node, inputs: &[Rc<Value>]) -> Option<Result<Value, Fail>> {
    (node.op.family().as_str() == "terrain.material").then(|| build(node, inputs))
}

fn number(node: &Node, name: &str) -> Result<f32, Fail> {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .map(|v| {
            #[expect(clippy::cast_possible_truncation, reason = "bounded parameters")]
            let value = v as f32;
            value
        })
        .ok_or_else(|| {
            fail(
                "param.type",
                format!("terrain.material {name} is missing or malformed"),
            )
        })
}

fn colour(node: &Node, name: &str) -> Result<[f32; 3], Fail> {
    let Some(Param::List(items)) = node.params.get(name) else {
        return Err(fail(
            "param.type",
            format!("terrain.material {name} must be three numbers"),
        ));
    };
    let values: Vec<f64> = items.iter().filter_map(Param::as_f64).collect();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "colour channels are 0 to 1"
    )]
    match values.as_slice() {
        [r, g, b] => Ok([*r as f32, *g as f32, *b as f32]),
        _ => Err(fail(
            "param.type",
            format!("terrain.material {name} must be three numbers"),
        )),
    }
}

fn build(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs.first().ok_or_else(|| {
        fail(
            "graph.missing_input",
            "terrain.material needs a surface (a terrain.tile)",
        )
    })?;
    // Degrees in the language, a fraction of a right angle in the shader (the same measure as the `slope` mask).
    let slope = |name: &str| number(node, name).map(|degrees| degrees / 90.0);
    let look = TerrainLook {
        colors: [
            colour(node, "grass")?,
            colour(node, "rock")?,
            colour(node, "snow")?,
            colour(node, "sand")?,
        ],
        roughness: [0.95, 0.9, 0.7, 0.95],
        rock: [slope("rock_slope")?, slope("rock_blend")?.max(0.005)],
        snow: [
            number(node, "snow_line")?,
            number(node, "snow_blend")?.max(0.1),
        ],
        sand: [
            number(node, "sand_line")?,
            number(node, "sand_blend")?.max(0.1),
        ],
        variation: number(node, "variation")?.clamp(0.0, 1.0),
        relief: [
            number(node, "relief")?.clamp(0.0, 1.0),
            number(node, "relief_scale")?.clamp(0.1, 500.0),
        ],
    };
    let no_repeat = !matches!(node.params.get("no_repeat"), Some(Param::Bool(false)));
    let no_repeat_scale = number(node, "no_repeat_scale").unwrap_or(1.0);
    if !(0.25..=8.0).contains(&no_repeat_scale) {
        return Err(fail(
            "param.range",
            "no_repeat_scale must be between 0.25 and 8",
        ));
    }
    Ok(with_surfaces(input, |existing| {
        let mut material = textured_surface_default(existing);
        material.no_repeat = no_repeat;
        material.no_repeat_scale = no_repeat_scale;
        material.finish = Some(Finish {
            terrain: Some(look),
            ..Finish::plain()
        });
        material
    }))
}
