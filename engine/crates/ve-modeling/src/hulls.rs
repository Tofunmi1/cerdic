//! `solid.hull` and `solid.collision_boxes`: convex hulls and fitted simple shapes, built on
//! `ve-measure`. Both replace their input with a new closed solid.

use std::f64::consts::{FRAC_PI_2, TAU};
use std::rc::Rc;

use glam::{DVec3, Vec3};
use ve_graph::{Diagnostic, Node, Value as Param};
use ve_kernel::{KernelError, Mesh, Profile, Solid};
use ve_measure::{BoundingShape, BoundingShapes, MeasureError, ShapeKind, bounding_shapes, convex_hull};

use crate::operations::kernel_fail;
use crate::{EvaluationWarning, Fail, cache::Value, fail_suggest};

/// Points per full turn of the fitted capsule.
const CAPSULE_SEGMENTS: u32 = 16;
/// Smallest box extent, so a flat fit still encloses a volume.
const MIN_EXTENT: f32 = 1.0e-6;
/// `max_vertices` default, used only if the planner handed us a node without it.
const DEFAULT_MAX_VERTICES: usize = 256;

/// Runs `solid.hull` and `solid.collision_boxes`, or returns `None` for any other operator.
pub(crate) fn run(node: &Node, inputs: &[Rc<Value>]) -> Option<Result<Value, Fail>> {
    match node.op.family().as_str() {
        "solid.hull" => Some(hull(node, inputs)),
        "solid.collision_boxes" => Some(collision_boxes(node, inputs)),
        _ => None,
    }
}

fn first_input<'a>(node: &Node, inputs: &'a [Rc<Value>]) -> Result<&'a Rc<Value>, Fail> {
    inputs.first().ok_or_else(|| {
        fail_suggest(
            "graph.missing_input",
            format!("{} needs a solid", node.op),
            "link the in input to a solid",
        )
    })
}

/// `solid.hull`: the convex hull of the solid's points, as a closed solid.
fn hull(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = first_input(node, inputs)?;
    if input.mesh.is_empty() {
        return Ok(Value::nothing());
    }
    let max_vertices = count(node, "max_vertices", DEFAULT_MAX_VERTICES);
    let points = &input.mesh.positions;
    if points.len() > max_vertices {
        return Err(fail_suggest(
            "solid.hull_too_many_points",
            format!(
                "the solid has {} points, above max_vertices {max_vertices}",
                points.len()
            ),
            "raise max_vertices (up to 2000) or decimate the mesh (poly.decimate) before hulling it"
                .to_owned(),
        ));
    }
    let hull = convex_hull(points).map_err(|error| measure_fail(node, &error))?;
    let mesh = Mesh {
        positions: hull.positions().to_vec(),
        indices: hull.indices().to_vec(),
    };
    let solid = Solid::from_mesh(&mesh).map_err(|error| kernel_fail(node, &error))?;
    let mut value = Value::solid(mesh, solid.volume());
    value.material.clone_from(&input.material);
    Ok(value)
}

/// `solid.collision_boxes`: replace the solid by the fitted box or capsule.
fn collision_boxes(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = first_input(node, inputs)?;
    if input.mesh.is_empty() {
        return Ok(Value::nothing());
    }
    let measured = ve_measure::Mesh::new(&input.mesh.positions, &input.mesh.indices);
    let shapes = bounding_shapes(&measured).map_err(|error| measure_fail(node, &error))?;
    let (kind, shape) = select(node, &shapes);
    let solid = shape_solid(node, kind, &shape)?;
    let mesh = solid.to_mesh().map_err(|error| kernel_fail(node, &error))?;
    let mut value = Value::solid(mesh, solid.volume());
    value.material.clone_from(&input.material);
    value.warnings.push(EvaluationWarning {
        code: "solid.collision_boxes.fit".to_owned(),
        node: node.id.to_string(),
        message: format!(
            "the {} collision shape fits the solid with volume ratio {:.1}",
            label(kind),
            shape.ratio
        ),
    });
    Ok(value)
}

/// The shape the node was asked for, and the fit it corresponds to.
fn select(node: &Node, shapes: &BoundingShapes) -> (ShapeKind, BoundingShape) {
    let kind = match node.params.get("shape") {
        Some(Param::Str(word)) if word == "box" => ShapeKind::Aabb,
        Some(Param::Str(word)) if word == "oriented_box" => ShapeKind::Obb,
        Some(Param::Str(word)) if word == "capsule" => ShapeKind::Capsule,
        _ => shapes.best,
    };
    (kind, shape_of(shapes, kind))
}

fn shape_of(shapes: &BoundingShapes, kind: ShapeKind) -> BoundingShape {
    match kind {
        ShapeKind::Aabb => shapes.aabb,
        ShapeKind::Obb => shapes.obb,
        ShapeKind::Capsule => shapes.capsule,
    }
}

fn label(kind: ShapeKind) -> &'static str {
    match kind {
        ShapeKind::Aabb => "box",
        ShapeKind::Obb => "oriented box",
        ShapeKind::Capsule => "capsule",
    }
}

/// The fitted shape as a solid the kernel accepts.
fn shape_solid(node: &Node, kind: ShapeKind, shape: &BoundingShape) -> Result<Solid, Fail> {
    match kind {
        ShapeKind::Aabb | ShapeKind::Obb => box_solid(node, shape),
        ShapeKind::Capsule => {
            build_capsule(shape).map_err(|error| kernel_fail(node, &error))
        }
    }
}

/// The axis-aligned or oriented box as the hull of its eight corners.
fn box_solid(node: &Node, shape: &BoundingShape) -> Result<Solid, Fail> {
    let centre = Vec3::from_array(shape.centre);
    let axes = shape.axes.map(Vec3::from_array);
    let half = shape
        .half_extents
        .map(|extent| extent.abs().max(MIN_EXTENT));
    let mut corners = Vec::with_capacity(8);
    for bits in 0..8_u32 {
        let mut corner = centre;
        for (axis, offset) in half.iter().enumerate() {
            let sign = if bits & (1 << axis) == 0 { -1.0 } else { 1.0 };
            corner += axes[axis] * (sign * offset);
        }
        corners.push(corner.to_array());
    }
    let hull = convex_hull(&corners).map_err(|error| measure_fail(node, &error))?;
    let mesh = Mesh {
        positions: hull.positions().to_vec(),
        indices: hull.indices().to_vec(),
    };
    Solid::from_mesh(&mesh).map_err(|error| kernel_fail(node, &error))
}

/// A capsule along the fit's axis, revolved from a quarter-circle profile like `solid.capsule`.
fn build_capsule(shape: &BoundingShape) -> Result<Solid, KernelError> {
    let radius = f64::from(shape.radius);
    let straight = (0.5 * f64::from(shape.height) - radius).max(0.0);
    let steps = (CAPSULE_SEGMENTS / 4).max(2);
    let arc = |from: f64, y: f64| {
        (0..=steps).map(move |i| {
            let angle = from + FRAC_PI_2 * f64::from(i) / f64::from(steps);
            [radius * angle.sin(), y - radius * angle.cos()]
        })
    };
    let mut points: Vec<[f64; 2]> = arc(0.0, -straight).collect();
    points.extend(arc(FRAC_PI_2, straight).skip(usize::from(straight <= 0.0)));
    Profile::polygon(&points)?
        .revolve(CAPSULE_SEGMENTS, TAU)?
        .transform(orientation(shape.axes[0], shape.centre))
}

/// A proper rigid 3x4 row-major matrix mapping local Y onto `direction` and the origin onto `centre`.
fn orientation(direction: [f32; 3], centre: [f32; 3]) -> [f64; 12] {
    let direction = DVec3::from_array(direction.map(f64::from));
    let d = direction
        .try_normalize()
        .unwrap_or(DVec3::Y);
    let helper = if d.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let u = helper.cross(d).normalize();
    let w = u.cross(d);
    let c = DVec3::from_array(centre.map(f64::from));
    [
        u.x, d.x, w.x, c.x, u.y, d.y, w.y, c.y, u.z, d.z, w.z, c.z,
    ]
}

/// A `ve-measure` refusal as a diagnostic naming the node.
fn measure_fail(node: &Node, error: &MeasureError) -> Fail {
    let (code, suggestion) = match error {
        MeasureError::PointBudget { .. } => (
            "solid.hull_too_many_points",
            "decimate the mesh (poly.decimate) or lower max_vertices before hulling it",
        ),
        MeasureError::Degenerate(_) => (
            "solid.hull_degenerate",
            "the solid has no volume to hull: check it is closed and not flat",
        ),
        _ => (
            "solid.measure_failed",
            "check the solid is a closed mesh",
        ),
    };
    Box::new(
        Diagnostic::error(code, error.to_string())
            .at_node(&node.id)
            .suggest(suggestion),
    )
}

fn count(node: &Node, name: &str, default: usize) -> usize {
    match node.params.get(name) {
        Some(Param::Int(value)) => usize::try_from(*value).unwrap_or(default),
        _ => default,
    }
}
