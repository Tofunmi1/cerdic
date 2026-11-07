//! `fx.emitter` and the nodes that refine it: particle effects authored in the language and carried by the part they are put on (plan W4).
//!
//! An emitter node puts an effect on its input shape (or adds an emitter when the shape already has one); `fx.force`, `fx.collide`,
//! `fx.trail` and `fx.sub` change an emitter by name (the last one when no name is given). The runtime simulates the effect with
//! `ve_particles::fx`, placed at the part's own pose each frame, so an effect rides on animated, parented or carried parts.
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use ve_graph::{Node, Value as Param};
use ve_particles::fx::{
    Blend, Burst, Collide, Effect, Emitter, Force, MAX_EMITTERS, Reaction, Render, Shape, Spawn,
    SubEmit, SubTrigger, Trail,
};

use crate::{EvaluationWarning, Fail, Part, cache::Value, fail, fail_suggest};

/// An effect on a part: the emitters, their names (for the nodes that refine them), and the ground height they collide with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartFx {
    /// The effect.
    pub effect: Effect,
    /// The emitters' names, in order.
    pub names: Vec<String>,
    /// World height of the ground plane that colliding emitters hit (`fx.collide ground=`); None when nothing collides.
    pub ground: Option<f32>,
}

fn num(node: &Node, name: &str, default: f64) -> f64 {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .unwrap_or(default)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "effect numbers are small and bounded by the spec"
)]
fn single(node: &Node, name: &str, default: f64) -> f32 {
    num(node, name, default) as f32
}

fn whole(node: &Node, name: &str, default: u32) -> u32 {
    match node.params.get(name) {
        Some(Param::Int(n)) => u32::try_from(*n).unwrap_or(default),
        Some(Param::Num(n)) if *n >= 0.0 => {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "bounded by the spec"
            )]
            let n = *n as u32;
            n
        }
        _ => default,
    }
}

fn text<'a>(node: &'a Node, name: &str, default: &'a str) -> &'a str {
    match node.params.get(name) {
        Some(Param::Str(s)) => s.as_str(),
        _ => default,
    }
}

fn flag(node: &Node, name: &str) -> bool {
    matches!(node.params.get(name), Some(Param::Bool(true)))
}

/// A list parameter as `N` single-precision numbers, `default` where missing.
fn list<const N: usize>(node: &Node, name: &str, default: [f32; N]) -> [f32; N] {
    let Some(Param::List(items)) = node.params.get(name) else {
        return default;
    };
    let mut out = default;
    for (slot, item) in out.iter_mut().zip(items) {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "effect numbers are small and bounded by the spec"
        )]
        {
            *slot = item.as_f64().unwrap_or(f64::from(*slot)) as f32;
        }
    }
    out
}

fn input_of<'a>(label: &str, inputs: &'a [Rc<Value>]) -> Result<&'a Rc<Value>, Fail> {
    inputs.first().ok_or_else(|| {
        fail(
            "graph.missing_input",
            format!("{label} needs a shape to carry the effect"),
        )
    })
}

fn checked(label: &str, effect: &Effect) -> Result<(), Fail> {
    effect
        .validate()
        .map_err(|error| fail("param.range", format!("{label}: {error}")))
}

/// The index of the emitter a refining node means: the one called `emitter`, or the last one when none is named.
fn target(label: &str, fx: &PartFx, node: &Node) -> Result<usize, Fail> {
    let name = text(node, "emitter", "").trim();
    if name.is_empty() {
        return Ok(fx.names.len() - 1);
    }
    index_of(label, fx, name, "emitter")
}

fn index_of(label: &str, fx: &PartFx, name: &str, param: &str) -> Result<usize, Fail> {
    fx.names.iter().position(|n| n == name).ok_or_else(|| {
        fail_suggest(
            "fx.unknown_emitter",
            format!(
                "{label}: no emitter called \"{name}\"; this effect has: {}",
                fx.names.join(", ")
            ),
            format!("set {param} to one of: {}", fx.names.join(", ")),
        )
    })
}

/// A copy of the input's effect, ready to be refined; refuses a shape that has no emitter yet.
fn carried(label: &str, inputs: &[Rc<Value>]) -> Result<(Value, PartFx), Fail> {
    let input = input_of(label, inputs)?;
    let Some(fx) = input.fx.clone() else {
        return Err(fail_suggest(
            "fx.no_emitter",
            format!(
                "{label} refines an effect, and this shape has none: make the emitter first with fx.emitter@1(shape, ...) and pass its result here"
            ),
            "make the emitter first: fx.emitter@1(shape, rate=30, ...), then pass its result here",
        ));
    };
    Ok(((**input).clone(), fx))
}

fn shape_of(node: &Node) -> Shape {
    let extent = list(node, "extent", [0.1; 3]);
    match text(node, "shape", "point") {
        "sphere" => Shape::Sphere { radius: extent[0] },
        "disc" => Shape::Disc { radius: extent[0] },
        "box" => Shape::Box {
            half_extents: extent,
        },
        "line" => Shape::Line {
            length: extent[1] * 2.0,
        },
        _ => Shape::Point,
    }
}

/// `fx.emitter`: puts an emitter on the shape (a second one when the shape already has an effect).
pub(crate) fn emitter(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = input_of("fx.emitter", inputs)?;
    let mut fx = input.fx.clone().unwrap_or(PartFx {
        effect: Effect::default(),
        names: Vec::new(),
        ground: None,
    });
    if fx.names.len() >= MAX_EMITTERS {
        return Err(fail(
            "param.range",
            format!("an effect has at most {MAX_EMITTERS} emitters"),
        ));
    }
    let given = text(node, "name", "").trim();
    let name = if given.is_empty() {
        format!("e{}", fx.names.len())
    } else {
        given.to_owned()
    };
    if name.len() > 32
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(fail(
            "param.range",
            "fx.emitter name: use up to 32 letters, digits, _ or -",
        ));
    }
    if fx.names.contains(&name) {
        return Err(fail_suggest(
            "fx.duplicate_name",
            format!(
                "fx.emitter: an emitter called \"{name}\" already exists on this shape; give each emitter its own name=\"...\""
            ),
            "give each emitter its own name=\"...\"",
        ));
    }
    let burst = whole(node, "burst", 0);
    let spawn = Spawn {
        rate: single(node, "rate", 20.0),
        bursts: if burst > 0 {
            vec![Burst {
                time: single(node, "burst_at", 0.0),
                count: burst,
            }]
        } else {
            Vec::new()
        },
        delay: single(node, "delay", 0.0),
        duration: single(node, "duration", 0.0),
        looping: flag(node, "loop"),
    };
    let mut made = Emitter {
        spawn,
        lifetime: list(node, "lifetime", [1.0, 2.0]),
        shape: shape_of(node),
        speed: list(node, "speed", [1.0, 2.0]),
        direction: list(node, "direction", [0.0, 1.0, 0.0]),
        cone: single(node, "cone", 0.0),
        max_particles: usize::try_from(whole(node, "max_particles", 500)).unwrap_or(500),
        ..Emitter::default()
    };
    made.look.size = list(node, "size", [0.2, 0.2]);
    made.look.colour = [
        list(node, "colour", [1.0; 3]),
        list(node, "colour_end", [1.0; 3]),
    ];
    made.look.alpha = list(node, "alpha", [1.0, 0.0]);
    made.look.glow = single(node, "glow", 0.0);
    made.look.blend = if text(node, "blend", "alpha") == "additive" {
        Blend::Additive
    } else {
        Blend::Alpha
    };
    made.look.render = match text(node, "render", "sprite") {
        "ribbon" => Render::Ribbon,
        "cube" => Render::Cube,
        _ => Render::Sprite,
    };
    made.look.spin = single(node, "spin", 3.0);
    fx.effect.emitters.push(made);
    fx.names.push(name);
    checked("fx.emitter", &fx.effect)?;
    let mut value = (**input).clone();
    value.fx = Some(fx);
    Ok(value)
}

fn force_of(node: &Node) -> Force {
    let vector = list(node, "vector", [0.0; 3]);
    let strength = single(node, "strength", 1.0);
    let centre = list(node, "centre", [0.0; 3]);
    let radius = single(node, "radius", 5.0);
    let or_default = |v: [f32; 3], d: [f32; 3]| if v == [0.0; 3] { d } else { v };
    match text(node, "kind", "gravity") {
        "wind" => Force::Wind {
            velocity: vector,
            strength,
        },
        "drag" => Force::Drag(strength),
        "vortex" => Force::Vortex {
            centre,
            axis: or_default(vector, [0.0, 1.0, 0.0]),
            strength,
            radius,
        },
        "attractor" => Force::Attractor {
            centre,
            strength,
            radius,
        },
        "turbulence" => Force::Turbulence {
            strength,
            scale: single(node, "scale", 1.0),
            speed: single(node, "speed", 0.3),
        },
        _ => Force::Gravity(or_default(vector, [0.0, -9.8, 0.0])),
    }
}

/// `fx.force`: adds a force to an emitter.
pub(crate) fn force(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let (mut value, mut fx) = carried("fx.force", inputs)?;
    let at = target("fx.force", &fx, node)?;
    fx.effect.emitters[at].forces.push(force_of(node));
    checked("fx.force", &fx.effect)?;
    value.fx = Some(fx);
    Ok(value)
}

/// `fx.collide`: makes an emitter's particles hit the ground.
pub(crate) fn collide(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let (mut value, mut fx) = carried("fx.collide", inputs)?;
    let at = target("fx.collide", &fx, node)?;
    let reaction = match text(node, "reaction", "bounce") {
        "stick" => Reaction::Stick,
        "kill" => Reaction::Kill,
        _ => Reaction::Bounce {
            restitution: single(node, "restitution", 0.4),
            friction: single(node, "friction", 0.3),
        },
    };
    fx.effect.emitters[at].collide = Some(Collide {
        radius: single(node, "radius", 0.02),
        reaction,
    });
    fx.ground = Some(single(node, "ground", 0.0));
    checked("fx.collide", &fx.effect)?;
    value.fx = Some(fx);
    Ok(value)
}

/// `fx.trail`: keeps the recent path of an emitter's particles (for ribbons and streaks).
pub(crate) fn trail(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let (mut value, mut fx) = carried("fx.trail", inputs)?;
    let at = target("fx.trail", &fx, node)?;
    fx.effect.emitters[at].trail = Some(Trail {
        points: usize::try_from(whole(node, "points", 12)).unwrap_or(12),
        min_step: single(node, "min_step", 0.05),
    });
    checked("fx.trail", &fx.effect)?;
    value.fx = Some(fx);
    Ok(value)
}

/// Most parts that may emit in one scene (the runtime stages no more).
pub const MAX_EFFECT_PARTS: usize = 64;

/// After evaluation: warns about emitters that cannot show anything yet, and drops the effects of parts beyond the scene's budget so the
/// renderer never has to refuse the whole scene over them.
pub(crate) fn check_parts(parts: &mut [Part]) -> Vec<EvaluationWarning> {
    let warn = |part: &Part, code: &str, message: String| EvaluationWarning {
        code: code.to_owned(),
        node: part.node.to_string(),
        related: Vec::new(),
        message,
    };
    let mut warnings = Vec::new();
    let mut kept = 0;
    for part in parts.iter_mut() {
        let Some(fx) = part.fx.take() else {
            continue;
        };
        kept += 1;
        if kept > MAX_EFFECT_PARTS {
            warnings.push(warn(
                part,
                "fx.budget",
                format!(
                    "a scene may have {MAX_EFFECT_PARTS} emitting parts; {} emits nothing",
                    part.name
                ),
            ));
            continue;
        }
        for (emitter, name) in fx.effect.emitters.iter().zip(&fx.names) {
            if emitter.look.render == Render::Ribbon && emitter.trail.is_none() {
                warnings.push(warn(
                    part,
                    "fx.ribbon_without_trail",
                    format!(
                        "{}: emitter \"{name}\" is a ribbon with no trail, so it draws nothing; add fx.trail@1(..., emitter=\"{name}\")",
                        part.name
                    ),
                ));
            }
        }
        part.fx = Some(fx);
    }
    warnings
}

/// `fx.sub`: makes particles of one emitter spawn particles of another when they are born, die or hit something.
pub(crate) fn sub(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let (mut value, mut fx) = carried("fx.sub", inputs)?;
    let parent = index_of("fx.sub", &fx, text(node, "parent", "").trim(), "parent")?;
    let child = index_of("fx.sub", &fx, text(node, "child", "").trim(), "child")?;
    let on = match text(node, "on", "death") {
        "birth" => SubTrigger::Birth,
        "collide" => SubTrigger::Collide,
        _ => SubTrigger::Death,
    };
    fx.effect.emitters[parent].sub.push(SubEmit {
        on,
        emitter: child,
        count: whole(node, "count", 3),
    });
    checked("fx.sub", &fx.effect)?;
    value.fx = Some(fx);
    Ok(value)
}
