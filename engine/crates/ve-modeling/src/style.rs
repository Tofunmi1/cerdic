//! `solid.style`: one word sets the whole look of a surface. A style is a bundle of the shading (cel bands or physically based), the
//! outline, and how baked texture maps are finished (painted over, pixelated, inked), so a scene, or one character in it, can
//! change look without touching its colours, textures or shape.
use crate::result::Toon;
use crate::{Fail, cache::Value, fail};
use std::rc::Rc;
use ve_graph::{Node, Value as Param};

/// The names `solid.style` accepts, in the order the node lists them.
pub(crate) const STYLES: [&str; 6] = ["anime", "realistic", "painterly", "flat", "retro", "ink"];

/// The shading and texture finishing of one style, before the outline scale.
struct Look {
    toon: Option<Toon>,
    texture_style: Option<&'static str>,
}

fn cel(
    bands: u32,
    feather: f32,
    shade_color: [f32; 3],
    rim: f32,
    outline: f32,
    outline_color: [f32; 3],
) -> Toon {
    Toon {
        bands,
        feather,
        shift: 0.0,
        spec: 0.0,
        shade_color,
        rim,
        outline,
        outline_color,
    }
}

/// What a style does. `realistic` is the absence of cel shading, so it also takes an earlier style away.
fn look(name: &str) -> Option<Look> {
    let (toon, texture_style) = match name {
        "anime" => (
            Some(cel(
                2,
                0.06,
                [0.62, 0.58, 0.82],
                0.25,
                2.0,
                [0.1, 0.06, 0.14],
            )),
            None,
        ),
        "realistic" => (None, None),
        "painterly" => (
            Some(cel(3, 0.4, [0.78, 0.74, 0.88], 0.1, 0.0, [0.1, 0.06, 0.14])),
            Some("painterly"),
        ),
        "flat" => (
            Some(cel(1, 0.0, [1.0, 1.0, 1.0], 0.0, 1.5, [0.08, 0.06, 0.1])),
            None,
        ),
        "retro" => (
            Some(cel(3, 0.0, [0.55, 0.55, 0.75], 0.0, 1.5, [0.05, 0.05, 0.1])),
            Some("retro"),
        ),
        "ink" => (
            Some(cel(
                2,
                0.0,
                [0.45, 0.45, 0.45],
                0.0,
                3.0,
                [0.02, 0.02, 0.03],
            )),
            Some("ink"),
        ),
        _ => return None,
    };
    Some(Look {
        toon,
        texture_style,
    })
}

pub(crate) fn style(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "solid.style needs a solid"))?;
    let name = match node.params.get("style") {
        Some(Param::Str(name)) => name.as_str(),
        None => "anime",
        _ => return Err(fail("param.type", "solid.style style must be a name")),
    };
    let Some(mut look) = look(name) else {
        return Err(fail(
            "param.range",
            format!(
                "solid.style style `{name}` is not one of {}",
                STYLES.join(", ")
            ),
        ));
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the outline scale is 0 to 4"
    )]
    let scale = node
        .params
        .get("outline_scale")
        .and_then(Param::as_f64)
        .map_or(1.0, |v| v as f32)
        .clamp(0.0, 4.0);
    if let Some(toon) = &mut look.toon {
        toon.outline = (toon.outline * scale).clamp(0.0, 16.0);
    }
    Ok(crate::appearance::restyle(
        input,
        look.toon,
        look.texture_style,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_style_has_a_look() {
        for name in STYLES {
            assert!(look(name).is_some(), "{name}");
        }
        assert!(look("watercolour").is_none());
    }

    #[test]
    fn only_realistic_has_no_cel_shading() {
        let plain: Vec<&str> = STYLES
            .into_iter()
            .filter(|name| look(name).is_some_and(|l| l.toon.is_none()))
            .collect();
        assert_eq!(plain, ["realistic"]);
    }
}
