//! The human body (ADR 0055): a person from named phenotype parameters, built from the CC0 human pack.
use super::spec;
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec, PortType};
use crate::value::{ParamType, Unit, Value};

fn fraction(name: &'static str, default: f64, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit: Unit::Ratio,
            min: 0.0,
            max: 1.0,
        },
        default: Some(Value::Num(default)),
        doc,
    }
}

fn measure(name: &'static str, min: f64, max: f64, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num {
            unit: Unit::Length,
            min,
            max,
        },
        default: Some(Value::Num(0.0)),
        doc,
    }
}

/// What `prefix` means on every node that skins a body.
const PREFIX_DOC: &str = "Put in front of every bone name, so a world can hold several people: with prefix=\"ken_\" the skin follows the parts named `ken_root`, `ken_spine01` and so on (give each body.bone that output name). Empty: the plain bone names. Use the same prefix on every node that skins this person.";

/// How the bones' motions combine on a skinned vertex (`blend` of every node that skins a body).
fn blend_param() -> ParamSpec {
    ParamSpec {
        name: "blend",
        ty: ParamType::Choice {
            options: &["dual", "linear"],
        },
        default: Some(Value::Str("dual".to_owned())),
        doc: "dual: dual quaternions, a shoulder or an elbow keeps its volume when it turns (drawn by the CPU, the default). linear: the renderer's own GPU skinning, cheaper, with some pinching at a raised arm. Use the same blend on every node that skins this person.",
    }
}

fn text(name: &'static str, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Str { max_len: 2048 },
        default: Some(Value::Str(String::new())),
        doc,
    }
}

fn triple(x: f64, y: f64, z: f64) -> Value {
    Value::List(vec![Value::Num(x), Value::Num(y), Value::Num(z)])
}

fn colour(name: &'static str, default: [f64; 3], doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Vec3 {
            unit: Unit::None,
            min: 0.0,
            max: 1.0,
        },
        default: Some(triple(default[0], default[1], default[2])),
        doc,
    }
}

fn eye() -> Option<OperatorSpec> {
    spec(
        "body.eye@1",
        "The two eyes of a body.human, layered like a real eye: a white sclera ball, a coloured iris, a black pupil and a clear glossy cornea over them, sized to the body's eye sockets (a child's eyes are smaller) and looking in the gaze direction. Connect the body.human node itself (before solid.material), and give the result its own output. Looks alive at any distance; eyes that look somewhere else are one gaze parameter away.",
        vec![super::single("in", &[PortType::Poly])],
        OutputSpec::Fixed(PortType::Solid),
        vec![
            colour(
                "iris_color",
                [0.25, 0.42, 0.55],
                "Colour of the iris [r, g, b] (sRGB): blue [0.25, 0.42, 0.55], brown [0.32, 0.18, 0.08], green [0.3, 0.45, 0.25].",
            ),
            colour(
                "sclera_color",
                [0.93, 0.9, 0.86],
                "Colour of the white of the eye [r, g, b]; slightly warm white is natural.",
            ),
            fraction(
                "iris_size",
                0.44,
                "Iris radius as a share of the eye ball's radius (0.4 to 0.5 is a human).",
            ),
            fraction(
                "pupil_size",
                0.4,
                "Pupil radius as a share of the iris radius: 0.25 in bright light, 0.7 in the dark.",
            ),
            ParamSpec {
                name: "gaze",
                ty: ParamType::Vec3 {
                    unit: Unit::None,
                    min: -1.0,
                    max: 1.0,
                },
                default: Some(triple(0.0, 0.0, 1.0)),
                doc: "Direction the eyes look, [x, y, z] in the body's space (+z is forward, +y up, +x the body's left). Need not be normalised.",
            },
        ],
    )
}

fn number(
    name: &'static str,
    unit: Unit,
    (min, max): (f64, f64),
    default: f64,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num { unit, min, max },
        default: Some(Value::Num(default)),
        doc,
    }
}

fn hair() -> Option<OperatorSpec> {
    spec(
        "body.hair@1",
        "Hair grown on the head of a body.human: thousands of tapered ribbon wisps that start on that head's own scalp inside a natural hairline (high forehead, temples, above the ears, down to the nape), leave it outward, fall under gravity, slide over the skull and wave a little. Connect the body.human node itself (before solid.material) and give the result its own output, then colour it with solid.material and add solid.finish(anisotropy=0.6, clearcoat=0.2) for the shine along the strands. Length 0.02 is a buzz cut, 0.15 a bob, 0.3 past the shoulders (very long hair passes through the shoulders: keep it under 0.35). It is cards, drawn from both sides; a few thousand wisps, about 40,000 triangles at density 0.6.",
        vec![super::single("in", &[PortType::Poly])],
        OutputSpec::Fixed(PortType::Solid),
        vec![
            number(
                "length",
                Unit::Length,
                (0.01, 1.0),
                0.22,
                "Length of the hair in metres, along the strand.",
            ),
            number(
                "density",
                Unit::Ratio,
                (0.0, 1.0),
                0.6,
                "How many wisps: 0.2 is thin, 0.6 normal, 1 thick (more triangles).",
            ),
            number(
                "volume",
                Unit::Ratio,
                (0.0, 1.0),
                0.3,
                "How far the hair stands off the head: 0 lies flat, 1 is big and full.",
            ),
            number(
                "wave",
                Unit::Ratio,
                (0.0, 1.0),
                0.25,
                "Waviness: 0 straight, 1 loose curls.",
            ),
            number(
                "fringe",
                Unit::Ratio,
                (0.0, 1.0),
                0.3,
                "How far the hairline comes down the forehead: 0 a high hairline, 1 bangs to the brows.",
            ),
            number(
                "width",
                Unit::Length,
                (0.004, 0.05),
                0.016,
                "Width of one wisp in metres at its root; wisps taper to a point.",
            ),
            number(
                "seed",
                Unit::None,
                (0.0, 100_000.0),
                1.0,
                "Picks a different arrangement of the same hair.",
            ),
            number(
                "fine",
                Unit::Ratio,
                (0.0, 1.0),
                0.0,
                "Splits each wisp into fine individual strands, up to 2.5 mm wide.",
            ),
            ParamSpec {
                name: "cover",
                ty: ParamType::Bool,
                default: Some(Value::Bool(false)),
                doc: "Adds a smooth hair shell and short root strands to cover the scalp.",
            },
        ],
    )
}

fn bone() -> Option<OperatorSpec> {
    spec(
        "body.bone@1",
        "A bone of a body.human skeleton, as a part you can animate: tiny and hidden inside the body at the named joint, with its rest segment from that joint (head, where it turns) to the next joint (tail), in the body's world position. Connect the body.human node itself. Join bones with anim.parent (a forearm to its upper arm), turn them with anim.track (pivot at the bone's head), and give each its OWN OUTPUT named exactly like the bone (`output upperarm01_l = ...`), because body.skin finds the bones by those names. Bone names are the pack's in lowercase with `.` and `-` as `_`: root, spine01 to spine05, neck01 to neck03, head, clavicle_l, shoulder01_l, upperarm01_l, upperarm02_l (the twist), lowerarm01_l (the elbow), lowerarm02_l, wrist_l, finger1_1_l (thumb) to finger5_3_l (little finger), metacarpal1_l to metacarpal4_l, upperleg01_l, lowerleg01_l, foot_l, toe1_1_l and the same ending `_r` for the right side. A wrong name lists them all.",
        vec![super::single("in", &[PortType::Poly])],
        OutputSpec::Fixed(PortType::Solid),
        vec![ParamSpec {
            name: "bone",
            ty: ParamType::Str { max_len: 64 },
            default: None,
            doc: "The bone, in the pack's name written lowercase with `_` for `.` and `-`: `upperarm01_l`, `finger2_1_r`.",
        }],
    )
}

fn skin() -> Option<OperatorSpec> {
    spec(
        "body.skin@1",
        "Skins a body.human to the bones you list, with the pack's own skin weights (the real ones, so knuckles, elbows and shoulders bend properly, unlike rig.skin's distance weights): each vertex follows the strongest four bones that move it. A bone you do not list hands its share to its nearest listed ancestor, so list only the joints you animate (an upper body with fingers is about 55 bones, the legs can stay with `root`). `bones` names the bone parts by their OUTPUT names, which must equal the bone names (see body.bone), and the body must be the input. A skinned body cannot also carry its own tracks or parent: move the bones. At most 64 bones. Give the result its own output, and put solid.material after body.skin, not before.",
        vec![super::single("in", &[PortType::Poly])],
        OutputSpec::SameAsInput("in"),
        vec![
            ParamSpec {
                name: "bones",
                ty: ParamType::Str { max_len: 2048 },
                default: None,
                doc: "Bone names separated by semicolons, root first, for example \"root; spine01; head; upperarm01_l; lowerarm01_l; wrist_l\".",
            },
            text("prefix", PREFIX_DOC),
            blend_param(),
        ],
    )
}

fn garment() -> Option<OperatorSpec> {
    spec(
        "body.garment@1",
        "Clothes for a body.human: the part of its surface a garment covers (a shirt, trousers, shoes), lifted off the skin by a few millimetres, as its own surface to give a fabric with solid.material and solid.texture. Connect the body.human node itself. Give it the same `bones` as body.skin and it is skinned with the very same weights, so it bends with the bones; leave `bones` empty for clothes on a body that does not move. `fit=\"skin\"` preserves the original fitted surface; `fit=\"tailored\"` adds looseness, folds and a closed cloth rim, and ignores `hem`. Several garments on one body are fine.",
        vec![super::single("in", &[PortType::Poly])],
        OutputSpec::Fixed(PortType::Solid),
        vec![
            ParamSpec {
                name: "cover",
                ty: ParamType::Choice {
                    options: &[
                        "shirt",
                        "long_sleeves",
                        "pants",
                        "shorts",
                        "shoes",
                        "gloves",
                    ],
                },
                default: Some(Value::Str("shirt".to_owned())),
                doc: "shirt: torso and upper arms (short sleeves). long_sleeves: to the wrist. pants: hips to ankles. shorts: hips to knees. shoes: feet. gloves: hands.",
            },
            ParamSpec {
                name: "offset",
                ty: ParamType::Num {
                    unit: Unit::Length,
                    min: 0.0,
                    max: 0.05,
                },
                default: Some(Value::Num(0.006)),
                doc: "How far the cloth stands off the skin in metres: 0.004 tight, 0.01 loose, 0.02 padded.",
            },
            ParamSpec {
                name: "bones",
                ty: ParamType::Str { max_len: 2048 },
                default: Some(Value::Str(String::new())),
                doc: "The bones of body.skin (the same list), to skin the garment with the body's weights; empty leaves it unskinned.",
            },
            text("prefix", PREFIX_DOC),
            blend_param(),
            ParamSpec {
                name: "hem",
                ty: ParamType::Bool,
                default: Some(Value::Bool(false)),
                doc: "Cut every open garment edge cleanly in its fitted plane; false preserves the body's original ragged skin-region edge.",
            },
        ],
    )
}

fn outfit() -> Option<OperatorSpec> {
    spec(
        "body.outfit@1",
        "Real clothes or hair made by artists (the CC0 MakeHuman library), fitted to a body.human: it is the real garment mesh with its own UVs, bound to the body's surface, so it follows every body slider (height, weight, muscle, gender) and sits on the skin the way the artist made it. `asset` picks one: clothes `female_casualsuit01` (a T-shirt and jeans), `shoes02`, hair `long01`, `bob02`, `short02`. Connect the body.human node itself. Give it the same `bones` as body.skin to skin it with the body's weights; leave `bones` empty for a body that does not move. It is a plain surface you can colour with solid.material and solid.texture, but its UVs are laid out for the artist's textures, so use body.outfit's own texture maps (solid.texture program `image(...)`) for the real look.",
        vec![super::single("in", &[PortType::Poly])],
        OutputSpec::Fixed(PortType::Solid),
        vec![
            ParamSpec {
                name: "asset",
                ty: ParamType::Str { max_len: 64 },
                default: None,
                doc: "The clothes or hair: female_casualsuit01, shoes02, long01, bob02, short02.",
            },
            ParamSpec {
                name: "offset",
                ty: ParamType::Num {
                    unit: Unit::Length,
                    min: -0.02,
                    max: 0.05,
                },
                default: Some(Value::Num(0.0)),
                doc: "Extra standoff from the body in metres (positive loosens the garment, negative tightens it).",
            },
            ParamSpec {
                name: "standoff",
                ty: ParamType::Num {
                    unit: Unit::Length,
                    min: 0.0,
                    max: 0.03,
                },
                default: Some(Value::Num(0.004)),
                doc: "The least distance from the skin in metres: cloth made for a slimmer body would sink into a heavier one, so no part of it goes closer than this.",
            },
            ParamSpec {
                name: "bones",
                ty: ParamType::Str { max_len: 2048 },
                default: Some(Value::Str(String::new())),
                doc: "The bones of body.skin (the same list), to skin the asset with the body's weights; empty leaves it unskinned.",
            },
            text("prefix", PREFIX_DOC),
            blend_param(),
        ],
    )
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    vec![
        garment(),
        outfit(),
        bone(),
        skin(),
        spec(
            "body.skin_to@1",
            "Transfers body.skin weights to a cloth solid from its nearest body surface vertex.",
            vec![
                super::single("cloth", &[PortType::Solid]),
                super::single("body", &[PortType::Poly]),
            ],
            OutputSpec::Fixed(PortType::Solid),
            vec![
                text("bones", "The body.skin bone list."),
                text("prefix", PREFIX_DOC),
                blend_param(),
            ],
        ),
        eye(),
        hair(),
        spec(
            "body.human@1",
            "A real human body and head with true proportions, from named sliders: one skinned-ready surface about 27,000 triangles (13,700 vertices) with UVs, standing with its origin at the pelvis, facing +z, in metres. It is built from open CC0 anthropometric data (MakeHuman/Anny), not sculpted. Put solid.material after it for skin colour. Every slider runs 0 to 1 and 0.5 is the middle; age is the one to set first (0.667 is a young adult, 0.333 a child, 0.0 a baby, 1.0 old). Combine them: an old, tall, heavy woman is gender=1, age=1, height=1, weight=0.9. `local` and `face` take lists of name=value pairs (see the human guide topic for the names).",
            vec![],
            OutputSpec::Fixed(PortType::Poly),
            vec![
                fraction(
                    "gender",
                    0.5,
                    "0 is the male prototype (taller, broader), 1 the female one; between blends them.",
                ),
                fraction(
                    "age",
                    0.667,
                    "0 baby, 0.333 child, 0.667 young adult, 1 elderly; in between blends (0.5 is a teenager).",
                ),
                fraction("height", 0.5, "0 short, 1 tall for the age and gender."),
                fraction("weight", 0.5, "0 very thin, 0.5 average, 1 heavy."),
                fraction("muscle", 0.5, "0 soft and slight, 0.5 average, 1 muscular."),
                fraction(
                    "proportions",
                    0.5,
                    "0 idealised proportions (long legs, small head), 1 the less common ones.",
                ),
                fraction(
                    "cupsize",
                    0.5,
                    "Breast size: 0 small, 0.5 average, 1 large. Only shows on the female end of gender, fading to nothing at gender 0.",
                ),
                fraction(
                    "firmness",
                    0.5,
                    "Breast firmness: 0 soft and low, 1 high and firm.",
                ),
                fraction(
                    "african",
                    0.5,
                    "Share of the African body and face shape. The three shares are normalised, so only their ratio matters.",
                ),
                fraction("asian", 0.5, "Share of the Asian body and face shape."),
                fraction(
                    "caucasian",
                    0.5,
                    "Share of the Caucasian body and face shape.",
                ),
                measure(
                    "stature",
                    0.0,
                    3.0,
                    "Ask for this height in metres (0 leaves the height slider in charge). The body is fitted to it, changing the least it can: stature=1.82.",
                ),
                measure(
                    "waist",
                    0.0,
                    2.5,
                    "Ask for this waist circumference in metres (0 for none): 0.84.",
                ),
                measure(
                    "hip",
                    0.0,
                    2.5,
                    "Ask for this hip circumference in metres (0 for none): 1.02.",
                ),
                measure(
                    "chest",
                    0.0,
                    2.5,
                    "Ask for this chest circumference in metres (0 for none): 1.04.",
                ),
                ParamSpec {
                    name: "mass",
                    ty: ParamType::Num {
                        unit: Unit::None,
                        min: 0.0,
                        max: 400.0,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "Ask for this body mass in kilograms (0 for none): 78. Mass comes from the surface volume at 980 kg/m3, so it follows the weight and muscle sliders. When several measurements conflict, the report in the warnings says what was reached.",
                },
                ParamSpec {
                    name: "at",
                    ty: ParamType::Vec3 {
                        unit: Unit::Length,
                        min: -10000.0,
                        max: 10000.0,
                    },
                    default: Some(triple(0.0, 0.0, 0.0)),
                    doc: "Where the person stands, [x, y, z] in metres: the feet at y = 0 when y is 0. Place people side by side with this (the eyes follow the body).",
                },
                ParamSpec {
                    name: "turn",
                    ty: ParamType::Num {
                        unit: Unit::None,
                        min: -360.0,
                        max: 360.0,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "Degrees to turn the person about the vertical axis, around where they stand: 90 faces +x, -90 faces -x, 180 faces the camera in a studio.",
                },
                text(
                    "local",
                    "Local changes, as `name=value; name=value`, value -1 to 1: `l-upperarm-fat-incr=0.5; nose-width1-incr=-0.3; head-fat-incr=0.2` (left and right limbs are separate, `l-` and `r-`). The valid names are listed by the error message when one is wrong.",
                ),
                text(
                    "face",
                    "Face actions, as `name=value; name=value`, value 0 to 1, from the 52 ARKit-style shapes: `jawOpen=0.8; mouthSmileLeft=0.6; eyeBlinkLeft=1`.",
                ),
            ],
        ),
    ]
}
