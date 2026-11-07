//! `cloth.drape`: a flat panel let fall onto a body (or anything closed) and settled by the cloth solver, at evaluation time.
//!
//! The panel is a polygon mesh, so the result keeps its faces, UVs and layers and only the points move. The collider is any closed mesh
//! (the body, a table), or a ready volume, sampled into a distance grid; pins are a vertex selection (the shoulders, a waistband). The
//! simulation is deterministic (see `ve_world::cloth`), so the same panel on the same body always drapes the same way.

use std::rc::Rc;

use ve_geometry::PolyMesh;
use ve_graph::{Node, Value as Param};
use ve_kernel::ExecutionContext;
use ve_world::cloth::{Collider, SdfGrid, cloth_from_mesh};

use crate::cache::Value;
use crate::poly_ops::{Elements, fingerprint, value_from_poly};
use crate::{Fail, fail, fail_suggest};

/// The most particles a panel may have.
const MAX_PARTICLES: usize = 20_000;
/// The most particle-substeps one drape may run (about two seconds of work).
const MAX_WORK: usize = 250_000_000;

fn number(node: &Node, name: &str) -> Result<f32, Fail> {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .map(|v| v as f32)
        .ok_or_else(|| fail("param.type", format!("{name} is missing or not a number")))
}

fn vector(node: &Node, name: &str) -> Result<[f32; 3], Fail> {
    match node.params.get(name) {
        Some(Param::List(items)) if items.len() == 3 => {
            let mut out = [0.0_f32; 3];
            for (slot, item) in out.iter_mut().zip(items) {
                *slot = item
                    .as_f64()
                    .ok_or_else(|| fail("param.type", format!("{name} must be three numbers")))?
                    as f32;
            }
            Ok(out)
        }
        _ => Err(fail("param.type", format!("{name} must be three numbers"))),
    }
}

pub(crate) fn run(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Option<Result<Value, Fail>> {
    (node.op.family().as_str() == "cloth.drape").then(|| drape(node, inputs, context))
}

fn drape(node: &Node, inputs: &[Rc<Value>], context: &ExecutionContext) -> Result<Value, Fail> {
    // Linked ports arrive in name order; match them back to names.
    let by_name = |name: &str| {
        node.inputs
            .keys()
            .position(|k| k == name)
            .and_then(|at| inputs.get(at))
    };
    let panel = by_name("in")
        .ok_or_else(|| fail("graph.missing_input", "cloth.drape needs a panel on `in`"))?;
    let Some(poly) = panel.poly.as_ref() else {
        return Ok(Value::nothing());
    };
    if panel.mesh.positions.len() != poly.positions().len() {
        return Err(fail(
            "cloth.panel",
            "the panel's drawn points do not match its polygon points; pass it through a plain polygon node first",
        ));
    }
    let particles = poly.positions().len();
    if particles > MAX_PARTICLES {
        return Err(fail_suggest(
            "budget.cloth",
            format!("the panel has {particles} points; the most a drape takes is {MAX_PARTICLES}"),
            "use a coarser panel (fewer columns and rows)",
        ));
    }
    let seconds = number(node, "seconds")?;
    let substeps = number(node, "substeps")? as usize;
    let work = particles
        .saturating_mul((seconds * 60.0) as usize)
        .saturating_mul(substeps);
    if work > MAX_WORK {
        return Err(fail_suggest(
            "budget.cloth",
            format!(
                "{particles} points over {seconds} s at {substeps} substeps is {work} particle-steps (the most is {MAX_WORK})"
            ),
            "lower seconds or substeps, or use a coarser panel",
        ));
    }
    let triangles: Vec<[u32; 3]> = panel
        .mesh
        .indices
        .chunks_exact(3)
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    let mut solver = cloth_from_mesh(
        &panel.mesh.positions,
        &triangles,
        number(node, "density")?,
        number(node, "stretch")?,
        number(node, "bend")?,
    )
    .ok_or_else(|| fail("cloth.panel", "the panel could not be turned into cloth"))?;
    solver.settings.substeps = substeps as u32;
    solver.settings.thickness = number(node, "thickness")?;
    solver.settings.friction = number(node, "friction")?;
    solver.settings.damping = number(node, "damping")?;
    solver.settings.wind = vector(node, "wind")?;
    solver.settings.drag = number(node, "drag")?;
    solver.settings.gravity = [0.0, -number(node, "gravity")?, 0.0];
    if let Some(pins) = by_name("pins") {
        let selection = pins.selection.as_deref().ok_or_else(|| {
            fail(
                "poly.selection_kind",
                "pins needs a selection of vertices from poly.select",
            )
        })?;
        if selection.fingerprint != fingerprint(poly) {
            return Err(fail(
                "selection.stale",
                "the pin selection was made on a different mesh than the panel",
            ));
        }
        let Elements::Vertices(items) = &selection.elements else {
            return Err(fail_suggest(
                "poly.selection_kind",
                format!(
                    "pins needs a selection of vertices but got {}s",
                    selection.elements.kind()
                ),
                "set element=vertex on poly.select",
            ));
        };
        for &i in items {
            solver.pin(i as usize);
        }
    }
    if let Some(body) = by_name("with") {
        let grid = collider_grid(node, body)?;
        solver.colliders.push(Collider::Sdf(grid));
        // A panel that starts inside the body would be thrown out of it; say so instead.
        let margin = solver.settings.thickness;
        let inside = solver
            .pos
            .iter()
            .filter(|p| solver.colliders[0].distance(**p).0 < -margin)
            .count();
        if inside > 0 {
            return Err(fail_suggest(
                "cloth.starts_inside",
                format!("{inside} of the panel's {particles} points start inside the body"),
                "move the panel so it starts outside the body, above or around it, then let it fall",
            ));
        }
    }
    if let Some(floor) = node.params.get("floor").and_then(Param::as_f64) {
        if floor.is_finite() && floor > -1.0e5 {
            solver.colliders.push(Collider::Plane {
                point: [0.0, floor as f32, 0.0],
                normal: [0.0, 1.0, 0.0],
            });
        }
    }
    let frames = (seconds * 60.0).round() as usize;
    for frame in 0..frames {
        if frame % 15 == 0 && context.is_cancelled() {
            return Err(fail(
                "eval.cancelled",
                "the drape was cancelled between frames",
            ));
        }
        if !solver.step(1.0 / 60.0) {
            return Err(fail_suggest(
                "cloth.diverged",
                "the simulation blew up (a position became not-a-number)".to_owned(),
                "raise substeps, lower stretch or bend compliance, or start the panel farther from the body",
            ));
        }
    }
    let mut mesh = PolyMesh::new(
        solver.pos.clone(),
        poly.face_sizes().to_vec(),
        poly.corners().to_vec(),
    )
    .map_err(|error| {
        fail(
            "cloth.result",
            format!("the draped cloth is not a valid mesh: {error}"),
        )
    })?;
    mesh.set_triangulation_mode(ve_geometry::TriangulationMode::CurvedQuads);
    for (name, attribute) in poly.attributes() {
        mesh.set_attribute(name, attribute.clone())
            .map_err(|error| {
                fail(
                    "cloth.result",
                    format!("a layer could not be kept: {error}"),
                )
            })?;
    }
    value_from_poly(node, mesh, panel.material.clone(), context)
}

/// The collider as a distance grid: a ready volume is used as it is, a closed mesh is sampled.
fn collider_grid(node: &Node, body: &Rc<Value>) -> Result<SdfGrid, Fail> {
    let volume = match &body.sdf {
        Some(volume) => Rc::clone(volume),
        None => {
            let cell = f64::from(number(node, "voxel_size")?);
            Rc::new(crate::volume::sample_mesh(node, &body.mesh, cell, 4)?)
        }
    };
    let mut grid = SdfGrid {
        origin: volume.origin.map(|v| v as f32),
        cell: volume.cell as f32,
        dims: volume.dims,
        values: volume.values.clone(),
    };
    // The grid is exact only near the surface; give every point farther out or deeper a direction too, so pressed-in cloth is pushed out.
    grid.fill_far(volume.trunc);
    Ok(grid)
}
