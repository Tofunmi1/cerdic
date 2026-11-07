//! `sim.program`: a simulation written in SimL. At evaluation the program is compiled (every mistake is a refusal with line, column and
//! fix) and run for no steps to read its start state. A program that ends in `output particles` makes the part one small tetrahedron per
//! particle; one that ends in `output mesh` drives the vertices of the mesh given on `in`. The runtime steps the same program at a fixed
//! rate, feeding it the poses of the bones it names.

use std::rc::Rc;

use serde::{Deserialize, Serialize};
use ve_geometry::PolyMesh;
use ve_graph::{Node, Value as Param};
use ve_kernel::ExecutionContext;
use ve_world::sim::{MeshData, Sim, compile, library};

use crate::cache::Value;
use crate::poly_ops::value_from_poly;
use crate::{Fail, fail, fail_suggest};

/// Most particles one program may draw as tetrahedra.
pub const MAX_PARTICLES: usize = 4000;
/// Most points of a mesh a program may drive.
pub const MAX_MESH_POINTS: usize = 20_000;

/// The corners of one particle's tetrahedron around its centre, for a size of 1.
pub const TETRA_CORNERS: [[f32; 3]; 4] = [[0.5, 0.5, 0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, -0.5], [0.5, -0.5, -0.5]];
/// Its four triangles (counter-clockwise from outside) as indices into [`TETRA_CORNERS`].
pub const TETRA_FACES: [[usize; 3]; 4] = [[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]];

/// What the runtime needs to run a part as a simulation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimSpec {
    /// The SimL program.
    pub source: String,
    /// Seed of `hash` and `rand`.
    pub seed: u32,
    /// Steps per second.
    pub rate: f32,
    /// Width of each particle's tetrahedron, in metres (particles mode).
    pub size: f32,
    /// Name of the state array drawn.
    pub particles: String,
    /// How many particles or mesh points it has.
    pub count: u32,
    /// `particles` (tetrahedra) or `mesh` (the vertices of the input mesh).
    pub mode: String,
    /// Output names of the bone parts the program reads (`input bone`), resolved in the part's scope at run time.
    pub bones: Vec<String>,
    /// Settings for the program's `param`s (name, numbers), applied after the program is made.
    #[serde(default)]
    pub values: Vec<(String, Vec<f32>)>,
}

/// Parses `name=1.5; other=0,1,2` into settings.
fn parse_values(text: &str) -> Result<Vec<(String, Vec<f32>)>, Fail> {
    let mut out = Vec::new();
    for item in text.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, numbers) = item.split_once('=').ok_or_else(|| fail_suggest("sim.values", format!("`{item}` has no `=`"), "write `name=number` or `name=x,y,z`"))?;
        let parsed: Result<Vec<f32>, _> = numbers.split(',').map(|n| n.trim().parse::<f32>()).collect();
        let parsed = parsed.map_err(|_| fail_suggest("sim.values", format!("`{item}` is not numbers"), "write `name=1.5` or `name=0,0.7,-0.05`"))?;
        out.push((name.trim().to_owned(), parsed));
    }
    Ok(out)
}

fn number(node: &Node, name: &str) -> Result<f64, Fail> {
    node.params.get(name).and_then(Param::as_f64).ok_or_else(|| fail("param.type", format!("{name} is missing or not a number")))
}

pub(crate) fn run(node: &Node, inputs: &[Rc<Value>], context: &ExecutionContext) -> Option<Result<Value, Fail>> {
    (node.op.family().as_str() == "sim.program").then(|| program(node, inputs, context))
}

fn source_of(node: &Node) -> Result<String, Fail> {
    let Some(Param::Str(source)) = node.params.get("source") else {
        return Err(fail("param.type", "source is missing or not text"));
    };
    if !source.trim().is_empty() {
        return Ok(source.clone());
    }
    match node.params.get("library") {
        Some(Param::Str(name)) if name == "cloth" => Ok(library::CLOTH.to_owned()),
        Some(Param::Str(name)) if name == "bridge" => Ok(library::BRIDGE.to_owned()),
        _ => Err(fail_suggest("sim.source", "the program is empty", "write the SimL text in `source`, or set library=\"cloth\" or \"bridge\"")),
    }
}

fn program(node: &Node, inputs: &[Rc<Value>], context: &ExecutionContext) -> Result<Value, Fail> {
    let source = source_of(node)?;
    let compiled = compile(&source).map_err(|diagnostics| {
        let d = &diagnostics[0];
        fail_suggest("sim.compile", format!("line {}, column {}: {}", d.line, d.column, d.message), d.suggestion.clone())
    })?;
    let Some((kind, drawn)) = compiled.outputs().into_iter().find(|(kind, _)| *kind == "particles" || *kind == "mesh") else {
        return Err(fail_suggest("sim.output", "the program draws nothing", "add `output particles pos` (tetrahedra) or `output mesh pos` (drives the `in` mesh)"));
    };
    let (kind, drawn) = (kind.to_owned(), drawn.to_owned());
    let bones: Vec<String> = compiled.bone_inputs().into_iter().map(str::to_owned).collect();
    let seed = number(node, "seed")? as u32;
    let (rate, size) = (number(node, "rate")? as f32, number(node, "size")? as f32);
    let given = node.inputs.keys().position(|k| k == "in").and_then(|at| inputs.get(at));
    let mesh_inputs = compiled.mesh_inputs().len();
    let (start, base) = if mesh_inputs == 0 {
        (Sim::new(compiled, seed), None)
    } else {
        let garment = given.ok_or_else(|| fail_suggest("graph.missing_input", "this program reads a mesh but `in` is empty", "link a polygon mesh (poly.grid, a rope or hair strip) to `in`"))?;
        if mesh_inputs > 1 {
            return Err(fail("sim.mesh_inputs", "a sim.program binds one mesh; the program declares more"));
        }
        if garment.mesh.positions.len() > MAX_MESH_POINTS {
            return Err(fail_suggest("budget.sim", format!("the mesh has {} points; a program drives at most {MAX_MESH_POINTS}", garment.mesh.positions.len()), "use a coarser mesh"));
        }
        let edges: Vec<(u32, u32)> = garment.mesh.indices.chunks_exact(3).flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])]).collect();
        let data = MeshData::new(garment.mesh.positions.clone(), &edges);
        (Sim::with_meshes(compiled, seed, vec![data]), Some(garment))
    };
    let mut start = start.map_err(|e| fail_suggest("sim.init", e.to_string(), "fix the `init` kernel or the state sizes"))?;
    let values = match node.params.get("values") {
        Some(Param::Str(text)) => parse_values(text)?,
        _ => Vec::new(),
    };
    for (name, numbers) in &values {
        start.set_param(name, numbers).map_err(|e| fail_suggest("sim.values", e.message, "name a `param` of the program with the right number of numbers"))?;
    }
    start.reset().map_err(|e| fail_suggest("sim.init", e.to_string(), "fix the `init` kernel or the state sizes"))?;
    let at = start.state(&drawn).ok_or_else(|| fail("sim.output", "the drawn state array is missing"))?;
    let count = at.len() / 3;
    let spec = SimSpec { source, seed, rate, size, particles: drawn, count: u32::try_from(count).unwrap_or(u32::MAX), mode: kind.clone(), bones, values };
    if kind == "mesh" {
        let Some(garment) = base else {
            return Err(fail_suggest("graph.missing_input", "`output mesh` needs a mesh on `in`", "link a polygon mesh to `in`"));
        };
        let mut value = (**garment).clone();
        value.sim = Some(Rc::new(spec));
        return Ok(value);
    }
    if count > MAX_PARTICLES {
        return Err(fail_suggest(
            "budget.sim",
            format!("the program draws {count} particles; this node draws at most {MAX_PARTICLES}"),
            "draw fewer particles, or make the count a `param` and lower it",
        ));
    }
    let mut positions = Vec::with_capacity(count * 12);
    let mut corners = Vec::with_capacity(count * 12);
    for centre in at.chunks(3) {
        for face in TETRA_FACES {
            for corner in face {
                let c = TETRA_CORNERS[corner];
                corners.push(u32::try_from(positions.len()).unwrap_or(0));
                positions.push([centre[0] + c[0] * size, centre[1] + c[1] * size, centre[2] + c[2] * size]);
            }
        }
    }
    let mesh = PolyMesh::new(positions, vec![3; count * 4], corners).map_err(|e| fail("sim.mesh", format!("the particles could not be made into a mesh: {e}")))?;
    let mut value = value_from_poly(node, mesh, None, context)?;
    value.sim = Some(Rc::new(spec));
    Ok(value)
}
