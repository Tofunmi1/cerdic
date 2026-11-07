//! `poly.remesh_quads`: an all-quad mesh of about the requested size from any triangle or polygon mesh, by quadrs (Instant Meshes).
//!
//! quadrs suits smooth organic shapes. It reports no error when it cannot remesh a hard-surface shape: the result then has holes, loose
//! edges or a fraction of the area. So the result is checked here and refused with the numbers when it is not a faithful surface.

use std::rc::Rc;

use quadrs::{Mesh, RemeshOptions, RemeshTarget, Vec3, remesh};
use ve_geometry::PolyMesh;
use ve_graph::Node;
use ve_kernel::ExecutionContext;

use super::{
    convert::value_from_poly,
    errors::{poly_fail, refuse},
    params::{count, input_error, int},
};
use crate::{Fail, cache::Value};

/// The most input triangles one call accepts; about 200,000 take roughly four seconds.
const MAX_TRIANGLES: usize = 200_000;
/// quadrs gives about 4.3 times the faces it is asked for (measured on spheres, a torus and a field-built creature), so it is asked for a quarter.
const TARGET_DIVISOR: usize = 4;
/// The output's area may differ from the input's by this share before the result is called unfaithful.
const AREA_TOLERANCE: f64 = 0.3;

pub(super) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| input_error(node, "a polygon mesh"))?;
    let Some(source) = input.poly.as_ref() else {
        return Ok(Value::nothing());
    };
    let triangles = input.mesh.indices.len() / 3;
    if triangles > MAX_TRIANGLES {
        return Err(refuse(
            node,
            "poly.remesh_too_large",
            format!(
                "the mesh has {triangles} triangles; the most this node takes is {MAX_TRIANGLES} (simplify it first with poly.decimate)"
            ),
        ));
    }
    let faces = count(node, "faces")? as usize;
    let seed = u64::try_from(int(node, "seed")?).unwrap_or(1);
    let mesh = Mesh {
        vertices: input
            .mesh
            .positions
            .iter()
            .map(|p| Vec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2])))
            .collect(),
        faces: input
            .mesh
            .indices
            .chunks_exact(3)
            .map(|t| vec![t[0] as usize, t[1] as usize, t[2] as usize])
            .collect(),
    };
    // quadrs closes a limbed shape on only a share of its seeds (3 to 7 of 12 on a test creature), and a bad layout has holes, so the next
    // seeds are tried in turn: the result is still a pure function of the seed given. Bigger inputs get fewer tries.
    let attempts: u64 = match triangles {
        0..=20_000 => 8,
        20_001..=60_000 => 4,
        _ => 2,
    };
    let mut last = None;
    let mut found = None;
    for k in 0..attempts {
        if context.is_cancelled() {
            return Err(crate::fail("eval.cancelled", "remesh cancelled"));
        }
        let mut options =
            RemeshOptions::new(RemeshTarget::FaceCount((faces / TARGET_DIVISOR).max(1)));
        options.seed = Some(seed.wrapping_add(k));
        let result = remesh(&mesh, &options).map_err(|error| {
            refuse(
                node,
                "poly.remesh_failed",
                format!("the remesher refused the mesh: {error}"),
            )
        })?;
        match check(node, &result, attempts) {
            Ok(()) => {
                found = Some(result);
                break;
            }
            Err(problem) => last = Some(problem),
        }
    }
    let Some(result) = found else {
        return Err(last.unwrap_or_else(|| crate::fail("poly.remesh_failed", "no attempt ran")));
    };
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(result.mesh.vertices.len());
    for v in &result.mesh.vertices {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "positions are stored as f32"
        )]
        positions.push([v.x as f32, v.y as f32, v.z as f32]);
    }
    let mut corners: Vec<u32> = Vec::with_capacity(result.mesh.faces.len() * 4);
    for face in &result.mesh.faces {
        for &i in face {
            corners.push(u32::try_from(i).unwrap_or(0));
        }
    }
    // The result must face the way the input did: a negative volume against a positive one means every face is reversed.
    if signed_volume(&positions, &corners) * signed_volume_of(input) < 0.0 {
        for quad in corners.chunks_exact_mut(4) {
            quad.reverse();
        }
    }
    let sizes = vec![4_u32; corners.len() / 4];
    let mut poly = PolyMesh::new(positions, sizes, corners)
        .map_err(|error| poly_fail(node, &error, Some(source)))?;
    poly.set_triangulation_mode(ve_geometry::TriangulationMode::CurvedQuads);
    value_from_poly(node, poly, input.material.clone(), context)
}

/// Refuses a result that is not a faithful surface, naming what is wrong and what to do.
fn check(node: &Node, result: &quadrs::RemeshResult, attempts: u64) -> Result<(), Fail> {
    let (before, after) = (&result.input, &result.output);
    let mut problems = Vec::new();
    if after.nonmanifold_edge_count > 0 {
        problems.push(format!(
            "{} edges shared by more than two faces",
            after.nonmanifold_edge_count
        ));
    }
    if before.boundary_edge_count == 0 && after.boundary_edge_count > 0 {
        problems.push(format!(
            "{} open edges on a mesh that was closed",
            after.boundary_edge_count
        ));
    }
    if let Some(ratio) = result.area_ratio
        && (ratio - 1.0).abs() > AREA_TOLERANCE
    {
        problems.push(format!(
            "{:.0} percent of the original surface area",
            ratio * 100.0
        ));
    }
    if after.face_count == 0 {
        problems.push("no faces".to_owned());
    }
    if problems.is_empty() {
        return Ok(());
    }
    Err(refuse(
        node,
        "poly.remesh_unfaithful",
        format!(
            "the remesh did not keep the shape after {attempts} seeds ({}). This remesher suits smooth organic shapes; sharp hard-surface edges (a cut box, a machined part) defeat it. Try a different seed or another number of faces, or keep the original mesh",
            problems.join("; ")
        ),
    ))
}

fn signed_volume(positions: &[[f32; 3]], corners: &[u32]) -> f64 {
    let p = |i: u32| positions[i as usize].map(f64::from);
    let mut volume = 0.0;
    for quad in corners.chunks_exact(4) {
        for (a, b, c) in [(quad[0], quad[1], quad[2]), (quad[0], quad[2], quad[3])] {
            let (a, b, c) = (p(a), p(b), p(c));
            volume += a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]);
        }
    }
    volume / 6.0
}

fn signed_volume_of(input: &Value) -> f64 {
    let p = |i: u32| input.mesh.positions[i as usize].map(f64::from);
    input
        .mesh
        .indices
        .chunks_exact(3)
        .map(|t| {
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0])
        })
        .sum::<f64>()
        / 6.0
}
