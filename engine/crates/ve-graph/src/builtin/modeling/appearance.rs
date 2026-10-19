use super::{COLOURABLE, number, single, spec, triple};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec};
use crate::value::{ParamType, Unit, Value};

pub(super) fn specs() -> Vec<Option<OperatorSpec>> {
    vec![
        spec(
            "solid.material@1",
            "Gives a solid, a set of placements or a polygon mesh a surface: colour, metal, roughness and glow. The shape is unchanged.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                ParamSpec {
                    name: "color",
                    ty: ParamType::Vec3 {
                        unit: Unit::None,
                        min: 0.0,
                        max: 1.0,
                    },
                    default: Some(triple(0.8, 0.8, 0.8)),
                    doc: "Base colour [r, g, b], each 0 to 1, as picked in a colour chooser (sRGB).",
                },
                number(
                    "metallic",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "0 for plastic, wood, stone, paint; 1 for bare metal.",
                ),
                number(
                    "roughness",
                    Unit::None,
                    (0.0, 1.0),
                    0.5,
                    "0 is a mirror, 1 is fully matte.",
                ),
                number(
                    "opacity",
                    Unit::None,
                    (0.0, 1.0),
                    1.0,
                    "Surface opacity: 0 is clear, 0.3 is tinted glass, 0.6 is murky water, 1 is solid.",
                ),
                ParamSpec {
                    name: "emissive",
                    ty: ParamType::Vec3 {
                        unit: Unit::None,
                        min: 0.0,
                        max: 100.0,
                    },
                    default: Some(triple(0.0, 0.0, 0.0)),
                    doc: "Light given off, [r, g, b] in linear colour; values above 1 glow (a flame is about [3, 1.6, 0.4]).",
                },
                ParamSpec {
                    name: "emits_light",
                    ty: ParamType::Bool,
                    default: Some(Value::Bool(false)),
                    doc: "Create a bounded point light from this material's emissive colour and surface area.",
                },
                ParamSpec {
                    name: "recipe",
                    ty: ParamType::Choice {
                        options: &[
                            "none",
                            "wood",
                            "planks",
                            "brushed_metal",
                            "leather",
                            "fabric",
                            "brick",
                            "concrete",
                            "marble",
                            "painted_wall",
                        ],
                    },
                    default: Some(Value::Str("none".to_owned())),
                    doc: "Bounded object-space procedural surface pattern; no external textures are loaded.",
                },
                number(
                    "texture_scale",
                    Unit::None,
                    (0.1, 50.0),
                    1.0,
                    "Object-space pattern frequency multiplier; larger values repeat more often.",
                ),
            ],
        ),
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
        ),
        spec(
            "solid.toon@1",
            "Gives a surface an anime / cel-shaded look. Light falls into flat colour bands (with a tinted shadow colour) instead of \
                     smooth shading. Put it after solid.material (and solid.texture): the colour, roughness and textures are kept and become the \
                     lit colour. Use bands=1 for a hard two-tone look, 2 for lit / mid / shadow. rim brightens the edge facing away from the \
                     viewer on the sunlit side, spec adds a small hard highlight.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                ParamSpec {
                    name: "bands",
                    ty: ParamType::Int { min: 1, max: 4 },
                    default: Some(Value::Int(2)),
                    doc: "Steps between the shadow colour and the lit colour: 1 is two-tone, 2 adds a mid tone, up to 4.",
                },
                number(
                    "feather",
                    Unit::None,
                    (0.0, 1.0),
                    0.08,
                    "Softness of each band edge: 0 is a razor edge, 0.1 a crisp anime edge, 1 a soft blend.",
                ),
                number(
                    "shift",
                    Unit::None,
                    (-1.0, 1.0),
                    0.0,
                    "Moves every band edge toward the light (positive: less shadow) or away from it (negative: more shadow).",
                ),
                number(
                    "spec",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "Size of a small hard highlight from the sun; 0 for none, 0.3 for hair or plastic shine.",
                ),
                ParamSpec {
                    name: "shade_color",
                    ty: ParamType::Vec3 {
                        unit: Unit::None,
                        min: 0.0,
                        max: 1.0,
                    },
                    default: Some(triple(0.62, 0.58, 0.82)),
                    doc: "What the surface colour is multiplied by in shadow, [r, g, b] (sRGB). A cool purple-blue gives the classic anime shadow; white gives none.",
                },
                number(
                    "rim",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "Strength of a rim light on the sunlit edge facing away from the viewer; 0.3 is gentle.",
                ),
                number(
                    "outline",
                    Unit::None,
                    (0.0, 16.0),
                    0.0,
                    "Outline thickness in screen pixels, constant at any distance; 0 for none, 1.5 to 3 for a character, 0.8 for fine detail. Works best on smooth, rounded shapes (a character, a sphere, a rock); a hard-edged box shows gaps at its corners.",
                ),
                ParamSpec {
                    name: "outline_color",
                    ty: ParamType::Vec3 {
                        unit: Unit::None,
                        min: 0.0,
                        max: 1.0,
                    },
                    default: Some(triple(0.1, 0.06, 0.14)),
                    doc: "Outline colour [r, g, b] (sRGB). A dark purple-brown reads softer than black; use a darker version of the surface colour for a painted look.",
                },
            ],
        ),
        spec(
            "solid.style@1",
            "Sets the whole look of a surface with one word, keeping its colours, roughness, textures and shape. `anime`: two-band cel shading with a \
                     tinted shadow, rim light and an outline. `flat`: one flat colour with an outline, no shading. `painterly`: soft three-band shading and \
                     painted-over textures (a Kuwahara filter), no outline. `retro`: hard three-band shading, pixelated textures in a few colours, thin outline. \
                     `ink`: hard two-tone shading, textures in three greys, a thick black outline. `realistic`: physically based shading again, and takes \
                     an earlier style away. Put it after solid.material and solid.texture. Use solid.toon instead for exact control of the bands.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                ParamSpec {
                    name: "style",
                    ty: ParamType::Choice {
                        options: &["anime", "realistic", "painterly", "flat", "retro", "ink"],
                    },
                    default: Some(Value::Str("anime".to_owned())),
                    doc: "The look: anime, realistic, painterly, flat, retro or ink.",
                },
                number(
                    "outline_scale",
                    Unit::None,
                    (0.0, 4.0),
                    1.0,
                    "Multiplies the style's outline width: 0 removes the outline, 2 doubles it.",
                ),
            ],
        ),
        spec(
            "solid.foliage@1",
            "Makes a surface behave like leaves, grass, a fence or any thin cut-out card. It is cut away wherever its albedo map's alpha is under alpha_cutoff, so a leaf shape drawn into a texture (a texture program or an expr node \
                     that ends in a vec4 whose last number is the alpha) becomes a leaf-shaped silhouette in the picture, in its shadow and in the exported glTF (alphaMode MASK). It also moves in the wind: sway bends the whole shape \
                     (more the higher a point is above the shape's base) and flutter makes the surface itself tremble. The wind's direction and strength come from scene.environment. Colours, roughness and textures are kept: put it \
                     after solid.material and solid.texture. All three at 0 makes it an ordinary surface again. Without a texture map there is no alpha to cut, so only the wind has an effect.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                number(
                    "alpha_cutoff",
                    Unit::None,
                    (0.0, 1.0),
                    0.5,
                    "Pixels whose albedo alpha is below this are cut away: 0 keeps everything, 0.5 is the usual leaf edge, near 1 keeps only fully opaque pixels.",
                ),
                number(
                    "sway",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "How far the whole shape bends with the wind, 0 still to 1 strongly; a tree swings from its base, so tall parts move more.",
                ),
                number(
                    "flutter",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "How much the surface trembles at a small scale (leaves in a breeze), 0 to 1.",
                ),
            ],
        ),
    ]
}
