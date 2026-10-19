//! Material parsing at the projection boundary.
use super::{Color, VeMaterial};
pub(super) fn parse(text: &str, node: &str) -> Option<VeMaterial> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let triple = |name: &str, max: f64| -> Option<[f32; 3]> {
        let items = value.get(name)?.as_array()?;
        if items.len() != 3 {
            return None;
        }
        let mut out = [0.0; 3];
        for (slot, item) in out.iter_mut().zip(items) {
            let channel = item.as_f64().filter(|v| (0.0..=max).contains(v))?;
            #[expect(clippy::cast_possible_truncation, reason = "bounded material channels")]
            {
                *slot = channel as f32;
            }
        }
        Some(out)
    };
    let bounded = |name: &str, min: f64, max: f64| -> Option<f32> {
        let number = value
            .get(name)?
            .as_f64()
            .filter(|v| (min..=max).contains(v))?;
        #[expect(clippy::cast_possible_truncation, reason = "bounded material factors")]
        Some(number as f32)
    };
    let factor = |name: &str| bounded(name, 0.0, 1.0);
    let recipe = match value.get("recipe").and_then(serde_json::Value::as_str) {
        None | Some("none") => ve_render::ProceduralRecipe::None,
        Some("wood") => ve_render::ProceduralRecipe::Wood,
        Some("planks") => ve_render::ProceduralRecipe::Planks,
        Some("brushed_metal") => ve_render::ProceduralRecipe::BrushedMetal,
        Some("leather") => ve_render::ProceduralRecipe::Leather,
        Some("fabric") => ve_render::ProceduralRecipe::Fabric,
        Some("brick") => ve_render::ProceduralRecipe::Brick,
        Some("concrete") => ve_render::ProceduralRecipe::Concrete,
        Some("marble") => ve_render::ProceduralRecipe::Marble,
        Some("painted_wall") => ve_render::ProceduralRecipe::PaintedWall,
        Some("water") => ve_render::ProceduralRecipe::Water,
        Some(_) => return None,
    };
    let texture_scale = bounded("texture_scale", 0.1, 50.0).unwrap_or(1.0);
    let opacity = match value.get("opacity") {
        None => 1.0,
        Some(_) => bounded("opacity", 0.0, 1.0)?,
    };
    let color = triple("color", 1.0)?;
    let emissive = triple("emissive", 100.0)?;
    let toon = match value.get("toon") {
        None | Some(serde_json::Value::Null) => ve_render::ToonStyle::default(),
        Some(toon) => parse_toon(toon)?,
    };
    let foliage = match value.get("foliage") {
        None | Some(serde_json::Value::Null) => ve_render::FoliageStyle::default(),
        Some(foliage) => parse_foliage(foliage)?,
    };
    Some(VeMaterial {
        base_color: Color::srgb(color[0], color[1], color[2]),
        opacity,
        water_time: 0.0,
        sort_key: node.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        }),
        metallic: factor("metallic")?,
        roughness: factor("roughness")?,
        emissive: Color::linear_rgb(emissive[0], emissive[1], emissive[2]),
        recipe,
        texture_scale,
        map_layers: [-1; 4],
        toon,
        foliage,
    })
}

/// A material's `foliage` object: the cutoff, sway and flutter, each 0 to 1, or the whole material is refused.
fn parse_foliage(foliage: &serde_json::Value) -> Option<ve_render::FoliageStyle> {
    let number = |name: &str| -> Option<f32> {
        let value = foliage
            .get(name)?
            .as_f64()
            .filter(|v| (0.0..=1.0).contains(v))?;
        #[expect(clippy::cast_possible_truncation, reason = "bounded foliage factors")]
        Some(value as f32)
    };
    Some(ve_render::FoliageStyle {
        alpha_cutoff: number("alpha_cutoff")?,
        sway: number("sway")?,
        flutter: number("flutter")?,
    })
}

/// A material's `toon` object: bands 1 to 4 and every other number inside its range, or the whole material is refused.
fn parse_toon(toon: &serde_json::Value) -> Option<ve_render::ToonStyle> {
    let number = |name: &str, min: f64, max: f64| -> Option<f32> {
        let value = toon
            .get(name)?
            .as_f64()
            .filter(|v| (min..=max).contains(v))?;
        #[expect(clippy::cast_possible_truncation, reason = "bounded toon factors")]
        Some(value as f32)
    };
    let bands = u32::try_from(
        toon.get("bands")?
            .as_u64()
            .filter(|b| (1..=4).contains(b))?,
    )
    .ok()?;
    let colour = |name: &str| -> Option<Color> {
        let items = toon.get(name)?.as_array()?;
        if items.len() != 3 {
            return None;
        }
        let mut channels = [0.0_f32; 3];
        for (slot, item) in channels.iter_mut().zip(items) {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "colour channels are bounded to 0..=1"
            )]
            {
                *slot = item.as_f64().filter(|v| (0.0..=1.0).contains(v))? as f32;
            }
        }
        Some(Color::srgb(channels[0], channels[1], channels[2]))
    };
    // Older saved results have no outline.
    let outline_width = match toon.get("outline") {
        None => 0.0,
        Some(_) => number("outline", 0.0, 16.0)?,
    };
    let outline_color = match toon.get("outline_color") {
        None => ve_render::ToonStyle::default().outline_color,
        Some(_) => colour("outline_color")?,
    };
    Some(ve_render::ToonStyle {
        bands,
        feather: number("feather", 0.0, 1.0)?,
        shift: number("shift", -1.0, 1.0)?,
        spec: number("spec", 0.0, 1.0)?,
        shade_color: colour("shade_color")?,
        rim: number("rim", 0.0, 1.0)?,
        outline_width,
        outline_color,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = r#"{"color":[0.1,0.3,0.9],"metallic":0,"roughness":0.3,"emissive":[0,0,0]}"#;

    #[test]
    fn absent_opacity_is_opaque_and_authored_opacity_is_bounded() {
        assert_eq!(parse(BASE, "n_glass").unwrap().opacity, 1.0);
        let glass = BASE.replace("\"metallic\"", "\"opacity\":0.35,\"metallic\"");
        assert!((parse(&glass, "n_glass").unwrap().opacity - 0.35).abs() < 1e-6);
        for opacity in ["-0.1", "1.1", "null", "\"clear\""] {
            let invalid = BASE.replace(
                "\"metallic\"",
                &format!("\"opacity\":{opacity},\"metallic\""),
            );
            assert!(parse(&invalid, "n_glass").is_none());
        }
        assert_eq!(
            parse(BASE, "n_glass").unwrap().sort_key,
            parse(BASE, "n_glass").unwrap().sort_key
        );
    }
}
