#[path = "appearance/foliage.rs"]
mod foliage;
#[path = "appearance/texture.rs"]
mod texture;
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
        texture::specification(),
        spec(
            "material.preset@1",
            "Applies a named tiled procedural surface with optional edge wear, crease dirt, top-facing moss and wetness. oak: warm wood; planks: boards; red_brick: fired brick; cobblestone: rounded stone; concrete: cast grey stone; plaster: pale wall; rusted_iron: oxidized metal; brushed_steel: satin steel; leather: pebbled hide; linen: woven cloth; sand: fine grains; mud: damp earth; snow: soft snow; ice: blue ice; asphalt: dark road surface (give it wet for a night street); corrugated_metal: ribbed steel sheet; painted_metal: dark painted panel that shows bare metal on its edges; wall_tile: small glazed tiles.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                ParamSpec {
                    name: "name",
                    ty: ParamType::Choice {
                        options: &[
                            "oak",
                            "planks",
                            "red_brick",
                            "cobblestone",
                            "concrete",
                            "plaster",
                            "rusted_iron",
                            "brushed_steel",
                            "leather",
                            "linen",
                            "sand",
                            "mud",
                            "snow",
                            "ice",
                            "asphalt",
                            "corrugated_metal",
                            "painted_metal",
                            "wall_tile",
                        ],
                    },
                    default: Some(Value::Str("oak".to_owned())),
                    doc: "Named surface look.",
                },
                ParamSpec {
                    name: "tile",
                    ty: ParamType::Num {
                        unit: Unit::Length,
                        min: 0.1,
                        max: 10.0,
                    },
                    default: Some(Value::Num(1.0)),
                    doc: "Length in metres of one repeat.",
                },
                number("wear", Unit::None, (0.0, 1.0), 0.3, "Edge wear amount."),
                number(
                    "dirt",
                    Unit::None,
                    (0.0, 1.0),
                    0.2,
                    "Dirt gathered in creases.",
                ),
                number(
                    "moss",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "Moss on upward-facing surfaces.",
                ),
                number(
                    "wet",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "Wetness darkening and gloss.",
                ),
                ParamSpec {
                    name: "size",
                    ty: ParamType::Choice {
                        options: &["256", "512", "1024"],
                    },
                    default: Some(Value::Str("512".to_owned())),
                    doc: "Side in pixels of the baked maps.",
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
        foliage::spec(),
        spec(
            "solid.finish@1",
            "Adds layers on top of a surface that the physically based renderer draws: a glossy clear coat (car paint, wet skin, glazed ceramic, scales, eyes), highlights stretched along the surface \
                     (brushed metal, silk, hair) and translucency, light passing through a thin body (leaves, wax, ears, wings). Colours, roughness and textures are kept: put it after solid.material and solid.texture. \
                     All zero makes it the plain surface again. It shows in the Bevy renderer (the WebGPU and WebGL2 bundles); the exported glTF does not carry it.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                number(
                    "clearcoat",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "Strength of a glossy clear layer over the surface: 0 none, 1 a full lacquer. A wet look is about 0.6 with clearcoat_roughness 0.05.",
                ),
                number(
                    "clearcoat_roughness",
                    Unit::None,
                    (0.0, 1.0),
                    0.1,
                    "Roughness of the clear layer: 0 a mirror, 1 a satin finish.",
                ),
                number(
                    "anisotropy",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "How much highlights stretch along the surface (brushed metal, silk), 0 to 1.",
                ),
                number(
                    "translucency",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "Share of diffuse light that passes through a thin body and lights the far side: 0 opaque, 0.4 leaves, wax and ears, 1 a lampshade.",
                ),
                number(
                    "relief",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "Procedural surface relief drawn in the pixel shader, finer than the mesh and free of texture memory: 0 none, 0.3 skin pores or leather, 1 deep scales or bark. It follows a part that moves as a whole; on a skinned part it slides, so use a baked normal map there.",
                ),
                number(
                    "relief_scale",
                    Unit::None,
                    (0.1, 500.0),
                    30.0,
                    "How many relief features fit in a metre: 8 coarse bark, 30 scales on a lizard, 120 fine pores.",
                ),
                number(
                    "wetness",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "A wet look: the surface darkens and turns glossy, most of all in the low parts of its relief (crevices stay wet). 0.5 damp skin, 1 just out of the water.",
                ),
                number(
                    "iridescence",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "Colour that shifts with the viewing angle, like a beetle shell, fish scales or a soap film: 0.3 a hint, 1 strong rainbow.",
                ),
                number(
                    "sheen",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "A soft brightening at grazing angles (velvet, peach fuzz, fine hair, moss): 0.3 a soft edge glow, 1 strong.",
                ),
                number(
                    "subsurface",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "Light scattering under the surface, which softens the shadow edge with a warm tint (skin, wax, ears, jelly): 0.5 skin, 1 wax.",
                ),
                number(
                    "thickness",
                    Unit::None,
                    (0.0, 1.0),
                    0.0,
                    "Sunlight passing through a thin body: with the sun behind it the surface glows in its own colour on the side facing you (wing membranes, leaves, ears, candle wax, paper, thin cloth): 0.4 a faint glow, 1 a lantern. Use it on thin parts only: it does not check whether something else shades the sun, so a thick body would glow too.",
                ),
                ParamSpec {
                    name: "relief_style",
                    ty: ParamType::Choice {
                        options: &["smooth", "ridged", "scales"],
                    },
                    default: Some(Value::Str("smooth".to_owned())),
                    doc: "smooth is soft lumps, ridged is creases and wrinkles, scales is round overlapping cells.",
                },
            ],
        ),
    ]
}
