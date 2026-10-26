//! Cutout, wind and the grass look parameters.
use super::super::{COLOURABLE, number, single, triple};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec};
use crate::value::{ParamType, Unit, Value};
pub(super) fn spec() -> Option<OperatorSpec> {
    super::super::spec(
        "solid.foliage@1",
        "Makes a surface behave like leaves, grass, a fence or any thin cut-out card. It is cut away wherever its albedo map's alpha is under alpha_cutoff, so a leaf shape drawn into a texture (a texture program or an expr node \
                     that ends in a vec4 whose last number is the alpha) becomes a leaf-shaped silhouette in the picture, in its shadow and in the exported glTF (alphaMode MASK). It also moves in the wind: sway bends the whole shape \
                     (more the higher a point is above the shape's base) and flutter makes the surface itself tremble. The wind's direction and strength come from scene.environment. Colours, roughness and textures are kept: put it \
                     after solid.material and solid.texture. Set grass=true for root shading, upward normals and distance shrink. With grass=false and alpha_cutoff, sway and flutter at 0 it is an ordinary surface again.",
        vec![single("in", COLOURABLE)],
        OutputSpec::SameAsInput("in"),
        vec![
            ParamSpec {
                name: "grass",
                ty: ParamType::Bool,
                default: Some(Value::Bool(false)),
                doc: "Enable the grass look: soil-coloured roots, upward normals and blades shrinking with camera distance.",
            },
            ParamSpec {
                name: "ground_color",
                ty: ParamType::Vec3 {
                    unit: Unit::None,
                    min: 0.0,
                    max: 1.0,
                },
                default: Some(triple(0.25, 0.18, 0.10)),
                doc: "Host soil colour [r,g,b] in sRGB, used at the blade roots.",
            },
            number(
                "ground_blend",
                Unit::None,
                (0.0, 1.0),
                0.0,
                "Strength of the root colour blend; 0 disables it, 1 fully matches the soil at the root.",
            ),
            number(
                "fade_near",
                Unit::Length,
                (0.0, 100000.0),
                25.0,
                "Blades remain full size through this camera distance in metres.",
            ),
            number(
                "fade_far",
                Unit::Length,
                (0.0, 100000.0),
                70.0,
                "Blades shrink linearly to zero here; at or below fade_near disables fading.",
            ),
            number(
                "translucency",
                Unit::None,
                (0.0, 1.0),
                0.0,
                "Sunlight through the blade, strongest at the tip; ignored at Low quality.",
            ),
            number(
                "exposure_boost",
                Unit::None,
                (0.0, 10.0),
                1.0,
                "Blade colour multiplier, floored at 0.1 by the renderer.",
            ),
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
    )
}
