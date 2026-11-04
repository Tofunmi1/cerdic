//! The deformers on a polygon mesh: every vertex goes through the same point map a solid's vertices do, with no refinement.
//!
//! A solid is refined where the map bends it (tolerance) and then checked as a solid. A polygon mesh is moved exactly as it is:
//! the detail the result has is the detail the input had, so give it enough vertices (a denser grid, `poly.subdivide_edges`) for a
//! strong bend, and join the edges of a closed result back with `poly.remove_doubles` if the map pulled neighbours together.

use std::rc::Rc;

use ve_geometry::PolyMesh;
use ve_graph::Node;
use ve_kernel::ExecutionContext;

use super::{Map, to_f32, validate_bend};
use crate::{Fail, cache::Value, fail, poly_ops};

pub(super) fn build(
    node: &Node,
    input: &Rc<Value>,
    poly: &PolyMesh,
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for position in poly.positions() {
        for axis in 0..3 {
            let value = f64::from(position[axis]);
            low[axis] = low[axis].min(value);
            high[axis] = high[axis].max(value);
        }
    }
    let map = Map::from_node(node)?;
    if let Some(angle) = map.bend_angle() {
        validate_bend(node, angle, low, high)?;
    }
    let mut moved = Vec::with_capacity(poly.positions().len());
    for (index, position) in poly.positions().iter().enumerate() {
        if index % 512 == 0 && context.is_cancelled() {
            return Err(fail(
                "eval.cancelled",
                "deformation cancelled between vertices",
            ));
        }
        moved.push(to_f32(map.apply(position.map(f64::from), low, high))?);
    }
    let mut mesh = PolyMesh::new(moved, poly.face_sizes().to_vec(), poly.corners().to_vec())
        .map_err(|error| poly_ops::poly_failure(node, &error, poly))?;
    // A deformed face is no longer flat, so the result is drawn as a surface, like terrain.
    mesh.set_triangulation_mode(ve_geometry::TriangulationMode::CurvedQuads);
    for (name, attribute) in poly.attributes() {
        mesh.set_attribute(name, attribute.clone())
            .map_err(|error| poly_ops::poly_failure(node, &error, poly))?;
    }
    poly_ops::value_from_poly(node, mesh, input.material.clone(), context)
}
