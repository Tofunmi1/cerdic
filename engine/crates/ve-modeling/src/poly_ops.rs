//! Polygon-mesh nodes: sources, conversion to and from solids, element selection and edits.
//!
//! A polygon value keeps its editable `PolyMesh` and, for drawing, the mesh's properly triangulated
//! form. Selections are made by a rule on the mesh a node receives and travel to the edit that uses
//! them with a fingerprint of that mesh, so a selection can never be applied to a different mesh.

mod blender;
mod convert;
mod edit;
mod errors;
mod import;
mod lod;
mod params;
mod place;
mod program;
mod remesh;
mod round;
mod script;
mod script_ops;
mod sculpt;
mod select;
mod selection;
mod subdivision;
mod skin;
mod surface_expr;
mod uv_edit;
mod uv_report;

use crate::{Fail, cache::Value};
use std::rc::Rc;
use ve_graph::Node;
use ve_kernel::ExecutionContext;

pub(crate) use convert::value_from_poly;
pub use errors::suggestion;
pub(crate) use program::{compile, compile_param};

/// A geometry error on a polygon mesh as a diagnostic that names the node and the offending element.
pub(crate) fn poly_failure(
    node: &Node,
    error: &ve_geometry::GeometryError,
    mesh: &ve_geometry::PolyMesh,
) -> Fail {
    errors::poly_fail(node, error, Some(mesh))
}
pub(crate) use selection::{Elements, Selection, fingerprint};

/// Memory held by a polygon mesh, for the cache budget.
pub(crate) fn bytes(mesh: &ve_geometry::PolyMesh) -> usize {
    size_of_val(mesh.positions())
        + size_of_val(mesh.corners())
        + size_of_val(mesh.face_sizes())
        + mesh
            .attributes()
            .values()
            .map(|attribute| attribute.data.len() * 8)
            .sum::<usize>()
}

/// Runs a `poly.*` node, or returns `None` for any other operator.
pub(crate) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Option<Result<Value, Fail>> {
    let family = node.op.family().as_str();
    if family == "poly.bisect" {
        if let Some(poly) = inputs.first().and_then(|v| v.poly.as_ref()) {
            eprintln!(
                "bisect source mode {:?} faces {}",
                poly.triangulation_mode(),
                poly.face_count()
            );
        }
    }
    // A polygon mesh moved, turned or scaled by a plain transform keeps its faces (the solid kernel would refuse an open one).
    if place::applies(family, inputs) {
        return Some(place::run(node, family, inputs, context));
    }
    family
        .starts_with("poly.")
        .then(|| dispatch(node, family, inputs, context))
}

fn dispatch(
    node: &Node,
    family: &str,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    match family {
        "poly.grid" => convert::grid(node, context),
        "poly.import" => import::run(node, context),
        "poly.from_solid" => convert::from_solid(node, inputs, context),
        "poly.to_solid" => convert::to_solid(node, inputs, context),
        "poly.subdivide" => subdivision::run(node, inputs, context),
        "poly.select" => select::run(node, inputs, context),
        "poly.round_edges" => round::run(node, inputs, context),
        "poly.lod" => lod::run(node, inputs, context),
        "poly.program" => program::run(node, inputs, context),
        "poly.script" => script::run(node, inputs, context),
        "poly.remesh_quads" => remesh::run(node, inputs, context),
        "poly.sculpt" => sculpt::run(node, inputs, context),
        "poly.uv_transform" => uv_edit::transform(node, inputs, context),
        "poly.uv_layout" => uv_edit::layout(node, inputs, context),
        "poly.surface_expr" => surface_expr::run(node, context),
        "poly.skin" => skin::run(node, context),
        _ => edit::run(node, family, inputs, context),
    }
}
