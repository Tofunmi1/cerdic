//! Graph adapter for Catmull-Clark subdivision.

use std::rc::Rc;

use ve_geometry::{SubdivisionOptions, subdivide};
use ve_graph::{Node, Value as Param};
use ve_kernel::{CornerSharpness, Crease, ExecutionContext};

use super::selection::{Elements, Selection, fingerprint};
use super::{convert::value_from_poly, errors::poly_fail};
use crate::regions::Region;
use crate::{Fail, cache::Value};

pub(super) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let input = port_input(node, inputs, "in")
        .ok_or_else(|| super::params::input_error(node, "a polygon mesh"))?;
    let Some(cage) = input.poly.as_ref() else {
        return Ok(Value::nothing());
    };
    let levels = match node.params.get("levels") {
        Some(Param::Int(value)) => u32::try_from(*value).unwrap_or(0),
        _ => 0,
    };
    let boundary_rule = match node.params.get("boundary") {
        Some(Param::Str(value)) if value == "edges" => 1,
        Some(Param::Str(value)) if value == "edges_and_corners" => 2,
        _ => {
            return Err(Box::new(
                ve_graph::Diagnostic::error(
                    "param.choice",
                    "poly.subdivide boundary must be edges or edges_and_corners",
                )
                .at_node(&node.id),
            ));
        }
    };
    let options = SubdivisionOptions {
        levels,
        boundary_rule,
        crease_method: 0,
        creases: selected_edges(node, port_input(node, inputs, "creases"), cage)?,
        sharp_corners: selected_corners(node, port_input(node, inputs, "corners"), cage)?,
    };
    let result =
        subdivide(cage, options, context).map_err(|error| poly_fail(node, &error, Some(cage)))?;
    let dropped = result.dropped_attributes.clone();
    let triangulated_faces = result.triangulated_faces;
    let mut value = value_from_poly(node, result.mesh, input.material.clone(), context)?;
    value.warnings.extend(input.warnings.iter().cloned());
    if triangulated_faces > 0 {
        value
            .warnings
            .push(crate::EvaluationWarning::concave_face_triangulated(
                node.id.to_string(),
                triangulated_faces,
            ));
    }
    value.warnings.extend(
        dropped
            .into_iter()
            .map(|name| crate::EvaluationWarning::attribute_dropped(node.id.to_string(), &name)),
    );
    if input.regions.len() > 1
        && let Some(mesh) = value.poly.as_deref()
    {
        value.regions = material_runs(mesh, &input.regions);
    }
    Ok(value)
}

fn material_runs(mesh: &ve_geometry::PolyMesh, source: &[Region]) -> Vec<Region> {
    let Some(ve_geometry::Attribute {
        domain: ve_geometry::Domain::Face,
        data: ve_geometry::AttrData::U32(labels),
    }) = mesh.attributes().get("region")
    else {
        return Vec::new();
    };
    let mut runs: Vec<Region> = Vec::new();
    let mut triangle = 0_usize;
    for (face, &size) in mesh.face_sizes().iter().enumerate() {
        let label = labels.get(face).copied().unwrap_or(0) as usize;
        let material = source.get(label).and_then(|region| region.material.clone());
        if runs
            .last()
            .is_none_or(|previous| previous.material != material)
        {
            runs.push(Region {
                first_triangle: triangle,
                material,
            });
        }
        triangle = triangle.saturating_add(size.saturating_sub(2) as usize);
    }
    runs
}

fn port_input<'a>(node: &Node, inputs: &'a [Rc<Value>], port: &str) -> Option<&'a Rc<Value>> {
    let offset: usize = node
        .inputs
        .range(..port.to_owned())
        .map(|(_, links)| links.len())
        .sum();
    node.inputs
        .get(port)
        .filter(|links| !links.is_empty())
        .and_then(|_| inputs.get(offset))
}

fn checked_selection<'a>(
    node: &Node,
    value: Option<&'a Rc<Value>>,
    cage: &ve_geometry::PolyMesh,
    port: &str,
) -> Result<Option<&'a Selection>, Fail> {
    let Some(selection) = value.and_then(|value| value.selection.as_deref()) else {
        return Ok(None);
    };
    if selection.fingerprint != fingerprint(cage) {
        return Err(Box::new(
            ve_graph::Diagnostic::error(
                "selection.stale",
                format!("{port} selection was made on a different polygon cage"),
            )
            .at_node(&node.id),
        ));
    }
    Ok(Some(selection))
}

fn selected_edges(
    node: &Node,
    value: Option<&Rc<Value>>,
    cage: &ve_geometry::PolyMesh,
) -> Result<Vec<Crease>, Fail> {
    let Some(selection) = checked_selection(node, value, cage, "creases")? else {
        return Ok(Vec::new());
    };
    let Elements::Edges(edges) = &selection.elements else {
        return Err(selection_kind(node, "edges", selection));
    };
    let sharpness = number(node, "sharpness")?;
    Ok(edges
        .iter()
        .map(|&(a, b)| Crease {
            vertices: [a, b],
            sharpness,
        })
        .collect())
}

fn selected_corners(
    node: &Node,
    value: Option<&Rc<Value>>,
    cage: &ve_geometry::PolyMesh,
) -> Result<Vec<CornerSharpness>, Fail> {
    let Some(selection) = checked_selection(node, value, cage, "corners")? else {
        return Ok(Vec::new());
    };
    let Elements::Vertices(vertices) = &selection.elements else {
        return Err(selection_kind(node, "vertices", selection));
    };
    let sharpness = number(node, "corner_sharpness")?;
    Ok(vertices
        .iter()
        .map(|&vertex| CornerSharpness { vertex, sharpness })
        .collect())
}

fn number(node: &Node, key: &str) -> Result<f32, Fail> {
    match node.params.get(key) {
        Some(Param::Num(value)) if value.is_finite() && (0.0..=10.0).contains(value) =>
        {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "sharpness is bounded to 0..=10 and stored as OpenSubdiv f32"
            )]
            Ok(*value as f32)
        }
        _ => Err(Box::new(
            ve_graph::Diagnostic::error("param.type", format!("{key} must be from 0 to 10"))
                .at_node(&node.id),
        )),
    }
}

fn selection_kind(node: &Node, wanted: &str, selection: &Selection) -> Fail {
    Box::new(
        ve_graph::Diagnostic::error(
            "poly.selection_kind",
            format!(
                "poly.subdivide needs a selection of {wanted}, got {}",
                selection.elements.kind()
            ),
        )
        .at_node(&node.id),
    )
}
