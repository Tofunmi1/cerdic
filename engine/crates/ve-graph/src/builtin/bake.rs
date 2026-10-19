//! `bake.maps`: bake a high-poly part's normals and ambient occlusion onto a low-poly part's UVs.

use super::{EXTENT, single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

const MESHES: &[PortType] = &[PortType::Solid, PortType::Poly];

fn setting(name: &'static str, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit: Unit::Length,
            min: 0.0,
            max: EXTENT,
        },
        default: Some(Value::Num(0.0)),
        doc,
    }
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![spec(
        "bake.maps@1",
        "Bakes the detail of a high-poly part onto a low-poly part: a tangent-space normal map and an ambient-occlusion map in the low part's UV layout. The result is the low part itself with the maps attached, \
         so it keeps them through moves and materials, shows the normal map in the viewer and exports both in glTF. The low part must be unwrapped (poly.unwrap_uv, then poly.to_solid) and sit at the same place as the high part. \
         Settings left at 0 scale with the low part's size: cage 5%, max_distance 10%, bias 0.1% of its bounding-box diagonal.",
        vec![single("low", MESHES), single("high", MESHES)],
        OutputSpec::Fixed(PortType::Solid),
        vec![
            ParamSpec {
                name: "size",
                ty: ParamType::Int { min: 64, max: 2048 },
                default: Some(Value::Int(512)),
                doc: "Side of each baked image in pixels.",
            },
            ParamSpec {
                name: "maps",
                ty: ParamType::Choice {
                    options: &["both", "normal", "ao"],
                },
                default: Some(Value::Str("both".to_owned())),
                doc: "Which maps to bake: both, normal, or ao.",
            },
            setting(
                "cage",
                "How far above the low surface each ray starts, in metres (0 = automatic). Raise it when the high part rises above the low surface.",
            ),
            setting(
                "max_distance",
                "How far below the cage a ray may reach to find the high surface, in metres (0 = automatic).",
            ),
            ParamSpec {
                name: "rays",
                ty: ParamType::Int { min: 1, max: 256 },
                default: Some(Value::Int(64)),
                doc: "Rays per texel for the occlusion map. size x size x rays may not exceed 64 million.",
            },
            setting(
                "bias",
                "How far above the surface the occlusion rays start, in metres (0 = automatic).",
            ),
        ],
    )]
}
