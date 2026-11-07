//! Physics body marker (ADR 0035).
use super::{COLOURABLE, single, spec};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec};
use crate::value::{ParamType, Unit, Value};

fn number(
    name: &'static str,
    default: f64,
    min: f64,
    max: f64,
    unit: Unit,
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Num { unit, min, max },
        default: Some(Value::Num(default)),
        doc,
    }
}

fn vector(name: &'static str, default: [f64; 3], unit: Unit, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Vec3 {
            unit,
            min: -1000.0,
            max: 1000.0,
        },
        default: Some(Value::List(
            default.iter().map(|v| Value::Num(*v)).collect(),
        )),
        doc,
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "Keep the joint's parameter list together."
)]
fn joint() -> Option<OperatorSpec> {
    spec(
        "phys.joint@1",
        "Connects a physics body to another part or to the world with a joint, for mechanisms: a door on hinges, a pendulum, a drawer, a rope link, a wheel on an axle, a spring-hung weight. \
         Put it AFTER phys.body on the part that moves; `to` is the OUTPUT name of the other part (which must also have phys.body) or empty to join this part to the world. \
         Give the joint's `anchor` in world coordinates at the starting pose (the hinge line or the pivot point), and `axis` for hinges and sliders. \
         kind=hinge turns about the axis (limit it with `limited`, `low`, `high` in degrees); slider moves along the axis (limits in metres); ball turns freely about the anchor; \
         fixed welds the two together; spring pulls anchor and anchor_b toward `rest_length`. A hinge or slider with motor_force above 0 drives itself at motor_speed (a fan, a wheel, an elevator). \
         A part can carry up to 4 joints (a chain link has two). Whatever a joint hangs from and should not move (a post, a frame, a ceiling hook) needs phys.body kind=\"static\", or it swings too.",
        vec![single("in", COLOURABLE)],
        OutputSpec::SameAsInput("in"),
        vec![
            ParamSpec {
                name: "kind",
                ty: ParamType::Choice {
                    options: &["hinge", "slider", "ball", "fixed", "spring"],
                },
                default: Some(Value::Str("hinge".to_owned())),
                doc: "hinge: turns about an axis. slider: slides along an axis. ball: turns freely about the anchor. fixed: welded. spring: elastic pull between two points.",
            },
            ParamSpec {
                name: "to",
                ty: ParamType::Str { max_len: 64 },
                default: Some(Value::Str(String::new())),
                doc: "Output name of the other part (it needs phys.body too), or empty to attach to the world.",
            },
            vector(
                "anchor",
                [0.0; 3],
                Unit::Length,
                "The joint point in world coordinates at the start (the hinge line, the pivot, where the spring hooks on this part).",
            ),
            vector(
                "axis",
                [0.0, 1.0, 0.0],
                Unit::None,
                "Hinge axis or slider direction in world coordinates: [0,1,0] is a vertical hinge like a door, [0,0,1] a horizontal one like a swing or pendulum.",
            ),
            vector(
                "anchor_b",
                [0.0; 3],
                Unit::Length,
                "Springs only: where the spring hooks onto the other part (or the world). Leave at the anchor for other kinds.",
            ),
            ParamSpec {
                name: "limited",
                ty: ParamType::Bool,
                default: Some(Value::Bool(false)),
                doc: "Whether low and high limit the movement (a door that opens 100 degrees, a drawer that slides 0.4 m).",
            },
            number(
                "low",
                0.0,
                -100_000.0,
                100_000.0,
                Unit::None,
                "Lowest allowed angle in degrees (hinge) or distance in metres (slider) from the starting pose.",
            ),
            number(
                "high",
                0.0,
                -100_000.0,
                100_000.0,
                Unit::None,
                "Highest allowed angle in degrees (hinge) or distance in metres (slider) from the starting pose.",
            ),
            number(
                "stiffness",
                20.0,
                0.0,
                10_000.0,
                Unit::None,
                "Spring pull as acceleration per metre of stretch: 5 is gentle, 50 stiff; heavy and light bodies swing alike.",
            ),
            number(
                "damping",
                1.0,
                0.0,
                1_000.0,
                Unit::None,
                "Spring damping, or the friction that slows a hinge or slider (0 swings forever).",
            ),
            number(
                "motor_speed",
                0.0,
                -10_000.0,
                10_000.0,
                Unit::None,
                "Motor aim in degrees per second (hinge) or metres per second (slider); needs motor_force above 0.",
            ),
            number(
                "motor_force",
                0.0,
                0.0,
                1_000_000.0,
                Unit::None,
                "Strongest push or torque the motor may use; 0 means no motor.",
            ),
            number(
                "rest_length",
                -1.0,
                -1.0,
                1000.0,
                Unit::Length,
                "Springs only: the distance the spring rests at; -1 means the distance between the anchors at the start.",
            ),
        ],
    )
}

fn world() -> Option<OperatorSpec> {
    spec(
        "phys.world@1",
        "Sets the physics of the whole scene: gravity, and whether you WALK through it in play mode. Put it after phys.body on any ONE body (the floor is natural); \
         a second one is ignored with a warning. gravity [0,-9.81,0] is Earth, [0,-1.62,0] the Moon, [0,0,0] weightless. With walker=true, play mode replaces the flying camera \
         with a walking person: WASD walks, Space jumps, you fall under gravity, stand on static bodies, are stopped by walls and step over kerbs up to 30 cm; \
         start from where the camera is. Without it, play mode keeps the flying camera. Walking is for the desktop viewer; a headset keeps its own movement.",
        vec![single("in", COLOURABLE)],
        OutputSpec::SameAsInput("in"),
        vec![
            vector(
                "gravity",
                [0.0, -9.81, 0.0],
                Unit::None,
                "Gravity in metres per second squared: [0,-9.81,0] Earth, [0,-1.62,0] Moon, [0,0,0] weightless.",
            ),
            ParamSpec {
                name: "walker",
                ty: ParamType::Bool,
                default: Some(Value::Bool(false)),
                doc: "Walk through the scene in play mode (WASD, Space to jump) instead of flying.",
            },
            number(
                "height",
                1.7,
                0.6,
                3.0,
                Unit::Length,
                "The walker's height in metres (eye level is just below the top).",
            ),
            number(
                "radius",
                0.3,
                0.1,
                1.0,
                Unit::Length,
                "The walker's radius in metres; doorways and gaps must be wider than twice this.",
            ),
            number(
                "speed",
                3.0,
                0.5,
                20.0,
                Unit::None,
                "Walking speed in metres per second (3 is a brisk walk, Shift runs at double).",
            ),
            number(
                "jump",
                4.5,
                0.0,
                15.0,
                Unit::None,
                "Upward speed of a jump in metres per second: 4.5 jumps about 1 m high, 0 disables jumping.",
            ),
        ],
    )
}

pub(super) fn all() -> Vec<Option<OperatorSpec>> {
    let mut ops = vec![spec(
        "phys.body@1",
        "Makes a shape a physical object in play mode: it falls under gravity, collides with other bodies, and can be picked up and thrown. \
         kind=static for floors, walls and terrain (never moves), dynamic for things that fall, roll and stack (balls, crates, props), \
         kinematic for things that move by hand or script and shove others. Parts without phys.body ignore physics (a floor you \
         forgot to mark static is walked through). Put it last on the part, after material and behaviour. A part with its own \
         anim.track, anim.parent or skin cannot be a body. Physics runs only in play mode; editing shows the starting pose.",
        vec![single("in", COLOURABLE)],
        OutputSpec::SameAsInput("in"),
        vec![
            ParamSpec {
                name: "kind",
                ty: ParamType::Choice {
                    options: &["static", "dynamic", "kinematic"],
                },
                default: Some(Value::Str("dynamic".to_owned())),
                doc: "static: never moves. dynamic: falls and collides. kinematic: moved by hand or play, pushes dynamic bodies.",
            },
            ParamSpec {
                name: "shape",
                ty: ParamType::Choice {
                    options: &[
                        "auto", "box", "sphere", "capsule", "cylinder", "convex", "mesh",
                    ],
                },
                default: Some(Value::Str("auto".to_owned())),
                doc: "What it collides as. auto and convex use the part's vertices (a good fit for most solids); box, sphere, capsule and cylinder fit the bounding box (fastest and most stable, best for crates, balls and pillars); mesh is the exact surface for static terrain, floors and buildings only.",
            },
            number(
                "density",
                1.0,
                0.001,
                20_000.0,
                Unit::None,
                "Mass per cubic metre: 1 gives light props; wood is about 600, water 1000, stone 2500. Heavier things push lighter ones.",
            ),
            number(
                "friction",
                0.6,
                0.0,
                3.0,
                Unit::None,
                "Sliding grip: 0 is ice, 0.6 is typical, 1 or more is rubber.",
            ),
            number(
                "restitution",
                0.0,
                0.0,
                1.0,
                Unit::Ratio,
                "Bounciness: 0 stops dead, 0.8 is a rubber ball. A contact bounces as much as the bouncier of the two bodies.",
            ),
            number(
                "linear_damping",
                0.0,
                0.0,
                50.0,
                Unit::None,
                "Air drag: fraction of speed lost per second.",
            ),
            number(
                "angular_damping",
                0.05,
                0.0,
                50.0,
                Unit::None,
                "Fraction of spin lost per second (raise it so rolling things settle).",
            ),
        ],
    )];
    ops.push(joint());
    ops.push(world());
    ops.push(ragdoll());
    ops
}

fn ragdoll() -> Option<OperatorSpec> {
    let text = |name: &'static str, default: &str, max_len: usize, doc: &'static str| ParamSpec {
        name,
        ty: ParamType::Str { max_len },
        default: Some(Value::Str(default.to_owned())),
        doc,
    };
    spec(
        "phys.ragdoll@1",
        "Makes a rigged character (a skeleton of rig.bone parts, each anim.parent-ed to the one above it) able to go LIMP: on command it stops following its animation and falls and \
         tumbles as a ragdoll, colliding with every phys.body in the scene, starting from whatever pose it was in. Put this on any small shape (a marker is fine); `bones` lists the \
         OUTPUT names of the bone parts, parents first, for example \"pelvis; spine; head; arm_l; forearm_l; thigh_l; shin_l\". Every bone must carry rig.bone. Trigger it in play mode with the world \
         call physics.ragdoll {name}, which can add an `impulse` (a shot, an explosion); a game does the same from a script when a creature dies. The floor and walls need phys.body \
         kind=\"static\" or the ragdoll falls through them. Joints are sockets with a swing cone and a twist limit; list elbows and knees in `hinges` to make them hinges that bend one way.",
        vec![single("in", COLOURABLE)],
        OutputSpec::SameAsInput("in"),
        vec![
            text(
                "name",
                "",
                32,
                "The ragdoll's name (letters, digits, _ or -), used to trigger it.",
            ),
            text(
                "bones",
                "",
                512,
                "Output names of the bone parts, separated by semicolons, parents before children (1 to 32).",
            ),
            number(
                "swing",
                1.0,
                0.0,
                3.1416,
                Unit::Angle,
                "Socket freedom: how far a limb may swing away from its parent's line, in degrees (about 60 for shoulders and hips, 30 for a neck).",
            ),
            number(
                "twist",
                0.5,
                0.0,
                3.1416,
                Unit::Angle,
                "How far a limb may twist about its own length either way, in degrees.",
            ),
            text(
                "hinges",
                "",
                512,
                "Output names of the bones that are hinges (elbows, knees), separated by semicolons; the others are sockets.",
            ),
            ParamSpec {
                name: "hinge_limits",
                ty: ParamType::Vec2 {
                    unit: Unit::Angle,
                    min: -3.1416,
                    max: 3.1416,
                },
                default: Some(Value::List(vec![Value::Num(0.0), Value::Num(2.4)])),
                doc: "Lowest and highest hinge angle in degrees: [0, 140] bends one way only, like a knee.",
            },
            vector(
                "hinge_axis",
                [1.0, 0.0, 0.0],
                Unit::None,
                "Which way a straight hinge bone bends about (world axis); a bone that is already bent at rest bends in its own plane. A person standing facing +z has knees [-1, 0, 0].",
            ),
            number(
                "density",
                500.0,
                10.0,
                5000.0,
                Unit::None,
                "Mass per cubic metre of the bones (a person is about 500 to 1000).",
            ),
            number(
                "damping",
                0.3,
                0.0,
                20.0,
                Unit::None,
                "How much the joints resist moving: 0 is limp jelly, 3 is stiff.",
            ),
            number(
                "radius",
                0.0,
                0.0,
                2.0,
                Unit::Length,
                "Thickness of every bone's collision capsule in metres; 0 sizes each from its length.",
            ),
        ],
    )
}
