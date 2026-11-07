//! Number values: the `value.*` operators, and filling a node's number parameters from the values linked to them.
//!
//! A link into a port that is not a declared input but names a number parameter (`width`) or one component of a vector
//! parameter (`size.y`) is a socket. Before the node runs, each socket's number replaces that parameter, checked against
//! the parameter's own range; the operator then runs as if the author had typed the number.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::OnceLock;

use ve_graph::{
    Diagnostic, Node, NodeId, OperatorRegistry, ParamType, Value as Param, builtin_registry,
};

use crate::cache::Value;
use crate::{Fail, fail_suggest};

/// The operator specs, built once: looking a node's spec up must not rebuild the registry for every node evaluated.
pub(crate) fn registry() -> &'static OperatorRegistry {
    static REGISTRY: OnceLock<OperatorRegistry> = OnceLock::new();
    REGISTRY.get_or_init(builtin_registry)
}

/// Whether `port` of `node` is a number socket rather than a declared geometry input.
pub(crate) fn is_socket(node: &Node, port: &str) -> bool {
    registry()
        .get(&node.op)
        .is_some_and(|spec| spec.input(port).is_none() && spec.socket(port).is_some())
}

type Values = BTreeMap<NodeId, (String, Rc<Value>)>;

/// `node` with every number socket filled from the value linked to it (borrowed unchanged when it has none).
///
/// # Errors
/// `value.not_a_number` when a link does not carry a number, `value.not_whole` for a fraction into a whole-number
/// parameter, or the parameter's own range error with the node and parameter named.
pub(crate) fn resolve<'a>(node: &'a Node, values: &Values) -> Result<Cow<'a, Node>, Fail> {
    let Some(spec) = registry().get(&node.op) else {
        return Ok(Cow::Borrowed(node));
    };
    let mut filled: Option<Node> = None;
    for (port, links) in &node.inputs {
        if spec.input(port).is_some() {
            continue;
        }
        let (Some(socket), Some(link)) = (spec.socket(port), links.first()) else {
            continue;
        };
        let number = values
            .get(&link.node)
            .and_then(|(_, value)| value.number)
            .ok_or_else(|| {
                fail_suggest(
                    "value.not_a_number",
                    format!("{} gets {port} from {}, which does not produce a number", node.id, link.node),
                    "link a value.number, value.math, value.size or value.point node, or type the number in the parameter",
                )
            })?;
        let target = filled.get_or_insert_with(|| node.clone());
        let current = target.params.get(socket.param.name).cloned();
        let candidate = match socket.component {
            None => whole_or_real(&socket.param.ty, number, node, port)?,
            Some(index) => {
                let mut list = match current {
                    Some(Param::List(items)) => items,
                    _ => Vec::new(),
                };
                while list.len() <= index {
                    list.push(Param::Num(0.0));
                }
                list[index] = Param::Num(number);
                Param::List(list)
            }
        };
        let checked = socket
            .param
            .ty
            .check(&candidate)
            .map_err(|issue| Box::new(Diagnostic::from_issue(issue, Some(&node.id), port)))?;
        target.params.insert(socket.param.name.to_owned(), checked);
    }
    Ok(filled.map_or(Cow::Borrowed(node), Cow::Owned))
}

/// A whole number for a whole-number parameter (refusing a fraction), a real number otherwise.
fn whole_or_real(ty: &ParamType, number: f64, node: &Node, port: &str) -> Result<Param, Fail> {
    if !matches!(ty, ParamType::Int { .. }) {
        return Ok(Param::Num(number));
    }
    if number.fract() != 0.0 || number.abs() > 1.0e15 {
        return Err(fail_suggest(
            "value.not_whole",
            format!(
                "{} needs a whole number for {port}, but the linked value is {number}",
                node.id
            ),
            "round it first with value.math (op=add after a whole value), or use a number parameter instead of a count",
        ));
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a whole number below 1e15 is exact as an i64"
    )]
    Ok(Param::Int(number as i64))
}

fn text<'a>(node: &'a Node, name: &str) -> &'a str {
    match node.params.get(name) {
        Some(Param::Str(value)) => value,
        _ => "",
    }
}

fn real(node: &Node, name: &str) -> f64 {
    node.params.get(name).and_then(Param::as_f64).unwrap_or(0.0)
}

/// Runs a `value.*` node, or returns `None` for any other operator.
pub(crate) fn run(node: &Node, inputs: &[Rc<Value>]) -> Option<Result<Value, Fail>> {
    let family = node.op.family().as_str();
    if !family.starts_with("value.") {
        return None;
    }
    Some(match family {
        "value.number" => Ok(Value::number(real(node, "value"))),
        "value.math" => math(node),
        "value.size" => size(node, inputs),
        "value.point" => point(node, inputs),
        "value.motion" => motion(node, inputs),
        _ => Err(fail_suggest(
            "parse.unknown_op",
            format!("{family} is not a value operator"),
            "the value operators are value.number, value.math, value.size, value.point and value.motion",
        )),
    })
}

fn math(node: &Node) -> Result<Value, Fail> {
    let (a, b) = (real(node, "a"), real(node, "b"));
    let op = text(node, "op");
    if matches!(op, "div" | "mod") && b == 0.0 {
        return Err(fail_suggest(
            "value.divide_by_zero",
            format!("{}: {op} by zero", node.id),
            "change b so it is not zero (link or type a different number)",
        ));
    }
    let result = match op {
        "sub" => a - b,
        "mul" => a * b,
        "div" => a / b,
        "min" => a.min(b),
        "max" => a.max(b),
        "pow" => a.powf(b),
        "mod" => a % b,
        _ => a + b,
    };
    if result.is_finite() && result.abs() <= 1.0e9 {
        Ok(Value::number(result))
    } else {
        Err(fail_suggest(
            "value.not_finite",
            format!("{}: {a} {op} {b} is not a usable number", node.id),
            "keep the result finite and within a billion; check a power with a large exponent or a negative base",
        ))
    }
}

fn size(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let Some(input) = inputs.first().filter(|input| !input.mesh.is_empty()) else {
        return Err(fail_suggest(
            "value.nothing_to_measure",
            format!("{}: its input has no geometry", node.id),
            "link the node that makes the part to measure",
        ));
    };
    let (mut low, mut high) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for point in &input.mesh.positions {
        for axis in 0..3 {
            low[axis] = low[axis].min(point[axis]);
            high[axis] = high[axis].max(point[axis]);
        }
    }
    let extent = [0, 1, 2].map(|axis| f64::from(high[axis] - low[axis]));
    let result = match text(node, "measure") {
        "width" => extent[0],
        "depth" => extent[2],
        "longest" => extent.iter().copied().fold(f64::MIN, f64::max),
        "shortest" => extent.iter().copied().fold(f64::MAX, f64::min),
        "volume" => input.volume,
        "area" => area(&input.mesh),
        _ => extent[1],
    };
    Ok(Value::number(result))
}

/// `value.point`: one coordinate of the head or tail of a bone, or of the min, centre or max of a part's bounding box.
fn point(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let Some(input) = inputs.first() else {
        return Err(fail_suggest(
            "graph.missing_input",
            format!("{} needs a part to read", node.id),
            "link the node that makes the part",
        ));
    };
    let axis = match text(node, "axis") {
        "x" => 0,
        "z" => 2,
        _ => 1,
    };
    let at = text(node, "at");
    let found = match at {
        "head" | "tail" => {
            let segment = input.bone.ok_or_else(|| {
                fail_suggest(
                    "value.not_a_bone",
                    format!("{}: at={at:?} needs the part to be a bone", node.id),
                    "read a body.bone or rig.bone, or use at=min, center or max for any part",
                )
            })?;
            f64::from(
                if at == "head" {
                    segment.head
                } else {
                    segment.tail
                }[axis],
            )
        }
        _ => {
            if input.mesh.positions.is_empty() {
                return Err(fail_suggest(
                    "value.nothing_to_measure",
                    format!("{}: its input has no geometry", node.id),
                    "link the node that makes the part to read",
                ));
            }
            let (mut low, mut high) = (f32::INFINITY, f32::NEG_INFINITY);
            for position in &input.mesh.positions {
                low = low.min(position[axis]);
                high = high.max(position[axis]);
            }
            f64::from(match at {
                "min" => low,
                "max" => high,
                _ => (low + high) * 0.5,
            })
        }
    };
    Ok(Value::number(found))
}

/// `value.motion`: how far a clip moves a part. The part's position tracks in the named clip are added up (the clip's offsets from where the part
/// stands), sampled every sixtieth of a second: the straight horizontal travel from start to end, the horizontal path length, the duration, the
/// speed (travel over duration) and the vertical lift. A walk cycle's travel over its duration is the speed a state machine's `walk@` should say.
fn motion(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    use crate::anim::TrackProperty;
    let Some(input) = inputs.first() else {
        return Err(fail_suggest(
            "graph.missing_input",
            format!("{} needs a part with tracks", node.id),
            "link the node that makes the part and its anim.track",
        ));
    };
    let clip = text(node, "clip");
    let tracks: Vec<&crate::anim::Track> = input
        .animation
        .iter()
        .filter(|t| t.property == TrackProperty::Position && t.clip == clip)
        .collect();
    if tracks.is_empty() {
        return Err(fail_suggest(
            "value.no_motion",
            format!(
                "{}: the part has no position track in the clip {clip:?}",
                node.id
            ),
            "link the part after its position anim.track, and check the clip name (empty is the main clip)",
        ));
    }
    let duration = tracks.iter().map(|t| t.duration()).fold(0.0_f32, f32::max);
    let at = |s: f32| {
        let mut sum = [0.0_f32; 3];
        for t in &tracks {
            // A looping track wraps at its end, so the end of the clip is its last key, not a sample at that time.
            let v = if s >= t.duration() {
                t.keys.last().map_or([0.0; 3], |k| k.value)
            } else {
                t.sample(s)
            };
            for a in 0..3 {
                sum[a] += v[a];
            }
        }
        sum
    };
    let steps = ((duration * 60.0).ceil() as usize).max(1);
    let (mut path, mut lo, mut hi) = (0.0_f64, f32::MAX, f32::MIN);
    let mut previous = at(0.0);
    for i in 0..=steps {
        let p = at(duration * i as f32 / steps as f32);
        path += f64::from(((p[0] - previous[0]).powi(2) + (p[2] - previous[2]).powi(2)).sqrt());
        lo = lo.min(p[1]);
        hi = hi.max(p[1]);
        previous = p;
    }
    let (first, last) = (at(0.0), at(duration));
    let travel = f64::from(((last[0] - first[0]).powi(2) + (last[2] - first[2]).powi(2)).sqrt());
    let result = match text(node, "measure") {
        "path" => path,
        "duration" => f64::from(duration),
        "speed" => travel / f64::from(duration.max(1.0e-6)),
        "lift" => f64::from(hi - lo),
        _ => travel,
    };
    Ok(Value::number(result))
}

/// The surface area of a triangle mesh.
fn area(mesh: &ve_kernel::Mesh) -> f64 {
    mesh.indices
        .chunks_exact(3)
        .map(|tri| {
            let point = |i: u32| {
                mesh.positions
                    .get(i as usize)
                    .map_or([0.0; 3], |p| p.map(f64::from))
            };
            let (p0, p1, p2) = (point(tri[0]), point(tri[1]), point(tri[2]));
            let (e1, e2) = (
                [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]],
                [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]],
            );
            let cross = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            0.5 * (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt()
        })
        .sum()
}
