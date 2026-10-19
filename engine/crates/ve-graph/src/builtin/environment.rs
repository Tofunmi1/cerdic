//! The authored scene environment: sky/ground ambient irradiance and an optional ground bounce.
use super::{spec, vec3};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

fn colour(name: &'static str, default: [f64; 3], doc: &'static str) -> ParamSpec {
    let mut param = vec3(name, Unit::None, 0.0, 1.0, doc);
    param.default = Some(Value::List(default.map(Value::Num).to_vec()));
    param
}

/// A 0-to-1 strength parameter with a default.
fn strength(name: &'static str, default: f64, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit: Unit::None,
            min: 0.0,
            max: 1.0,
        },
        default: Some(Value::Num(default)),
        doc,
    }
}

fn intensity() -> ParamSpec {
    ParamSpec {
        name: "intensity",
        ty: ParamType::Num {
            unit: Unit::None,
            min: 0.0,
            max: 8.0,
        },
        default: Some(Value::Num(0.15)),
        doc: "Ambient brightness multiplier.",
    }
}

fn environment() -> Option<OperatorSpec> {
    spec(
        "scene.environment@1",
        "Sky and ground ambient irradiance, approximated as a low-order hemisphere; set gi=true for traced light probes (global illumination) instead.",
        vec![],
        OutputSpec::Fixed(PortType::Environment),
        vec![
            colour(
                "sky_color",
                [0.68, 0.76, 0.9],
                "Diffuse light from above, in sRGB.",
            ),
            colour(
                "ground_color",
                [0.46, 0.39, 0.3],
                "Diffuse light from below, in sRGB.",
            ),
            intensity(),
            strength(
                "reflection",
                0.0,
                "Strength of the reflected studio sky for metals and gloss.",
            ),
            strength(
                "bounce",
                0.0,
                "Warm ground-bounce strength added to downward-facing surfaces.",
            ),
            colour(
                "bounce_color",
                [0.6, 0.44, 0.28],
                "Ground-bounce colour, in sRGB.",
            ),
        ],
    )
}

fn sky() -> Option<OperatorSpec> {
    spec(
        "scene.sky@1",
        "Procedural analytic sky with a sun, atmosphere gradient and optional deterministic clouds. Supplies an approximate ambient hemisphere and sun light; a separate scene.environment overrides its ambient lighting while keeping this background.",
        vec![],
        OutputSpec::Fixed(PortType::Environment),
        vec![
            ParamSpec {
                name: "sun_elevation",
                ty: ParamType::Num {
                    unit: Unit::None,
                    min: -5.0,
                    max: 90.0,
                },
                default: Some(Value::Num(35.0)),
                doc: "Sun angle above the horizon in degrees; negative values place it just below the horizon.",
            },
            ParamSpec {
                name: "sun_azimuth",
                ty: ParamType::Num {
                    unit: Unit::None,
                    min: -180.0,
                    max: 180.0,
                },
                default: Some(Value::Num(0.0)),
                doc: "Sun compass angle in degrees around the vertical axis.",
            },
            ParamSpec {
                name: "sun_intensity",
                ty: ParamType::Num {
                    unit: Unit::None,
                    min: 0.0,
                    max: 100.0,
                },
                default: Some(Value::Num(3.0)),
                doc: "Direct sunlight brightness; drives the scene's shadow-casting sun.",
            },
            ParamSpec {
                name: "turbidity",
                ty: ParamType::Num {
                    unit: Unit::None,
                    min: 1.0,
                    max: 10.0,
                },
                default: Some(Value::Num(3.0)),
                doc: "Atmospheric haze; high values warm and soften the horizon.",
            },
            colour(
                "ground_color",
                [0.37, 0.29, 0.20],
                "Procedural horizon ground colour in sRGB.",
            ),
            ParamSpec {
                name: "clouds",
                ty: ParamType::Bool,
                default: Some(Value::Bool(false)),
                doc: "Draw a few-octave procedural cloud layer.",
            },
            strength(
                "cloud_cover",
                0.35,
                "Coverage threshold for the seeded cloud layer.",
            ),
            ParamSpec {
                name: "seed",
                ty: ParamType::Int {
                    min: 0,
                    max: 1_000_000,
                },
                default: Some(Value::Int(0)),
                doc: "Picks a deterministic cloud pattern.",
            },
        ],
    )
}

/// The settings that turn on light probes (global illumination). Both nodes set the scene's lighting, so both take them.
fn probe_params() -> Vec<ParamSpec> {
    let whole = |name, min, max, default, doc| ParamSpec {
        name,
        ty: ParamType::Int { min, max },
        default: Some(Value::Int(default)),
        doc,
    };
    vec![
        ParamSpec {
            name: "gi",
            ty: ParamType::Bool,
            default: Some(Value::Bool(false)),
            doc: "Light the scene with traced light probes (global illumination) instead of the constant sky ambient: the sky as each place sees it (sheltered corners are darker), and sun and glowing surfaces bounced off other surfaces, so walls pick up the colour of what is next to them. It is calculated a little at a time after each edit, and the scene looks as before until it is done. Not seen by anything that is not drawn in the viewer.",
        },
        ParamSpec {
            name: "gi_spacing",
            ty: ParamType::Num {
                unit: Unit::Length,
                min: 0.0,
                max: 64.0,
            },
            default: Some(Value::Num(0.0)),
            doc: "Distance between light probes in metres: 0 sizes it from the scene (about 24 probes across its longest side). Smaller captures finer light and takes longer; at most 64 probes along an axis.",
        },
        whole(
            "gi_bounces",
            1,
            4,
            2,
            "How many times light bounces between surfaces: 1 is sky, sun and glow bounced once, 2 or more also light bounced off bounced light.",
        ),
        whole(
            "gi_rays",
            32,
            512,
            128,
            "Rays traced per probe for each bounce: more is smoother and slower.",
        ),
    ]
}

/// The wind that bends foliage (`solid.foliage` with sway or flutter).
fn wind_params() -> Vec<ParamSpec> {
    vec![
        ParamSpec {
            name: "wind_direction",
            ty: ParamType::Num {
                unit: Unit::None,
                min: -360.0,
                max: 360.0,
            },
            default: Some(Value::Num(0.0)),
            doc: "Direction the wind blows toward, in degrees around the vertical axis: 0 is +x, 90 is +z.",
        },
        ParamSpec {
            name: "wind_strength",
            ty: ParamType::Num {
                unit: Unit::None,
                min: 0.0,
                max: 2.0,
            },
            default: Some(Value::Num(0.5)),
            doc: "How hard the wind blows: 0 is still air, 0.5 a gentle breeze, 1 a strong breeze, 2 a gale. It moves only foliage with sway or flutter (solid.foliage); the viewer shows it, an exported glTF does not.",
        },
    ]
}

fn with_probe_params(mut spec: OperatorSpec) -> OperatorSpec {
    spec.params.extend(probe_params());
    spec.params.extend(wind_params());
    spec
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    [environment(), sky()]
        .into_iter()
        .map(|spec| spec.map(with_probe_params))
        .collect()
}
