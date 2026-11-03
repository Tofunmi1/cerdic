//! The model-facing measurement tool: a short text summary or the full report as JSON.
//!
//! A model that just built geometry asks "is it closed, how big, how thick". [`measure_mesh`] takes
//! the same flat arrays the browser holds and answers, either as the short text a model reads by
//! default or as a complete JSON document. Every refusal comes back as
//! `{ "error": ..., "suggestion": ... }` rather than a panic.

use std::fmt::Write as _;

use crate::error::MeasureError;
use crate::mesh::{BoundingBox, Mesh};
use crate::problem::ProblemKind;
use crate::report::Report;
use crate::shapes::{BoundingShape, ShapeKind, bounding_shapes};
use crate::symmetry::{Plane, symmetry_error};
use crate::thickness::min_wall_thickness;
use crate::uv::uv_report;

/// How many samples the thickness check casts.
const THICKNESS_SAMPLES: usize = 256;

/// Measure a mesh from flat arrays: positions are xyz triples, indices are triangles into them and
/// `uvs`, when non-empty, are xy pairs with one pair per vertex.
///
/// `detail` is `"full"` for the complete JSON report, anything else for the short text summary. The
/// function never panics: malformed input comes back as `{"error": ..., "suggestion": ...}`.
#[must_use]
pub fn measure_mesh(positions: &[f32], indices: &[u32], uvs: &[f32], detail: &str) -> String {
    let (positions, uvs) = match flat_arrays(positions, uvs) {
        Ok(arrays) => arrays,
        Err(error) => return error_json(&error),
    };
    let mesh = match uvs.as_deref() {
        Some(uvs) => Mesh::with_uvs(&positions, indices, uvs),
        None => Mesh::new(&positions, indices),
    };
    match Report::of(&mesh) {
        Ok(report) if detail == "full" => full_json(&report, &mesh),
        Ok(report) => summary_text(&report, &mesh),
        Err(error) => error_json(&error),
    }
}

/// Split the flat arrays into the shapes [`Mesh`] borrows. Refuses odd lengths instead of dropping
/// the remainder.
fn flat_arrays(
    positions: &[f32],
    uvs: &[f32],
) -> Result<(Vec<[f32; 3]>, Option<Vec<[f32; 2]>>), MeasureError> {
    if !positions.len().is_multiple_of(3) {
        return Err(MeasureError::Degenerate("positions must be xyz triples"));
    }
    let positions = positions
        .chunks_exact(3)
        .map(|corner| [corner[0], corner[1], corner[2]])
        .collect();
    let uvs = if uvs.is_empty() {
        None
    } else {
        if !uvs.len().is_multiple_of(2) {
            return Err(MeasureError::Degenerate("uvs must be xy pairs"));
        }
        Some(uvs.chunks_exact(2).map(|pair| [pair[0], pair[1]]).collect())
    };
    Ok((positions, uvs))
}

/// The 12-line [`Report::summary`] with one thickness line appended.
fn summary_text(report: &Report, mesh: &Mesh) -> String {
    let thickness = match min_wall_thickness(mesh, THICKNESS_SAMPLES) {
        Ok(thickness) => format!(
            "thickness: {:.6} m min, {:.6} m at the 5th percentile",
            thickness.min, thickness.percentile_5
        ),
        Err(_) => "thickness: unknown (no inward wall was found)".to_string(),
    };
    format!("{}\n{thickness}", report.summary())
}

fn error_json(error: &MeasureError) -> String {
    format!(
        "{{\"error\":\"{}\",\"suggestion\":\"{}\"}}",
        escape(&error.to_string()),
        suggestion(error)
    )
}

fn suggestion(error: &MeasureError) -> &'static str {
    match error {
        MeasureError::Empty => "the mesh has no triangles: build geometry first",
        MeasureError::NotTriangles(_) => {
            "indices must come in whole triangles (multiples of three)"
        }
        MeasureError::IndexOutOfRange { .. } => {
            "an index pointed past the vertex list: check the part's indices"
        }
        MeasureError::NonFinite(_) => "a vertex coordinate was not finite: check the geometry",
        MeasureError::UvLength { .. } => "uvs must hold one [u, v] per vertex, or none at all",
        MeasureError::TriangleBudget { .. } => "the mesh is too large to measure in one call",
        MeasureError::PartBudget { .. } => "the scene has too many parts to check at once",
        MeasureError::WorkBudget { .. } => "the parts are too big to check in the browser: measure fewer or smaller parts",
        MeasureError::PointBudget { .. } => "the point set is too large to hull",
        MeasureError::MissingUv => {
            "the mesh has no uvs: unwrap it (poly.unwrap_uv) before measuring UVs"
        }
        MeasureError::NoThickness => "no wall was hit: the part may be open or have no interior",
        MeasureError::Degenerate(_) => "the input is degenerate: check positions and uvs",
        MeasureError::Bake(_) => {
            "check the bake settings (image side 8 to 2048, rays 1 to 256) and that the low mesh is unwrapped (poly.unwrap_uv)"
        }
        MeasureError::MotionFrames { .. }
        | MeasureError::MotionSetting { .. }
        | MeasureError::MotionNonFinite { .. }
        | MeasureError::MotionPointCount { .. }
        | MeasureError::MotionNoPoints
        | MeasureError::MotionLengthMismatch { .. }
        | MeasureError::MotionZeroLength => {
            "check the sampled motion frames, tracked points, and bone endpoints"
        }
    }
}

/// Every field of the report plus thickness, symmetry, UVs (when given) and bounding shapes.
fn full_json(report: &Report, mesh: &Mesh) -> String {
    let mut out = String::from("{");
    let mut first = true;
    field(
        &mut out,
        &mut first,
        "triangles",
        &report.triangle_count.to_string(),
    );
    field(
        &mut out,
        &mut first,
        "vertices",
        &report.vertex_count.to_string(),
    );
    field(
        &mut out,
        &mut first,
        "bounding_box",
        &bounds_json(report.bounding_box),
    );
    field(
        &mut out,
        &mut first,
        "surface_area",
        &num(report.surface_area),
    );
    field(&mut out, &mut first, "volume", &num(report.volume));
    field(
        &mut out,
        &mut first,
        "centre_of_mass",
        &vec3_json(report.centre_of_mass),
    );
    field(
        &mut out,
        &mut first,
        "watertight",
        bool_str(report.watertight),
    );
    field(
        &mut out,
        &mut first,
        "inside_out",
        bool_str(report.inside_out),
    );
    field(
        &mut out,
        &mut first,
        "euler_characteristic",
        &report.euler_characteristic.to_string(),
    );
    field(&mut out, &mut first, "genus", &option_i64(report.genus));
    field(&mut out, &mut first, "components", &components_json(report));
    field(&mut out, &mut first, "problems", &problems_json(report));
    field(&mut out, &mut first, "thickness", &thickness_json(mesh));
    field(&mut out, &mut first, "symmetry", &symmetry_json(mesh));
    field(&mut out, &mut first, "uv", &uv_json(mesh));
    field(&mut out, &mut first, "bounding_shapes", &shapes_json(mesh));
    out.push('}');
    out
}

fn field(out: &mut String, first: &mut bool, key: &str, value: &str) {
    if !*first {
        out.push(',');
    }
    *first = false;
    let _ = write!(out, "\"{key}\":{value}");
}

fn num(value: f32) -> String {
    if value.is_finite() {
        format!("{value}")
    } else {
        "null".to_string()
    }
}

fn bool_str(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn option_i64(value: Option<i64>) -> String {
    value.map_or_else(|| "null".to_string(), |value| value.to_string())
}

fn vec3_json(value: [f32; 3]) -> String {
    format!("[{},{},{}]", num(value[0]), num(value[1]), num(value[2]))
}

fn box_json(min: [f32; 3], max: [f32; 3]) -> String {
    format!("{{\"min\":{},\"max\":{}}}", vec3_json(min), vec3_json(max))
}

fn bounds_json(bounds: BoundingBox) -> String {
    format!(
        "{{\"min\":{},\"max\":{},\"size\":{},\"centre\":{}}}",
        vec3_json(bounds.min),
        vec3_json(bounds.max),
        vec3_json(bounds.size()),
        vec3_json(bounds.centre())
    )
}

fn components_json(report: &Report) -> String {
    let items: Vec<String> = report
        .components
        .iter()
        .map(|component| {
            format!(
                "{{\"triangles\":{},\"volume\":{},\"bounding_box\":{}}}",
                component.triangles,
                num(component.volume),
                box_json(component.bounding_box.min, component.bounding_box.max)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

fn problems_json(report: &Report) -> String {
    let items: Vec<String> = report
        .problems
        .iter()
        .map(|problem| {
            format!(
                "{{\"kind\":\"{}\",\"count\":{},\"example\":{},\"advice\":\"{}\"}}",
                problem_code(problem.kind),
                problem.count,
                option_vec3(problem.example),
                escape(problem.kind.advice())
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

fn problem_code(kind: ProblemKind) -> &'static str {
    match kind {
        ProblemKind::OpenEdge => "open_edge",
        ProblemKind::NonManifoldEdge => "non_manifold_edge",
        ProblemKind::DuplicateVertex => "duplicate_vertex",
        ProblemKind::DegenerateTriangle => "degenerate_triangle",
        ProblemKind::ZeroLengthEdge => "zero_length_edge",
    }
}

fn option_vec3(value: Option<[f32; 3]>) -> String {
    value.map_or_else(|| "null".to_string(), vec3_json)
}

fn thickness_json(mesh: &Mesh) -> String {
    match min_wall_thickness(mesh, THICKNESS_SAMPLES) {
        Ok(thickness) => {
            let worst: Vec<String> = thickness
                .worst
                .iter()
                .map(|hit| {
                    format!(
                        "{{\"position\":{},\"thickness\":{}}}",
                        vec3_json(hit.position),
                        num(hit.thickness)
                    )
                })
                .collect();
            format!(
                "{{\"min\":{},\"percentile_5\":{},\"samples\":{},\"samples_hit\":{},\"worst\":[{}]}}",
                num(thickness.min),
                num(thickness.percentile_5),
                thickness.samples,
                thickness.samples_hit,
                worst.join(",")
            )
        }
        Err(error) => format!("{{\"error\":\"{}\"}}", escape(&error.to_string())),
    }
}

fn symmetry_json(mesh: &Mesh) -> String {
    let items: Vec<String> = Plane::all()
        .into_iter()
        .map(|plane| {
            let key = match plane {
                Plane::X => "x",
                Plane::Y => "y",
                Plane::Z => "z",
            };
            match symmetry_error(mesh, plane) {
                Ok(report) => format!(
                    "\"{key}\":{{\"max\":{},\"mean\":{}}}",
                    num(report.max),
                    num(report.mean)
                ),
                Err(_) => format!("\"{key}\":null"),
            }
        })
        .collect();
    format!("{{{}}}", items.join(","))
}

fn uv_json(mesh: &Mesh) -> String {
    if mesh.uvs.is_none() {
        return "null".to_string();
    }
    match uv_report(mesh) {
        Ok(report) => format!(
            "{{\"islands\":{},\"overlapping_area_ratio\":{},\"coverage\":{},\"texel_density_min\":{},\"texel_density_median\":{},\"texel_density_max\":{},\"stretch_mean\":{},\"stretch_max\":{},\"out_of_range\":{}}}",
            report.islands,
            num(report.overlapping_area_ratio),
            num(report.coverage),
            num(report.texel_density_min),
            num(report.texel_density_median),
            num(report.texel_density_max),
            num(report.stretch_mean),
            num(report.stretch_max),
            report.out_of_range
        ),
        Err(error) => format!("{{\"error\":\"{}\"}}", escape(&error.to_string())),
    }
}

fn shapes_json(mesh: &Mesh) -> String {
    match bounding_shapes(mesh) {
        Ok(shapes) => format!(
            "{{\"best\":\"{}\",\"aabb\":{},\"obb\":{},\"capsule\":{}}}",
            shape_code(shapes.best),
            shape_json(&shapes.aabb),
            shape_json(&shapes.obb),
            shape_json(&shapes.capsule)
        ),
        Err(error) => format!("{{\"error\":\"{}\"}}", escape(&error.to_string())),
    }
}

fn shape_json(shape: &BoundingShape) -> String {
    format!(
        "{{\"kind\":\"{}\",\"centre\":{},\"half_extents\":{},\"axes\":[{},{},{}],\"radius\":{},\"height\":{},\"volume\":{},\"ratio\":{}}}",
        shape_code(shape.kind),
        vec3_json(shape.centre),
        vec3_json(shape.half_extents),
        vec3_json(shape.axes[0]),
        vec3_json(shape.axes[1]),
        vec3_json(shape.axes[2]),
        num(shape.radius),
        num(shape.height),
        num(shape.volume),
        num(shape.ratio)
    )
}

fn shape_code(kind: ShapeKind) -> &'static str {
    match kind {
        ShapeKind::Aabb => "aabb",
        ShapeKind::Obb => "obb",
        ShapeKind::Capsule => "capsule",
    }
}

/// Escape a string for a JSON string literal.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}
