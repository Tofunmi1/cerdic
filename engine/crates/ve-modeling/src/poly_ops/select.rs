//! `poly.select`: choose faces, edges, vertices or boundary loops of a polygon mesh by a rule.

use std::rc::Rc;

use ve_geometry::{AttrData, Domain, PolyMesh, Topology};
use ve_graph::Node;
use ve_kernel::ExecutionContext;

use super::errors::poly_fail;
use super::params::{input_error, int, number, text, vector};
use super::selection::{Elements, Selection, fingerprint};
use crate::{Fail, cache::Value};

struct Rule<'a> {
    by: &'a str,
    direction: [f64; 3],
    angle: f64,
    low: [f64; 3],
    high: [f64; 3],
    attribute: &'a str,
    value: i64,
    axis: usize,
    height: f64,
    tolerance: f64,
    highest: bool,
}

impl<'a> Rule<'a> {
    fn read(node: &'a Node) -> Result<Self, Fail> {
        Ok(Self {
            by: text(node, "by")?,
            direction: vector(node, "direction")?,
            angle: number(node, "angle")?,
            low: vector(node, "box_min")?,
            high: vector(node, "box_max")?,
            attribute: text(node, "attribute")?,
            value: int(node, "value")?,
            axis: match text(node, "axis")? {
                "x" => 0,
                "z" => 2,
                _ => 1,
            },
            height: number(node, "height")?,
            tolerance: number(node, "tolerance")?,
            highest: text(node, "side")? == "max",
        })
    }

    fn inside(&self, point: [f64; 3]) -> bool {
        (0..3).all(|axis| point[axis] >= self.low[axis] && point[axis] <= self.high[axis])
    }

    /// Whether the rule reads one coordinate per element (`height`, `extreme`).
    fn by_coordinate(&self) -> bool {
        matches!(self.by, "height" | "extreme")
    }

    /// For `height` and `extreme`: which of the elements' coordinates along the axis are chosen.
    /// `height` keeps those near the target; `extreme` keeps those near the highest (or lowest) coordinate of all.
    fn pick(&self, coordinates: &[f64]) -> Vec<bool> {
        let target = if self.by == "height" {
            self.height
        } else if self.highest {
            coordinates
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max)
        } else {
            coordinates.iter().copied().fold(f64::INFINITY, f64::min)
        };
        coordinates
            .iter()
            .map(|c| (c - target).abs() <= self.tolerance)
            .collect()
    }
}

/// Each face's unit normal (zero for a degenerate face) and centre.
pub(super) fn face_data(mesh: &PolyMesh) -> Vec<([f64; 3], [f64; 3])> {
    let mut start = 0_usize;
    mesh.face_sizes()
        .iter()
        .map(|&size| {
            let corners = &mesh.corners()[start..start + size as usize];
            start += size as usize;
            let points: Vec<[f64; 3]> = corners
                .iter()
                .map(|&v| mesh.positions()[v as usize].map(f64::from))
                .collect();
            let mut sum = [0.0; 3];
            let mut centre = [0.0; 3];
            for (i, a) in points.iter().enumerate() {
                let b = points[(i + 1) % points.len()];
                sum[0] += (a[1] - b[1]) * (a[2] + b[2]);
                sum[1] += (a[2] - b[2]) * (a[0] + b[0]);
                sum[2] += (a[0] - b[0]) * (a[1] + b[1]);
                for axis in 0..3 {
                    centre[axis] += a[axis];
                }
            }
            let length = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
            let normal = if length > 1.0e-18 {
                sum.map(|v| v / length)
            } else {
                [0.0; 3]
            };
            let n = f64::from(u32::try_from(points.len().max(1)).unwrap_or(1));
            (normal, centre.map(|v| v / n))
        })
        .collect()
}

pub(super) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let length = dot(a, a).sqrt();
    if length > 1.0e-18 {
        a.map(|v| v / length)
    } else {
        [0.0; 3]
    }
}

fn attribute_matches(data: &AttrData, index: usize, value: i64) -> bool {
    // The parameter is at most i32::MAX, so it is exact as an f64.
    let target = f64::from(i32::try_from(value).unwrap_or(i32::MAX));
    match data {
        AttrData::U32(v) => v.get(index).is_some_and(|x| i64::from(*x) == value),
        AttrData::I32(v) => v.get(index).is_some_and(|x| i64::from(*x) == value),
        AttrData::F32(v) => v
            .get(index)
            .is_some_and(|x| (f64::from(*x) - target).abs() < 0.5),
        AttrData::F64(v) => v.get(index).is_some_and(|x| (x - target).abs() < 0.5),
    }
}

fn unsupported(node: &Node, element: &str, by: &str) -> Fail {
    super::errors::refuse(
        node,
        "poly.select_rule_unsupported",
        format!("by={by} does not apply to {element}s"),
    )
}

pub(super) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| input_error(node, "a polygon mesh"))?;
    let poly = input
        .poly
        .as_ref()
        .ok_or_else(|| input_error(node, "a polygon mesh with faces"))?;
    let rule = Rule::read(node)?;
    let element = text(node, "element")?;
    let elements = match element {
        "face" => faces(node, poly, &rule)?,
        "edge" => edges(node, poly, &rule, context)?,
        "vertex" => vertices(node, poly, &rule, context)?,
        _ => loops(node, poly, &rule, context)?,
    };
    if elements.len() == 0 {
        return Err(selection_error(
            node,
            "selection.lost",
            format!("no {element} matches by {}", rule.by),
        ));
    }
    if text(node, "expect")? == "one" && elements.len() > 1 {
        return Err(selection_error(
            node,
            "selection.ambiguous",
            format!(
                "{} {element}s match by {}; one was expected",
                elements.len(),
                rule.by
            ),
        ));
    }
    let mut value = Value::nothing();
    value.selection = Some(Rc::new(Selection {
        elements,
        fingerprint: fingerprint(poly),
    }));
    Ok(value)
}

fn selection_error(node: &Node, code: &str, message: String) -> Fail {
    super::errors::refuse(node, code, message)
}

fn faces(node: &Node, poly: &PolyMesh, rule: &Rule) -> Result<Elements, Fail> {
    let data = face_data(poly);
    let attribute = poly.attributes().get(rule.attribute);
    let direction = unit(rule.direction);
    let found: Vec<usize> = match rule.by {
        "height" | "extreme" => {
            let coordinates: Vec<f64> = data.iter().map(|face| face.1[rule.axis]).collect();
            let keep = rule.pick(&coordinates);
            (0..data.len()).filter(|&f| keep[f]).collect()
        }
        "all" => (0..data.len()).collect(),
        "normal" => (0..data.len())
            .filter(|&f| {
                dot(data[f].0, direction) >= rule.angle.cos().min(1.0 - 1.0e-12)
                    && dot(data[f].0, data[f].0) > 0.5
            })
            .collect(),
        "box" => (0..data.len())
            .filter(|&f| rule.inside(data[f].1))
            .collect(),
        "attribute" => match attribute {
            Some(a) if a.domain == Domain::Face => (0..data.len())
                .filter(|&f| attribute_matches(&a.data, f, rule.value))
                .collect(),
            _ => Vec::new(),
        },
        by => return Err(unsupported(node, "face", by)),
    };
    Ok(Elements::Faces(found))
}

fn edges(
    node: &Node,
    poly: &PolyMesh,
    rule: &Rule,
    context: &ExecutionContext,
) -> Result<Elements, Fail> {
    let topology = Topology::build(poly, context).map_err(|e| poly_fail(node, &e, Some(poly)))?;
    let data = face_data(poly);
    let flags = if rule.by_coordinate() {
        let coordinates: Vec<f64> = topology
            .edges()
            .keys()
            .map(|&(a, b)| {
                f64::midpoint(
                    f64::from(poly.positions()[a as usize][rule.axis]),
                    f64::from(poly.positions()[b as usize][rule.axis]),
                )
            })
            .collect();
        rule.pick(&coordinates)
    } else {
        Vec::new()
    };
    let mut found = Vec::new();
    for (ordinal, (&(a, b), info)) in topology.edges().iter().enumerate() {
        let (pa, pb) = (
            poly.positions()[a as usize].map(f64::from),
            poly.positions()[b as usize].map(f64::from),
        );
        let keep = match rule.by {
            "height" | "extreme" => flags.get(ordinal).copied().unwrap_or(false),
            "all" => true,
            "box" => rule.inside([
                f64::midpoint(pa[0], pb[0]),
                f64::midpoint(pa[1], pb[1]),
                f64::midpoint(pa[2], pb[2]),
            ]),
            "sharp" => match info.uses() {
                [first, second] => {
                    dot(data[first.face].0, data[second.face].0)
                        .clamp(-1.0, 1.0)
                        .acos()
                        >= rule.angle
                }
                _ => false,
            },
            by => return Err(unsupported(node, "edge", by)),
        };
        if keep {
            found.push((a, b));
        }
    }
    Ok(Elements::Edges(found))
}

fn vertices(
    node: &Node,
    poly: &PolyMesh,
    rule: &Rule,
    context: &ExecutionContext,
) -> Result<Elements, Fail> {
    let topology = Topology::build(poly, context).map_err(|e| poly_fail(node, &e, Some(poly)))?;
    let attribute = poly.attributes().get(rule.attribute);
    let used: Vec<usize> = topology
        .vertex_faces()
        .iter()
        .enumerate()
        .filter(|(_, faces)| !faces.is_empty())
        .map(|(index, _)| index)
        .collect();
    let flags = if rule.by_coordinate() {
        let coordinates: Vec<f64> = used
            .iter()
            .map(|&index| f64::from(poly.positions()[index][rule.axis]))
            .collect();
        rule.pick(&coordinates)
    } else {
        Vec::new()
    };
    let mut found = Vec::new();
    for (ordinal, &index) in used.iter().enumerate() {
        let keep = match rule.by {
            "height" | "extreme" => flags.get(ordinal).copied().unwrap_or(false),
            "all" => true,
            "box" => rule.inside(poly.positions()[index].map(f64::from)),
            "attribute" => attribute.is_some_and(|a| {
                a.domain == Domain::Point && attribute_matches(&a.data, index, rule.value)
            }),
            by => return Err(unsupported(node, "vertex", by)),
        };
        if keep {
            found.push(u32::try_from(index).unwrap_or(u32::MAX));
        }
    }
    Ok(Elements::Vertices(found))
}

fn loops(
    node: &Node,
    poly: &PolyMesh,
    rule: &Rule,
    context: &ExecutionContext,
) -> Result<Elements, Fail> {
    let topology = Topology::build(poly, context).map_err(|e| poly_fail(node, &e, Some(poly)))?;
    let centres: Vec<[f64; 3]> = topology
        .boundary_loops()
        .iter()
        .map(|boundary| {
            let mut centre = [0.0; 3];
            for &v in boundary {
                for (sum, coordinate) in centre.iter_mut().zip(poly.positions()[v as usize]) {
                    *sum += f64::from(coordinate);
                }
            }
            let n = f64::from(u32::try_from(boundary.len().max(1)).unwrap_or(1));
            centre.map(|v| v / n)
        })
        .collect();
    let flags = if rule.by_coordinate() {
        let coordinates: Vec<f64> = centres.iter().map(|c| c[rule.axis]).collect();
        rule.pick(&coordinates)
    } else {
        Vec::new()
    };
    let mut found = Vec::new();
    for (ordinal, centre) in centres.iter().enumerate() {
        let keep = match rule.by {
            "height" | "extreme" => flags.get(ordinal).copied().unwrap_or(false),
            "all" => true,
            "box" => rule.inside(*centre),
            by => return Err(unsupported(node, "loop", by)),
        };
        if keep {
            found.push(ordinal);
        }
    }
    Ok(Elements::Loops(found))
}
