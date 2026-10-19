//! Polygon-mesh nodes: sources, conversion to and from solids, element selection and edits.
//!
//! A polygon value keeps its editable `PolyMesh` and, for drawing, the mesh's properly triangulated
//! form. Selections are made by a rule on the mesh a node receives and travel to the edit that uses
//! them with a fingerprint of that mesh, so a selection can never be applied to a different mesh.

mod blender;
mod convert;
mod edit;
mod errors;
mod lod;
mod params;
mod round;
mod select;
mod selection;
mod subdivision;
mod uv_report;

use crate::{Fail, cache::Value};
use std::rc::Rc;
use ve_graph::Node;
use ve_kernel::ExecutionContext;

pub use errors::suggestion;
pub(crate) use selection::Selection;

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
        "poly.from_solid" => convert::from_solid(node, inputs, context),
        "poly.to_solid" => convert::to_solid(node, inputs, context),
        "poly.subdivide" => subdivision::run(node, inputs, context),
        "poly.select" => select::run(node, inputs, context),
        "poly.round_edges" => round::run(node, inputs, context),
        "poly.lod" => lod::run(node, inputs, context),
        _ => edit::run(node, family, inputs, context),
    }
}
