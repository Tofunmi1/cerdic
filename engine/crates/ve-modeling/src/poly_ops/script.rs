//! `poly.script`: a sandboxed JavaScript program builds a polygon mesh, from nothing or from the mesh linked into `in`.
//!
//! The script sees a `Mesh` builder (`vertex(x, y, z)` returns the vertex number, `face(a, b, c, ...)` adds a polygon) and, when a
//! mesh is linked, `input` with its `points` and `faces`. Its last expression is the result: a `Mesh`, or any object with
//! `points` (a list of `[x, y, z]`) and `faces` (lists of vertex numbers). It has no clock, files or network; `Math.random` is
//! seeded, so the same script and seed always give the same mesh.

use std::rc::Rc;

use serde_json::Value as Json;
use ve_geometry::PolyMesh;
use ve_graph::{Diagnostic, Node};
use ve_kernel::ExecutionContext;

use super::{
    convert::value_from_poly,
    params::{int, text},
};
use crate::script::{Limits, ScriptError, run_with};
use crate::{Fail, cache::Value};

/// The most vertices and the most faces a script may build.
const MAX_ELEMENTS: usize = 200_000;

const HELPERS: &str = "function Mesh() { this.points = []; this.faces = []; }\nMesh.prototype.vertex = function (x, y, z) { this.points.push([x, y, z]); return this.points.length - 1; };\nMesh.prototype.face = function () { this.faces.push(Array.prototype.slice.call(arguments)); return this; };";

pub(super) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let code = text(node, "code")?;
    let seed = u32::try_from(int(node, "seed")?).unwrap_or(1);
    let polls = u64::try_from(int(node, "budget")?).unwrap_or(2_000);
    let source_mesh = inputs.first().and_then(|v| v.poly.as_ref());
    let mut setup = vec![HELPERS.to_owned(), super::script_ops::HELPER.to_owned()];
    setup.push(match source_mesh {
        Some(mesh) => format!("var input = {};", mesh_json(mesh)),
        None => "var input = null;".to_owned(),
    });
    let limits = Limits {
        polls,
        ..Limits::default()
    };
    // The engine's interrupt handler owns its closure, and the evaluation's cancel signal cannot be shared into it without
    // `unsafe`, so a script is bounded by its `budget` (at most about 200 million steps), not by the cancel signal.
    let answer = run_with(
        code,
        seed,
        &limits,
        Rc::new(|| false),
        &setup,
        Some(super::script_ops::host()),
    )
    .map_err(|e| script_failure(node, &e))?;
    let mesh = mesh_from(node, &answer)?;
    value_from_poly(node, mesh, None, context)
}

/// The linked mesh as the script sees it.
fn mesh_json(mesh: &PolyMesh) -> String {
    let mut faces = Vec::with_capacity(mesh.face_sizes().len());
    let mut at = 0;
    for &size in mesh.face_sizes() {
        let end = at + size as usize;
        faces.push(&mesh.corners()[at..end]);
        at = end;
    }
    serde_json::json!({ "points": mesh.positions(), "faces": faces }).to_string()
}

fn script_failure(node: &Node, error: &ScriptError) -> Fail {
    let (code, fix) = if error.stopped {
        (
            "script.budget",
            "make the script do less work, or raise `budget` (up to 20000)",
        )
    } else {
        (
            "script.error",
            "fix the line named; the script is JavaScript with a Mesh builder",
        )
    };
    Box::new(
        Diagnostic::error(code, format!("{}: script error: {error}", node.op))
            .at_node(&node.id)
            .suggest(fix),
    )
}

fn bad_result(node: &Node, message: String) -> Fail {
    Box::new(
        Diagnostic::error("script.result", format!("{}: {message}", node.op))
            .at_node(&node.id)
            .suggest("end the script with a Mesh, or an object with points [[x, y, z], ...] and faces [[a, b, c], ...]"),
    )
}

/// The script's answer (JSON text) as a polygon mesh, checking every number and index.
pub(super) fn mesh_from(node: &Node, answer: &str) -> Result<PolyMesh, Fail> {
    let json: Json = serde_json::from_str(answer).map_err(|_| {
        bad_result(
            node,
            "the script's last expression must be a Mesh or an object, not a plain value"
                .to_owned(),
        )
    })?;
    let points = json
        .get("points")
        .and_then(Json::as_array)
        .ok_or_else(|| bad_result(node, "the result has no `points` list".to_owned()))?;
    let faces = json
        .get("faces")
        .and_then(Json::as_array)
        .ok_or_else(|| bad_result(node, "the result has no `faces` list".to_owned()))?;
    if points.len() > MAX_ELEMENTS || faces.len() > MAX_ELEMENTS {
        return Err(bad_result(
            node,
            format!(
                "the result has {} points and {} faces; the most is {MAX_ELEMENTS} of each",
                points.len(),
                faces.len()
            ),
        ));
    }
    let mut positions = Vec::with_capacity(points.len());
    for (index, point) in points.iter().enumerate() {
        let numbers: Option<Vec<f64>> = point
            .as_array()
            .filter(|p| p.len() == 3)
            .map(|p| p.iter().filter_map(Json::as_f64).collect());
        match numbers {
            Some(n) if n.len() == 3 && n.iter().all(|v| v.is_finite()) => {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "positions are stored as f32"
                )]
                positions.push([n[0] as f32, n[1] as f32, n[2] as f32]);
            }
            _ => {
                return Err(bad_result(
                    node,
                    format!("point {index} is not three finite numbers [x, y, z]"),
                ));
            }
        }
    }
    let mut sizes = Vec::with_capacity(faces.len());
    let mut corners = Vec::new();
    for (index, face) in faces.iter().enumerate() {
        let ids = face.as_array().ok_or_else(|| {
            bad_result(
                node,
                format!("face {index} is not a list of vertex numbers"),
            )
        })?;
        if ids.len() < 3 {
            return Err(bad_result(
                node,
                format!(
                    "face {index} has {} corners; a face needs at least 3",
                    ids.len()
                ),
            ));
        }
        for id in ids {
            let number = id
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .filter(|&n| (n as usize) < positions.len())
                .ok_or_else(|| {
                    bad_result(
                        node,
                        format!(
                            "face {index} uses vertex {id}, but the mesh has {} vertices (0 to {})",
                            positions.len(),
                            positions.len().saturating_sub(1)
                        ),
                    )
                })?;
            corners.push(number);
        }
        sizes.push(u32::try_from(ids.len()).unwrap_or(3));
    }
    let mut mesh = PolyMesh::new(positions, sizes, corners)
        .map_err(|e| bad_result(node, format!("the mesh is not valid: {e}")))?;
    mesh.set_triangulation_mode(ve_geometry::TriangulationMode::CurvedQuads);
    Ok(mesh)
}
