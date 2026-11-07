//! `fluid.sim`: a liquid that runs live. A block of particles is described (where, how finely, in what container, how thick, which bones push it); the
//! runtime steps `ve_world::fluid` every frame and rebuilds the liquid's surface mesh from the particles. At evaluation the part is that surface at
//! rest, padded with tiny triangles so the runtime's mesh chunks have room for the surface to grow, so the liquid can also be exported and measured.

use std::rc::Rc;

use serde::{Deserialize, Serialize};
use ve_geometry::PolyMesh;
use ve_graph::{Node, Value as Param};
use ve_kernel::ExecutionContext;
use ve_world::fluid::{Apic, Fluid, MAX_APIC_PARTICLES, MAX_PARTICLES, surface};

use crate::cache::Value;
use crate::poly_ops::value_from_poly;
use crate::rig::Segment;
use crate::{EvaluationWarning, Fail, Part, fail, fail_suggest};

/// Most colliding bones one liquid may have.
pub const MAX_COLLIDER_BONES: usize = 16;
/// Most triangles a liquid's surface may have.
pub const MAX_SURFACE_TRIANGLES: usize = 12_000;

/// What the runtime needs to simulate a part as a liquid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FluidSpec {
    /// Corners of the block of liquid at the start, in metres.
    pub min: [f32; 3],
    /// The other corner.
    pub max: [f32; 3],
    /// Distance between particles at rest, in metres.
    pub spacing: f32,
    /// The container: particles stay between these corners.
    pub bounds_min: [f32; 3],
    /// The container's other corner.
    pub bounds_max: [f32; 3],
    /// Acceleration in metres per second squared.
    pub gravity: [f32; 3],
    /// Velocity blending with neighbours, 0 to 1.
    pub viscosity: f32,
    /// Share of sliding speed lost on contact.
    pub friction: f32,
    /// Solver steps per frame.
    pub substeps: u32,
    /// Density passes per step.
    pub iterations: u32,
    /// Output names of the bone parts that push the liquid, as capsules.
    pub collide_bones: Vec<String>,
    /// The rest segment of each collider bone (filled in after evaluation).
    pub segments: Vec<Segment>,
    /// Radius of every collider capsule, in metres.
    pub radius: f32,
    /// Grid size the surface is meshed on, in metres.
    pub cell: f32,
    /// Share of a settled liquid's density at which the surface sits.
    pub level: f32,
    /// Most triangles the surface may have while running.
    pub max_triangles: u32,
    /// Solve it with the APIC grid-and-particle solver instead of the position-based one.
    #[serde(default)]
    pub apic: bool,
}

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

pub(crate) fn run(node: &Node, context: &ExecutionContext) -> Option<Result<Value, Fail>> {
    (node.op.family().as_str() == "fluid.sim").then(|| sim(node, context))
}

fn sim(node: &Node, context: &ExecutionContext) -> Result<Value, Fail> {
    let (min, max) = (vector(node, "min")?, vector(node, "max")?);
    let spacing = number(node, "spacing")?;
    let apic = matches!(node.params.get("method"), Some(Param::Str(m)) if m == "apic");
    let (bounds_min, bounds_max) = (vector(node, "bounds_min")?, vector(node, "bounds_max")?);
    let rest_points = if apic {
        Apic::block(min, max, spacing, Some((bounds_min, bounds_max)))
            .map(|liquid| liquid.pos)
            .ok_or_else(|| {
                fail_suggest(
                    "budget.fluid",
                    format!("a block from {min:?} to {max:?} with particles {spacing} m apart is empty, has more than {MAX_APIC_PARTICLES} particles, or its container is too big for the solver's grid"),
                    "make the block smaller or the spacing larger (10,000 to 30,000 particles is a good size), or give the container tighter bounds_min and bounds_max",
                )
            })?
    } else {
        Fluid::block(min, max, spacing)
            .map(|liquid| liquid.pos)
            .ok_or_else(|| {
                fail_suggest(
                    "budget.fluid",
                    format!("a block from {min:?} to {max:?} with particles {spacing} m apart is empty or has more than {MAX_PARTICLES} particles"),
                    "make the block smaller or the spacing larger (2,000 to 6,000 particles is a good size), or use method=\"apic\" for up to 60,000",
                )
            })?
    };
    let collide_bones: Vec<String> = match node.params.get("collide_bones") {
        Some(Param::Str(list)) => list
            .split([';', ','])
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    };
    if collide_bones.len() > MAX_COLLIDER_BONES {
        return Err(fail(
            "param.range",
            format!(
                "fluid.sim collides with at most {MAX_COLLIDER_BONES} bones, got {}",
                collide_bones.len()
            ),
        ));
    }
    let cell = match number(node, "cell")? {
        c if c > 0.0 => c,
        _ => spacing * 0.7,
    };
    let level = number(node, "level")?;
    let max_triangles = (number(node, "max_triangles")? as usize).clamp(200, MAX_SURFACE_TRIANGLES);
    let spec = FluidSpec {
        min,
        max,
        spacing,
        bounds_min,
        bounds_max,
        gravity: [0.0, -number(node, "gravity")?, 0.0],
        viscosity: number(node, "viscosity")?,
        friction: number(node, "friction")?,
        substeps: number(node, "substeps")? as u32,
        iterations: number(node, "iterations")? as u32,
        collide_bones,
        segments: Vec::new(),
        radius: number(node, "collider_radius")?,
        cell,
        level,
        max_triangles: max_triangles as u32,
        apic,
    };
    let rest = surface(&rest_points, spacing, cell, level).ok_or_else(|| {
        fail(
            "fluid.surface",
            "the block has no surface at this cell size; make cell smaller than spacing",
        )
    })?;
    if rest.triangles.len() > max_triangles {
        return Err(fail_suggest(
            "budget.fluid",
            format!(
                "the liquid's surface has {} triangles but max_triangles is {max_triangles}",
                rest.triangles.len()
            ),
            "raise max_triangles (up to 12,000) or make cell larger",
        ));
    }
    let mut positions = rest.positions.clone();
    let mut sizes = vec![3_u32; rest.triangles.len()];
    let mut corners: Vec<u32> = rest.triangles.iter().flatten().copied().collect();
    // Room to grow: tiny triangles inside the liquid, taken over by the real surface while it runs.
    let centre = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];
    let pad = max_triangles - rest.triangles.len();
    let base = positions.len() as u32;
    for i in 0..pad {
        let k = base + (i * 3) as u32;
        let jitter = (i % 7) as f32 * 1.0e-4;
        positions.push([centre[0] + jitter, centre[1], centre[2]]);
        positions.push([centre[0] + jitter + 0.002, centre[1], centre[2]]);
        positions.push([centre[0] + jitter, centre[1] + 0.002, centre[2]]);
        sizes.push(3);
        corners.extend([k, k + 1, k + 2]);
    }
    let mut mesh = PolyMesh::new(positions, sizes, corners).map_err(|e| {
        fail(
            "fluid.mesh",
            format!("the liquid's surface could not be made into a mesh: {e}"),
        )
    })?;
    mesh.set_triangulation_mode(ve_geometry::TriangulationMode::CurvedQuads);
    let mut value = value_from_poly(node, mesh, None, context)?;
    value.fluid = Some(Rc::new(spec));
    Ok(value)
}

/// Fills each liquid's collider segments from the bone parts it names; drops what cannot work with a warning.
pub(crate) fn resolve(parts: &mut [Part]) -> Vec<EvaluationWarning> {
    let bones: std::collections::HashMap<String, Option<Segment>> =
        parts.iter().map(|p| (p.name.clone(), p.bone)).collect();
    let mut warnings = Vec::new();
    for part in parts.iter_mut() {
        let Some(mut spec) = part.fluid.take() else {
            continue;
        };
        let (mut kept, mut segments) = (Vec::new(), Vec::new());
        for name in std::mem::take(&mut spec.collide_bones) {
            match bones.get(&name) {
                Some(Some(segment)) => {
                    kept.push(name);
                    segments.push(*segment);
                }
                _ => warnings.push(EvaluationWarning {
                    code: "fluid.collide_bone".to_owned(),
                    node: part.node.to_string(),
                    related: Vec::new(),
                    message: format!("{}: collide bone \"{name}\" is not a bone part (it needs rig.bone); it is ignored", part.name),
                }),
            }
        }
        spec.collide_bones = kept;
        spec.segments = segments;
        part.fluid = Some(spec);
    }
    warnings
}
