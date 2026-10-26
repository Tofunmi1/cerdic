//! Tiled texture node specification.
use super::*;
pub(super) fn specification() -> Option<OperatorSpec> {
    spec(
        "solid.texture@1",
        "Attaches a procedural Material Maker texture program to a solid. The base material and geometry are preserved. \
                     By default the texture is laid out once over the part, which blurs on big surfaces (a wall, a floor, the ground); \
                     set tile to the size in metres of ONE repeat (0.5 to 4) and it repeats across the surface in world space at full \
                     sharpness, so write a pattern that tiles (bricks, noise at whole-number scales).",
        vec![single("in", COLOURABLE)],
        OutputSpec::SameAsInput("in"),
        vec![
            ParamSpec {
                name: "no_repeat",
                ty: ParamType::Bool,
                default: Some(Value::Bool(false)),
                doc: "Blend three shifted samples to hide tiled repetition; ignored at Low quality.",
            },
            number(
                "no_repeat_scale",
                Unit::None,
                (0.25, 8.0),
                1.0,
                "Grid cells per tile for non-repeating sampling; larger values make smaller patches.",
            ),
            ParamSpec {
                name: "program",
                ty: ParamType::Str { max_len: 8192 },
                default: None,
                doc: "Procedural texture DSL with albedo, roughness, metallic, height and emissive channels.",
            },
            ParamSpec {
                name: "tile",
                ty: ParamType::Num {
                    unit: Unit::Length,
                    min: 0.0,
                    max: 100.0,
                },
                default: Some(Value::Num(0.0)),
                doc: "0 lays the texture out once over the part. Above 0 it repeats every this many metres in world space (use 1 to 4 for ground, 0.5 to 2 for walls) so big surfaces stay sharp.",
            },
            ParamSpec {
                name: "size",
                ty: ParamType::Choice {
                    options: &["auto", "256", "512", "1024"],
                },
                default: Some(Value::Str("auto".to_owned())),
                doc: "Side in pixels of the maps the viewer bakes: auto is 1024. The viewer holds a fixed amount of texture memory, so small things (a leaf, a pebble, a distant prop) can say 256 or 512 and leave room for many more textured parts; when memory still runs out the viewer shrinks the biggest maps first and tells you. It does not change the exported file.",
            },
        ],
    )
}
