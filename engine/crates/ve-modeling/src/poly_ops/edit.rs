//! The polygon edit nodes: each applies one `ve-geometry` operator to its mesh and selection.

use std::rc::Rc;

use ve_geometry::{AttrData, Attribute, Domain, PolyMesh};
use ve_graph::Node;
use ve_kernel::ExecutionContext;

use super::convert::value_from_poly;
use super::errors::poly_fail;
use super::params::{count, flag, input_error, number, text, vector};
use super::selection::{Elements, Selection, fingerprint};
use crate::{Fail, cache::Value};

/// Candidate vertex pairs one weld may test.
const WELD_PAIR_BUDGET: usize = 1_000_000;

pub(super) fn run(
    node: &Node,
    family: &str,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| input_error(node, "a polygon mesh"))?;
    let Some(poly) = input.poly.as_ref() else {
        return Ok(Value::nothing());
    };
    let selection = if matches!(family, "poly.weld" | "poly.merge_coplanar")
        || super::blender::ignores_selection(family)
    {
        None
    } else if super::blender::selection_optional(family) {
        // These run on the whole mesh when no selection is wired in.
        inputs
            .get(1)
            .map(|value| chosen(node, Some(value), poly))
            .transpose()?
    } else {
        Some(chosen(node, inputs.get(1), poly)?)
    };
    let mut result = apply(node, family, poly, selection, context)?;
    if family != "poly.bevel_network" && !super::blender::handles(family) {
        // Blender-backed edits decide their own mode: their patches are curved quads.
        result.set_triangulation_mode(poly.triangulation_mode());
    }
    let unwrapped = family == "poly.unwrap_uv";
    let mut value = value_from_poly(node, result, input.material.clone(), context)?;
    if unwrapped && let Some(mesh) = value.poly.as_deref() {
        value
            .warnings
            .extend(super::uv_report::uv_report(node, mesh)?);
    }
    Ok(value)
}

/// The selection on the second input, checked to have been made on this mesh.
fn chosen<'a>(
    node: &Node,
    value: Option<&'a Rc<Value>>,
    poly: &PolyMesh,
) -> Result<&'a Selection, Fail> {
    let selection = value
        .and_then(|v| v.selection.as_deref())
        .ok_or_else(|| input_error(node, "a selection from poly.select"))?;
    if selection.fingerprint != fingerprint(poly) {
        return Err(refusal(
            node,
            "selection.stale",
            "the selection was made on a different mesh than the one it is applied to",
        ));
    }
    Ok(selection)
}

fn refusal(node: &Node, code: &str, message: &str) -> Fail {
    super::errors::refuse(node, code, message.to_owned())
}

fn wrong_kind(node: &Node, wanted: &str, selection: &Selection) -> Fail {
    refusal(
        node,
        "poly.selection_kind",
        &format!(
            "needs a selection of {wanted}s but got {}s (set element on poly.select)",
            selection.elements.kind()
        ),
    )
}

fn faces<'a>(node: &Node, selection: &'a Selection) -> Result<&'a [usize], Fail> {
    match &selection.elements {
        Elements::Faces(items) => Ok(items),
        _ => Err(wrong_kind(node, "face", selection)),
    }
}

/// Exactly one element, or `selection.ambiguous`.
fn exactly_one<T: Copy>(node: &Node, items: &[T], what: &str) -> Result<T, Fail> {
    match items {
        [only] => Ok(*only),
        _ => Err(refusal(
            node,
            "selection.ambiguous",
            &format!("needs exactly one {what} but {} are selected", items.len()),
        )),
    }
}

fn apply(
    node: &Node,
    family: &str,
    poly: &PolyMesh,
    selection: Option<&Selection>,
    context: &ExecutionContext,
) -> Result<PolyMesh, Fail> {
    let fail = |e: ve_geometry::GeometryError| poly_fail(node, &e, Some(poly));
    if family == "poly.weld" {
        return ve_geometry::weld(poly, number(node, "distance")?, WELD_PAIR_BUDGET, context)
            .map(|result| result.mesh)
            .map_err(fail);
    }
    if family == "poly.merge_coplanar" {
        let angle = number(node, "angle")?;
        // Absent in a document written before the parameter existed: those kept the strict behaviour.
        let strict = text(node, "holes").map_or(true, |holes| holes == "refuse");
        return if strict {
            ve_geometry::merge_coplanar(poly, angle, context).map_err(|e| match e {
                ve_geometry::GeometryError::PolyRefusal { .. } => crate::fail_suggest(
                    "poly.loop_invalid",
                    "a patch of coplanar faces around a hole or pinch cannot become one face",
                    "set holes=\"keep\" to leave such a patch as it is and merge the rest",
                ),
                other => fail(other),
            })
        } else {
            ve_geometry::merge_coplanar_keeping_holes(poly, angle, context).map_err(fail)
        };
    }
    if super::blender::handles(family) {
        return super::blender::apply(node, family, poly, selection, context);
    }
    let selection = selection.ok_or_else(|| input_error(node, "a selection"))?;
    match family {
        "poly.flip_faces" => {
            let allow = flag(node, "allow_inconsistent")?;
            ve_geometry::flip_faces(poly, faces(node, selection)?, allow, context)
                .map(|result| result.mesh)
                .map_err(fail)
        }
        "poly.dissolve_faces" => {
            ve_geometry::dissolve_faces(poly, faces(node, selection)?, context).map_err(fail)
        }
        "poly.delete_faces" => {
            ve_geometry::delete_faces(poly, faces(node, selection)?, context).map_err(fail)
        }
        "poly.extrude_faces" => extrude(node, poly, selection, context),
        "poly.inset_faces" => inset(node, poly, selection, context),
        _ => edge_vertex_loop(node, family, poly, selection, context),
    }
}

/// Face attribute naming what an inset or extrusion made: 1 for the moved or inner cap, 2 for the
/// ring or side faces. Selections can ask for it (`by=attribute`, `attribute=role`).
const ROLE: &str = "role";

/// The mesh with a `role` face attribute (zero where absent) so it follows every edit.
fn with_role(poly: &PolyMesh) -> Result<PolyMesh, ve_geometry::GeometryError> {
    let mut copy = poly.clone();
    if !copy.attributes().contains_key(ROLE) {
        copy.set_attribute(
            ROLE,
            Attribute {
                domain: Domain::Face,
                data: AttrData::U32(vec![0; copy.face_count()]),
            },
        )?;
    }
    Ok(copy)
}

fn mark_roles(
    mesh: &mut PolyMesh,
    caps: &[usize],
    sides: &[usize],
) -> Result<(), ve_geometry::GeometryError> {
    let Some(Attribute {
        data: AttrData::U32(current),
        ..
    }) = mesh.attributes().get(ROLE)
    else {
        return Err(ve_geometry::GeometryError::PolyRefusal {
            code: "poly.attribute_conflict",
            element: None,
        });
    };
    let mut roles = current.clone();
    for (faces, role) in [(caps, 1), (sides, 2)] {
        for &face in faces {
            if let Some(slot) = roles.get_mut(face) {
                *slot = role;
            }
        }
    }
    mesh.set_attribute(
        ROLE,
        Attribute {
            domain: Domain::Face,
            data: AttrData::U32(roles),
        },
    )
}

fn inset(
    node: &Node,
    poly: &PolyMesh,
    selection: &Selection,
    context: &ExecutionContext,
) -> Result<PolyMesh, Fail> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "distances are bounded to 1e6 by the parameter type and stored as f32"
    )]
    let distance = number(node, "distance")? as f32;
    let fail = |e: ve_geometry::GeometryError| poly_fail(node, &e, Some(poly));
    let tagged = with_role(poly).map_err(fail)?;
    let mut result = ve_geometry::inset_each(&tagged, faces(node, selection)?, distance, context)
        .map_err(fail)?;
    mark_roles(&mut result.mesh, &result.caps, &result.rings).map_err(fail)?;
    Ok(result.mesh)
}

fn extrude(
    node: &Node,
    poly: &PolyMesh,
    selection: &Selection,
    context: &ExecutionContext,
) -> Result<PolyMesh, Fail> {
    let chosen = faces(node, selection)?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "offsets are bounded to 1e6 by the parameter type and stored as f32"
    )]
    let offset = vector(node, "offset")?.map(|v| v as f32);
    let fail = |e: ve_geometry::GeometryError| poly_fail(node, &e, Some(poly));
    let tagged = with_role(poly).map_err(fail)?;
    let mut result = if flag(node, "individual")? {
        ve_geometry::extrude_each(&tagged, chosen, offset, context)
    } else {
        ve_geometry::extrude_faces(&tagged, chosen, offset, context)
    }
    .map_err(fail)?;
    let sides: Vec<usize> = (poly.face_count()..result.face_count()).collect();
    mark_roles(&mut result, chosen, &sides).map_err(fail)?;
    Ok(result)
}

fn edge_vertex_loop(
    node: &Node,
    family: &str,
    poly: &PolyMesh,
    selection: &Selection,
    context: &ExecutionContext,
) -> Result<PolyMesh, Fail> {
    let fail = |e: ve_geometry::GeometryError| poly_fail(node, &e, Some(poly));
    match (family, &selection.elements) {
        ("poly.dissolve_edge", Elements::Edges(items)) => {
            let edge = exactly_one(node, items, "edge")?;
            ve_geometry::dissolve_edge(poly, edge, context).map_err(fail)
        }
        ("poly.dissolve_vertex", Elements::Vertices(items)) => {
            let vertex = exactly_one(node, items, "vertex")?;
            ve_geometry::dissolve_vertex(poly, vertex, context).map_err(fail)
        }
        ("poly.bridge_loops", Elements::Loops(items)) => match items.as_slice() {
            [first, second] => {
                ve_geometry::bridge_loops(poly, *first, *second, context).map_err(fail)
            }
            _ => Err(refusal(
                node,
                "selection.ambiguous",
                &format!("needs exactly two loops but {} are selected", items.len()),
            )),
        },
        ("poly.bevel_edges", Elements::Edges(items)) => {
            let segments = count(node, "segments")? as usize;
            ve_geometry::bevel_edges(poly, items, number(node, "width")?, segments, context)
                .map_err(fail)
        }
        ("poly.bevel_network", Elements::Edges(items)) => bevel_network(node, poly, items, context),
        ("poly.dissolve_edge" | "poly.bevel_edges" | "poly.bevel_network", _) => {
            Err(wrong_kind(node, "edge", selection))
        }
        ("poly.dissolve_vertex", _) => Err(wrong_kind(node, "vertex", selection)),
        ("poly.bridge_loops", _) => Err(wrong_kind(node, "loop", selection)),
        _ => Err(refusal(
            node,
            "geometry.unsupported_case",
            "no evaluator for this operator",
        )),
    }
}

/// `poly.bevel_network`: Blender's bevel on any selection of manifold edges.
fn bevel_network(
    node: &Node,
    poly: &PolyMesh,
    edges: &[(u32, u32)],
    context: &ExecutionContext,
) -> Result<PolyMesh, Fail> {
    let offset = match text(node, "offset")? {
        "width" => ve_kernel::BevelOffset::Width,
        "depth" => ve_kernel::BevelOffset::Depth,
        "percent" => ve_kernel::BevelOffset::Percent,
        _ => ve_kernel::BevelOffset::Offset,
    };
    let options = ve_geometry::NetworkBevel {
        width: number(node, "width")?,
        segments: usize::try_from(count(node, "segments")?).unwrap_or(usize::MAX),
        profile: number(node, "profile")?,
        offset,
    };
    ve_geometry::bevel_network(poly, edges, &options, context)
        .map_err(|e| poly_fail(node, &e, Some(poly)))
}
