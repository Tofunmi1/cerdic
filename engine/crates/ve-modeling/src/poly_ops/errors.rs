//! Turning geometry refusals into diagnostics that name the node and the element.

use crate::Fail;
use ve_geometry::{Element, GeometryError, PolyMesh};
use ve_graph::{Diagnostic, Node};

/// The diagnostic for a geometry error raised by `node`. A refusal keeps its stable `poly.*` code and
/// says which face, edge or vertex of the node's input caused it; a face is named by its position in
/// that input and, when the mesh carries a `region` attribute, by the colour region it came from.
pub(super) fn poly_fail(node: &Node, error: &GeometryError, mesh: Option<&PolyMesh>) -> Fail {
    let diagnostic = match error {
        GeometryError::PolyRefusal { code, element } => {
            let detail = element.map_or_else(String::new, |element| describe(element, mesh));
            let diagnostic = Diagnostic::error(code, format!("{}: {code}{detail}", node.op));
            match suggestion(code) {
                Some(text) => diagnostic.suggest(text),
                None => diagnostic,
            }
        }
        GeometryError::Cancelled => Diagnostic::error("eval.cancelled", "evaluation was cancelled"),
        GeometryError::TriangulationUnsupported(why) => Diagnostic::error(
            "poly.triangulation_unsupported",
            format!("{} cannot triangulate a polygon: {why}", node.op),
        ),
        GeometryError::Kernel(kernel) => {
            Diagnostic::error("kernel.invalid_input", format!("{}: {kernel}", node.op))
        }
        other => Diagnostic::error("kernel.invalid_input", format!("{}: {other}", node.op)),
    };
    Box::new(diagnostic.at_node(&node.id))
}

/// What a model or person can change so the refusal does not happen again, by refusal code.
#[must_use]
pub fn suggestion(code: &str) -> Option<&'static str> {
    Some(match code {
        "poly.nonmanifold" | "poly.subdivide.nonmanifold" => {
            "an edge is shared by more than two faces or a vertex pinches two surfaces: split the mesh there, or rebuild from a solid with poly.from_solid"
        }
        "poly.face_degenerate" => {
            "a face has no area: remove it with poly.weld at a small distance, or poly.dissolve_limit"
        }
        "poly.winding_conflict" | "poly.subdivide.winding_conflict" => {
            "neighbouring faces face opposite ways: flip the odd ones with poly.flip_faces, or rebuild with poly.from_solid"
        }
        "poly.flip_increases_inconsistency" => {
            "flip a whole connected piece, not part of one, or set allow_inconsistent=true"
        }
        "poly.selection_empty" | "poly.selection_invalid" => {
            "the selection is empty or does not fit this edit: choose the element kind the edit needs on poly.select (face, edge, vertex or loop)"
        }
        "poly.selection_disconnected" | "poly.selection_splits_component" => {
            "select one connected patch, or run the edit once per patch"
        }
        "poly.loop_invalid" | "poly.loop_not_boundary" | "poly.loop_mismatch" => {
            "loops must be open boundaries with the same number of vertices: select two boundary loops with poly.select element=loop"
        }
        "poly.boundary_edge" => {
            "an edge on an open boundary has only one face: close the mesh first, or select inner edges only"
        }
        "poly.attribute_conflict" => {
            "two faces disagree on an attribute here: merge the attributes first, or remove the attribute you do not need"
        }
        "poly.weld_collapses_face" => "lower the weld distance so no face collapses",
        "poly.weld_pair_budget" | "poly.weld_grid_overflow" => {
            "too many close vertex pairs: weld a smaller area, or lower the distance"
        }
        "poly.bevel_width_invalid" => "give a bevel width above zero, in metres",
        "poly.bevel_segments_invalid" => "use between 1 and 16 segments",
        "poly.angle_invalid" | "poly.tolerance_invalid" => {
            "give an angle or tolerance inside the range written in the operator's parameters"
        }
        "poly.delete_all_faces" => "keep at least one face; select fewer faces",
        "poly.vertex_isolated" => "a vertex has no face left: select whole faces to delete",
        "poly.budget_exceeded" | "poly.bridge_pair_budget" => {
            "the mesh is too large for this edit: decimate it first with poly.decimate, or edit a smaller part"
        }
        "poly.topology_unexpected" => {
            "the mesh does not have the shape this edit needs: check it with poly.from_solid and poly.merge_coplanar first"
        }
        "poly.vertex_unknown" | "poly.edge_unknown" | "selection.stale" => {
            "the selection was made on a different mesh: select again with poly.select on the mesh this edit receives"
        }
        "selection.lost" => {
            "nothing matched: loosen the rule (a larger tolerance or box, a lower angle) or check element and by"
        }
        "selection.ambiguous" => {
            "more than one element matched: narrow the rule (a smaller tolerance or box), or use expect=any"
        }
        "poly.selection_kind" => {
            "set element on poly.select to the kind this edit needs (face, edge, vertex or loop)"
        }
        "poly.select_rule_unsupported" => {
            "that rule does not apply to this element kind: faces take normal, box, attribute, height, extreme; edges take sharp, box, height, extreme; vertices take box, attribute, height, extreme; loops take box, height, extreme"
        }
        _ => return None,
    })
}

/// A refusal with the suggested fix for its code, if there is one.
pub(super) fn refuse(node: &Node, code: &str, message: String) -> Fail {
    let diagnostic = Diagnostic::error(code, format!("{}: {message}", node.op)).at_node(&node.id);
    Box::new(match suggestion(code) {
        Some(text) => diagnostic.suggest(text),
        None => diagnostic,
    })
}

fn describe(element: Element, mesh: Option<&PolyMesh>) -> String {
    match element {
        Element::Face(face) => {
            let region = mesh
                .and_then(|m| m.attributes().get("region"))
                .and_then(|attribute| match &attribute.data {
                    ve_geometry::AttrData::U32(values) => values.get(face).copied(),
                    _ => None,
                });
            region.map_or_else(
                || format!(" (face {face} of the input)"),
                |region| format!(" (face {face} of the input, colour region {region})"),
            )
        }
        Element::Edge(a, b) => format!(" (edge {a}-{b} of the input)"),
        Element::Vertex(vertex) => format!(" (vertex {vertex} of the input)"),
    }
}
