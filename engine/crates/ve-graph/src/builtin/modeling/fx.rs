use super::{COLOURABLE, number, single, spec, triple};
use crate::spec::{OperatorSpec, OutputSpec, ParamSpec};
use crate::value::{ParamType, Unit, Value};

fn vec2(
    name: &'static str,
    unit: Unit,
    bounds: (f64, f64),
    default: [f64; 2],
    doc: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Vec2 {
            unit,
            min: bounds.0,
            max: bounds.1,
        },
        default: Some(Value::List(vec![
            Value::Num(default[0]),
            Value::Num(default[1]),
        ])),
        doc,
    }
}

fn vec3(name: &'static str, bounds: (f64, f64), default: [f64; 3], doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Vec3 {
            unit: Unit::None,
            min: bounds.0,
            max: bounds.1,
        },
        default: Some(triple(default[0], default[1], default[2])),
        doc,
    }
}

fn choice(name: &'static str, options: &'static [&'static str], doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Choice { options },
        default: Some(Value::Str(options[0].to_owned())),
        doc,
    }
}

fn whole(name: &'static str, bounds: (i64, i64), default: i64, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Int {
            min: bounds.0,
            max: bounds.1,
        },
        default: Some(Value::Int(default)),
        doc,
    }
}

fn text(name: &'static str, doc: &'static str) -> ParamSpec {
    ParamSpec {
        name,
        ty: ParamType::Str { max_len: 32 },
        default: Some(Value::Str(String::new())),
        doc,
    }
}

const EMITTER_NAME: &str = "Name of the emitter to change; empty means the last one made.";

pub(super) fn specs() -> Vec<Option<OperatorSpec>> {
    vec![
        spec(
            "fx.emitter@1",
            "Makes this shape EMIT particles (fire, smoke, sparks, dust, rain, magic): it gives birth to small sprites that fly, fade and die. The \
                     effect rides on the shape, so it follows it when the shape moves (anim.track, anim.parent, carried in a hand). Call it again on the \
                     result to add a second emitter (fire plus smoke); put fx.* AFTER geometry nodes such as xform.move (they rebuild the shape and drop it); name them to refine them with fx.force, fx.collide, fx.trail and fx.sub. \
                     A campfire: fx.emitter(logs, name=\"fire\", rate=60, lifetime=[0.4, 0.9], speed=[0.5, 1.2], cone=15, shape=\"disc\", \
                     extent=[0.15, 0, 0], size=[0.25, 0.05], colour=[1, 0.5, 0.1], colour_end=[0.6, 0.05, 0], alpha=[1, 0], glow=4, blend=\"additive\"), \
                     then fx.force(kind=\"turbulence\", strength=1.5, scale=1.2) on it.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                text(
                    "name",
                    "A name for this emitter (letters, digits, _ or -), used by fx.force, fx.collide, fx.trail and fx.sub. Empty names it e0, e1, ...",
                ),
                number(
                    "rate",
                    Unit::None,
                    (0.0, 5000.0),
                    20.0,
                    "Particles born per second, steadily. 0 for none (use burst).",
                ),
                whole(
                    "burst",
                    (0, 5000),
                    0,
                    "Particles born at once at `burst_at` seconds: an explosion, a splash, a puff. 0 for none.",
                ),
                number(
                    "burst_at",
                    Unit::None,
                    (0.0, 60.0),
                    0.0,
                    "When the burst happens, seconds after the emitter starts.",
                ),
                number(
                    "delay",
                    Unit::None,
                    (0.0, 600.0),
                    0.0,
                    "Seconds before the emitter starts.",
                ),
                number(
                    "duration",
                    Unit::None,
                    (0.0, 600.0),
                    0.0,
                    "Seconds the emitter stays on; 0 is forever.",
                ),
                ParamSpec {
                    name: "loop",
                    ty: ParamType::Bool,
                    default: Some(Value::Bool(false)),
                    doc: "Start over every `duration` seconds (needs a duration above 0): a repeating burst.",
                },
                vec2(
                    "lifetime",
                    Unit::None,
                    (0.05, 60.0),
                    [1.0, 2.0],
                    "How long a particle lives, seconds: [shortest, longest].",
                ),
                vec2(
                    "speed",
                    Unit::None,
                    (0.0, 100.0),
                    [1.0, 2.0],
                    "Birth speed in metres per second: [slowest, fastest].",
                ),
                vec3(
                    "direction",
                    (-1000.0, 1000.0),
                    [0.0, 1.0, 0.0],
                    "The direction particles leave in, in the shape's own frame (it turns with the shape).",
                ),
                number(
                    "cone",
                    Unit::Angle,
                    (0.0, 1.5),
                    0.0,
                    "Spread around the direction, in degrees (about 0 to 85): 0 is a jet, 30 a plume, 85 a fan.",
                ),
                choice(
                    "shape",
                    &["point", "sphere", "box", "disc", "line"],
                    "Where particles are born around the shape: point, inside a sphere, inside a box, on a flat disc, or along a vertical line.",
                ),
                vec3(
                    "extent",
                    (0.0, 1000.0),
                    [0.1, 0.1, 0.1],
                    "Size of the birth shape in metres: sphere and disc use the first number as radius, box uses all three as half-sizes, line is twice the second long.",
                ),
                whole(
                    "max_particles",
                    (1, 20000),
                    500,
                    "Most particles alive at once from this emitter.",
                ),
                vec2(
                    "size",
                    Unit::Length,
                    (0.0, 100.0),
                    [0.2, 0.2],
                    "Sprite size in metres: [at birth, at death]. A ribbon's width.",
                ),
                vec3(
                    "colour",
                    (0.0, 16.0),
                    [1.0, 1.0, 1.0],
                    "Linear colour at birth (values above 1 glow when `glow` is set).",
                ),
                vec3(
                    "colour_end",
                    (0.0, 16.0),
                    [1.0, 1.0, 1.0],
                    "Linear colour at death: fire goes from yellow-white to deep red.",
                ),
                vec2(
                    "alpha",
                    Unit::Ratio,
                    (0.0, 1.0),
                    [1.0, 0.0],
                    "Opacity: [at birth, at death]. [1, 0] fades out.",
                ),
                number(
                    "glow",
                    Unit::None,
                    (0.0, 16.0),
                    0.0,
                    "How much the particle shines on its own (0 none, 4 fire, 8 bright sparks).",
                ),
                choice(
                    "blend",
                    &["alpha", "additive"],
                    "alpha: ordinary transparency (smoke, dust, rain). additive: adds light (fire, sparks, magic).",
                ),
                choice(
                    "render",
                    &["sprite", "ribbon", "cube"],
                    "sprite: a soft round dot per particle. ribbon: a streak along the path each particle took (needs fx.trail). cube: a small LIT box that tumbles (debris, shards, chips; size is its edge, colour its paint, blend is ignored, glow still brightens it).",
                ),
                number(
                    "spin",
                    Unit::None,
                    (0.0, 50.0),
                    3.0,
                    "cube only: how fast each box tumbles, radians per second.",
                ),
            ],
        ),
        spec(
            "fx.force@1",
            "Adds a force to an emitter's particles: gravity, wind, drag, a vortex, an attractor, or turbulence (swirling curl noise with no sources \
                     or sinks: the way to make smoke and fire look alive). Several forces add up. Positions are world coordinates.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                text("emitter", EMITTER_NAME),
                choice(
                    "kind",
                    &[
                        "gravity",
                        "wind",
                        "drag",
                        "vortex",
                        "attractor",
                        "turbulence",
                    ],
                    "gravity: constant acceleration (`vector`, default [0, -9.8, 0]). wind: drags particles towards the speed `vector` at `strength` per second. drag: slows them (`strength`). vortex: swirls them round the axis `vector` through `centre` (`strength`, `radius`). attractor: pulls them to `centre` (`strength`, `radius`). turbulence: swirling noise (`strength`, `scale`, `speed`).",
                ),
                vec3(
                    "vector",
                    (-1000.0, 1000.0),
                    [0.0, 0.0, 0.0],
                    "gravity: the acceleration (0 means [0, -9.8, 0]). wind: the wind velocity. vortex: the axis (0 means up).",
                ),
                vec3(
                    "centre",
                    (-100_000.0, 100_000.0),
                    [0.0, 0.0, 0.0],
                    "vortex and attractor: the world position.",
                ),
                number(
                    "strength",
                    Unit::None,
                    (0.0, 1000.0),
                    1.0,
                    "How strong: drag and wind per second, vortex, attractor and turbulence in metres per second squared.",
                ),
                number(
                    "radius",
                    Unit::Length,
                    (0.01, 1000.0),
                    5.0,
                    "vortex and attractor: how far the force reaches; it fades towards the edge.",
                ),
                number(
                    "scale",
                    Unit::None,
                    (0.01, 100.0),
                    1.0,
                    "turbulence: features per metre (0.5 big slow eddies, 3 fine ones).",
                ),
                number(
                    "speed",
                    Unit::None,
                    (0.0, 100.0),
                    0.3,
                    "turbulence: how fast the pattern changes.",
                ),
            ],
        ),
        spec(
            "fx.collide@1",
            "Makes an emitter's particles hit the ground (a flat plane at `ground`): they bounce, stick or disappear. Sparks bounce, rain splashes \
                     (kill, with fx.sub on collide), embers stick.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                text("emitter", EMITTER_NAME),
                choice(
                    "reaction",
                    &["bounce", "stick", "kill"],
                    "bounce: rebound with `restitution` and slide with `friction`. stick: stop where they land. kill: disappear on impact.",
                ),
                number(
                    "radius",
                    Unit::Length,
                    (0.0, 10.0),
                    0.02,
                    "The particle's size for collisions, metres.",
                ),
                number(
                    "restitution",
                    Unit::Ratio,
                    (0.0, 1.0),
                    0.4,
                    "bounce: how much speed a rebound keeps (0 dead, 1 perfect).",
                ),
                number(
                    "friction",
                    Unit::None,
                    (0.0, 10.0),
                    0.3,
                    "bounce: how much a slide is slowed (0 ice, 1 rough).",
                ),
                number(
                    "ground",
                    Unit::Length,
                    (-1000.0, 1000.0),
                    0.0,
                    "World height of the ground plane (applies to the whole effect).",
                ),
            ],
        ),
        spec(
            "fx.trail@1",
            "Makes an emitter's particles remember the path they took (the last `points` positions). Needed for render=\"ribbon\": streaks, comet \
                     tails, sword swooshes, tracer bullets.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                text("emitter", EMITTER_NAME),
                whole(
                    "points",
                    (2, 64),
                    12,
                    "How many positions to keep (the newest is the particle itself).",
                ),
                number(
                    "min_step",
                    Unit::Length,
                    (0.005, 10.0),
                    0.05,
                    "A point is recorded after the particle moves this far, metres: small is smooth, large is angular.",
                ),
            ],
        ),
        spec(
            "fx.sub@1",
            "Makes particles of one emitter spawn particles of another emitter of the same effect when they are born, die or hit the ground: \
                     smoke from dying embers, a splash from each raindrop (parent kill-on-collide), sparks from a fuse. The child emitter usually has rate=0.",
            vec![single("in", COLOURABLE)],
            OutputSpec::SameAsInput("in"),
            vec![
                text("parent", "Name of the emitter whose particles trigger it."),
                text(
                    "child",
                    "Name of the emitter that spawns particles where the trigger happens.",
                ),
                choice(
                    "on",
                    &["death", "birth", "collide"],
                    "When: death (dies of old age), birth, or collide (hits the ground).",
                ),
                whole("count", (1, 1000), 3, "Particles spawned each time."),
            ],
        ),
    ]
}
