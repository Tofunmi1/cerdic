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
    let finish = match value.get("finish") {
        None | Some(serde_json::Value::Null) => ve_render::SurfaceFinish::default(),
        Some(finish) => parse_finish(finish)?,
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
        no_repeat: match value.get("no_repeat") {
            None => false,
            Some(flag) => flag.as_bool()?,
        },
        no_repeat_scale: match value.get("no_repeat_scale") {
            None => 1.0,
            Some(_) => bounded("no_repeat_scale", 0.25, 8.0)?,
        },
        map_layers: [-1; 4],
        toon,
        foliage,
        finish,
    })
}

/// A material's `finish` object: clearcoat, clearcoat roughness, anisotropy and translucency, each 0 to 1, or the whole material is refused.
fn parse_finish(finish: &serde_json::Value) -> Option<ve_render::SurfaceFinish> {
    let number = |name: &str| -> Option<f32> {
        let value = finish
            .get(name)?
            .as_f64()
            .filter(|v| (0.0..=1.0).contains(v))?;
        #[expect(clippy::cast_possible_truncation, reason = "bounded finish factors")]
        Some(value as f32)
    };
    Some(ve_render::SurfaceFinish {
        // Authored materials are never particles: the effects system makes its own.
        particle: 0,
        clearcoat: number("clearcoat")?,
        clearcoat_roughness: number("clearcoat_roughness")?,
        anisotropy: number("anisotropy")?,
        translucency: number("translucency")?,
        relief: number("relief")?,
        wetness: number("wetness")?,
        iridescence: number("iridescence")?,
        sheen: number("sheen")?,
        subsurface: number("subsurface")?,
        // Absent in a finish written before the field existed.
        thickness: finish
            .get("thickness")
            .and_then(serde_json::Value::as_f64)
            .map_or(0.0, |v| {
                #[expect(clippy::cast_possible_truncation, reason = "bounded finish factor")]
                let value = v.clamp(0.0, 1.0) as f32;
                value
            }),
        relief_scale: finish
            .get("relief_scale")?
            .as_f64()
            .filter(|v| (0.1..=500.0).contains(v))
            .map(|v| {
                #[expect(clippy::cast_possible_truncation, reason = "bounded relief scale")]
                let scale = v as f32;
                scale
            })?,
        relief_style: finish
            .get("relief_style")?
            .as_f64()
            .filter(|v| (0.0..=2.0).contains(v))
            .map(|v| {
                #[expect(clippy::cast_possible_truncation, reason = "bounded relief style")]
                let style = v as f32;
                style
            })?,
        terrain: match finish.get("terrain") {
            None | Some(serde_json::Value::Null) => None,
            Some(terrain) => Some(parse_terrain(terrain)?),
        },
        water: match finish.get("water") {
            None | Some(serde_json::Value::Null) => None,
            Some(water) => Some(parse_water(water)?),
        },
    })
}

/// A material's `finish.water` object (shallow and deep colours, clarity, flow, ripple, foam), or the whole material is refused.
#[expect(clippy::cast_possible_truncation, reason = "bounded water parameters")]
fn parse_water(water: &serde_json::Value) -> Option<ve_render::SurfaceWater> {
    let colour = |name: &str| -> Option<[f32; 3]> {
        let values: Option<Vec<f32>> = water
            .get(name)?
            .as_array()?
            .iter()
            .map(|v| {
                v.as_f64()
                    .filter(|v| (0.0..=1.0).contains(v))
                    .map(|v| v as f32)
            })
            .collect();
        values?.try_into().ok()
    };
    let number = |name: &str, lo: f64, hi: f64| -> Option<f32> {
        water
            .get(name)?
            .as_f64()
            .filter(|v| (lo..=hi).contains(v))
            .map(|v| v as f32)
    };
    Some(ve_render::SurfaceWater {
        volumetric: water
            .get("volumetric")
            .map_or(Some(false), serde_json::Value::as_bool)?,
        sim: water
            .get("sim")
            .map_or(Some(false), serde_json::Value::as_bool)?,
        sim_resolution: match water.get("sim_resolution") {
            None => 128,
            Some(v) => u32::try_from(v.as_u64().filter(|n| (32..=256).contains(n))?).ok()?,
        },
        sim_damping: if water.get("sim_damping").is_some() {
            number("sim_damping", 0.0, 5.0)?
        } else {
            0.6
        },
        sim_depth: if water.get("sim_depth").is_some() {
            number("sim_depth", 0.05, 50.0)?
        } else {
            1.0
        },
        rain: if water.get("rain").is_some() {
            number("rain", 0.0, 1.0)?
        } else {
            0.0
        },
        sim_id: 0,
        sim_bounds: [0.0; 4],
        shallow: colour("shallow")?,
        deep: colour("deep")?,
        clarity: number("clarity", 0.1, 100.0)?,
        flow: number("flow", 0.0, 4.0)?,
        ripple: number("ripple", 0.0, 2.0)?,
        direction: match water.get("direction") {
            None => 0.0,
            Some(_) => number("direction", -360.0, 360.0)?,
        },
        seed: match water.get("seed") {
            None => 0,
            Some(seed) => u32::try_from(seed.as_u64()?).ok()?,
        },
        foam: number("foam", 0.0, 1.0)?,
    })
}

/// A material's `finish.terrain` object (grass, rock, snow and sand colours and thresholds), or the whole material is refused.
fn parse_terrain(terrain: &serde_json::Value) -> Option<ve_render::SurfaceTerrain> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "bounded terrain parameters"
    )]
    let numbers = |name: &str, count: usize, lo: f64, hi: f64| -> Option<Vec<f32>> {
        let items = terrain.get(name)?.as_array()?;
        (items.len() == count)
            .then(|| {
                items
                    .iter()
                    .map(|v| {
                        v.as_f64()
                            .filter(|v| (lo..=hi).contains(v))
                            .map(|v| v as f32)
                    })
                    .collect()
            })
            .flatten()
    };
    let colors = terrain.get("colors")?.as_array()?;
    if colors.len() != 4 {
        return None;
    }
    let mut out = ve_render::SurfaceTerrain::default();
    for (slot, color) in out.colors.iter_mut().zip(colors) {
        let values: Option<Vec<f32>> = color
            .as_array()?
            .iter()
            .map(|v| {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "colour channels are 0 to 1"
                )]
                v.as_f64()
                    .filter(|v| (0.0..=1.0).contains(v))
                    .map(|v| v as f32)
            })
            .collect();
        *slot = values?.try_into().ok()?;
    }
    out.roughness = numbers("roughness", 4, 0.0, 1.0)?.try_into().ok()?;
    out.rock = numbers("rock", 2, 0.0, 1.0)?.try_into().ok()?;
    out.snow = numbers("snow", 2, -1.0e5, 1.0e5)?.try_into().ok()?;
    out.sand = numbers("sand", 2, -1.0e5, 1.0e5)?.try_into().ok()?;
    out.relief = numbers("relief", 2, 0.0, 500.0)?.try_into().ok()?;
    out.variation = terrain
        .get("variation")?
        .as_f64()
        .filter(|v| (0.0..=1.0).contains(v))? as f32;
    Some(out)
}

/// Foliage JSON uses serde defaults for old documents and refuses out-of-range authored settings.
fn parse_foliage(value: &serde_json::Value) -> Option<ve_render::FoliageStyle> {
    let f: ve_render::FoliageStyle = serde_json::from_value(value.clone()).ok()?;
    let factors = [
        f.alpha_cutoff,
        f.sway,
        f.flutter,
        f.ground_blend,
        f.translucency,
    ];
    (factors
        .into_iter()
        .chain(f.ground_color)
        .all(|v| (0.0..=1.0).contains(&v))
        && [f.fade_near, f.fade_far]
            .into_iter()
            .all(|v| (0.0..=100000.0).contains(&v))
        && (0.0..=10.0).contains(&f.exposure_boost))
    .then_some(f)
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
