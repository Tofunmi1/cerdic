use super::{COLOURABLE, single, spec, triple};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec};
use crate::value::{ParamType, Unit, Value};

pub(super) fn specs() -> Vec<Option<OperatorSpec>> {
    vec![
        spec(
            "anim.track@1",
            "Makes a shape move: attaches one keyframed track (position, rotation or scale over time) and passes the shape through unchanged. \
                     Chain several anim.track nodes to move and turn together. Rotation and scale act about `pivot` (a door's hinge, a fan's axle), \
                     given in the shape's own coordinates. Plays in the viewport and exports as a glTF animation.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                ParamSpec {
                    name: "property",
                    ty: ParamType::Choice {
                        options: &["position", "rotation", "scale", "glow", "visible", "color"],
                    },
                    default: Some(Value::Str("position".to_owned())),
                    doc: "What changes: position (metres, added to where the shape is), rotation (degrees about X, Y, Z), scale (factors), glow (a brightness factor on the shape's emissive colour and the light it emits; keys are `seconds:factor`, 1 is as authored, 0 is dark), visible (keys `seconds:1` shown or `seconds:0` hidden; use interpolation step) or color (keys `seconds:r,g,b` with each 0 to 1; the shape's surface colour fades between them, replacing the authored colour).",
                },
                ParamSpec {
                    name: "keys",
                    ty: ParamType::Str { max_len: 2048 },
                    default: None,
                    doc: "Keyframes as `seconds:x,y,z` separated by semicolons, at least two with increasing times, for example \"0:0,0,0; 2:0,90,0\" swings 90 degrees about Y over two seconds. Every key names all three values.",
                },
                ParamSpec {
                    name: "interpolation",
                    ty: ParamType::Choice {
                        options: &[
                            "linear", "smooth", "ease_in", "ease_out", "bezier", "back", "bounce",
                            "elastic", "step",
                        ],
                    },
                    default: Some(Value::Str("smooth".to_owned())),
                    doc: "How values move between keys: linear, smooth (ease in and out of every key), ease_in (start slowly), ease_out (end slowly), bezier (one smooth curve through ALL the keys without stopping at each, no overshoot; the best choice for motion with three or more keys), back (overshoots the next key a little and returns), bounce (bounces against the next key), elastic (springs past the next key and settles) or step (hold until the next key).",
                },
                ParamSpec {
                    name: "loop",
                    ty: ParamType::Bool,
                    default: Some(Value::Bool(true)),
                    doc: "Start over after the last key. Off holds the last key's value.",
                },
                ParamSpec {
                    name: "clip",
                    ty: ParamType::Str { max_len: 24 },
                    default: Some(Value::Str(String::new())),
                    doc: "The named clip this track belongs to (lowercase, for example \"wave\" or \"reach\"); empty is the part's main clip. A behaviour runs one clip at a time, so a part can have several separate motions. The poses of all its clips combine.",
                },
                ParamSpec {
                    name: "phase_step",
                    ty: ParamType::Num {
                        unit: Unit::None,
                        min: -60.0,
                        max: 60.0,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "For a set of placed copies (inst.array, inst.radial, inst.repeat): seconds each copy plays ahead of the one before, so a motion runs along a row like a wave. Only for placed sets.",
                },
                ParamSpec {
                    name: "phase_jitter",
                    ty: ParamType::Num {
                        unit: Unit::None,
                        min: 0.0,
                        max: 60.0,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "For a set of placed copies: up to this many extra seconds of a fixed pseudo-random offset per copy, so a field of reeds or flags does not move in step. Only for placed sets.",
                },
                ParamSpec {
                    name: "pivot",
                    ty: ParamType::Vec3 {
                        unit: Unit::Length,
                        min: -1000.0,
                        max: 1000.0,
                    },
                    default: Some(triple(0.0, 0.0, 0.0)),
                    doc: "The point, in the shape's own coordinates, that rotation and scale act about.",
                },
            ],
        ),
        spec(
            "anim.wave@1",
            "Makes a shape oscillate forever without writing keyframes: a bob, a sway, a pulse, a flicker. It becomes one looping anim.track \
                     (so it plays, chains and exports like any track). The value is `base + amplitude * wave`, where the wave swings between -1 and 1 \
                     once per `period`. Rotation acts about `pivot`.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                ParamSpec {
                    name: "property",
                    ty: ParamType::Choice {
                        options: &["position", "rotation", "scale", "glow"],
                    },
                    default: Some(Value::Str("position".to_owned())),
                    doc: "What oscillates: position (metres), rotation (degrees), scale (factors around base 1) or glow (brightness factor around base 1; only the first value is used).",
                },
                ParamSpec {
                    name: "shape",
                    ty: ParamType::Choice {
                        options: &["sine", "triangle"],
                    },
                    default: Some(Value::Str("sine".to_owned())),
                    doc: "sine is smooth; triangle moves at constant speed and turns sharply at the ends.",
                },
                ParamSpec {
                    name: "amplitude",
                    ty: ParamType::Vec3 {
                        unit: Unit::None,
                        min: -1000.0,
                        max: 1000.0,
                    },
                    default: Some(triple(0.0, 0.0, 0.0)),
                    doc: "How far each axis swings from base (metres, degrees or a factor, depending on property). [0, 0.15, 0] bobs 15 cm up and down.",
                },
                ParamSpec {
                    name: "base",
                    ty: ParamType::Vec3 {
                        unit: Unit::None,
                        min: -1000.0,
                        max: 1000.0,
                    },
                    default: Some(triple(0.0, 0.0, 0.0)),
                    doc: "The middle value. Leave it at 0 for position and rotation; for scale and glow an untouched base of [0, 0, 0] means [1, 1, 1].",
                },
                ParamSpec {
                    name: "period",
                    ty: ParamType::Num {
                        unit: Unit::None,
                        min: 0.05,
                        max: 600.0,
                    },
                    default: Some(Value::Num(2.0)),
                    doc: "Seconds for one full swing.",
                },
                ParamSpec {
                    name: "phase",
                    ty: ParamType::Num {
                        unit: Unit::None,
                        min: 0.0,
                        max: 1.0,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "Where in the swing it starts, as a fraction of the period (0.5 starts half a swing in, so two shapes can move in opposite phase).",
                },
                ParamSpec {
                    name: "clip",
                    ty: ParamType::Str { max_len: 24 },
                    default: Some(Value::Str(String::new())),
                    doc: "The named clip this wave belongs to; empty is the part's main clip (see anim.track).",
                },
                ParamSpec {
                    name: "phase_step",
                    ty: ParamType::Num {
                        unit: Unit::None,
                        min: -60.0,
                        max: 60.0,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "For a set of placed copies: seconds each copy plays ahead of the one before (a wave along a row). Only for placed sets.",
                },
                ParamSpec {
                    name: "phase_jitter",
                    ty: ParamType::Num {
                        unit: Unit::None,
                        min: 0.0,
                        max: 60.0,
                    },
                    default: Some(Value::Num(0.0)),
                    doc: "For a set of placed copies: up to this many extra seconds of a fixed pseudo-random offset per copy, so a field does not sway in step. Only for placed sets.",
                },
                ParamSpec {
                    name: "pivot",
                    ty: ParamType::Vec3 {
                        unit: Unit::Length,
                        min: -1000.0,
                        max: 1000.0,
                    },
                    default: Some(triple(0.0, 0.0, 0.0)),
                    doc: "The point, in the shape's own coordinates, that rotation and scale act about.",
                },
            ],
        ),
        spec(
            "anim.parent@1",
            "Attaches a shape to another part so it moves with it, like a forearm on an upper arm or a wheel on a car. \
                     `parent` is the OUTPUT name of the part to follow. The shape keeps its own anim.track motion (an elbow bending) \
                     on top of the parent's motion. A name that matches no part, a loop, or a chain longer than 8 is dropped with a warning.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![ParamSpec {
                name: "parent",
                ty: ParamType::Str { max_len: 64 },
                default: None,
                doc: "The output name of the part to follow. Author this shape at its rest position in the same world coordinates; its pivots stay in those coordinates.",
            }],
        ),
    ]
}
