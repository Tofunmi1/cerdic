//! The report: size, volume, topology problems and a short text summary.

use std::collections::{BTreeMap, HashMap, HashSet};

use glam::Vec3;

use crate::error::MeasureError;
use crate::mesh::{AREA_EPSILON, BoundingBox, Dsu, LENGTH_EPSILON, Mesh, VOLUME_EPSILON};
use crate::problem::{Component, Problem, ProblemKind};

/// Everything a model needs to know about its own geometry.
///
/// Derives [`PartialEq`]: the same input gives an identical report.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// Number of triangles.
    pub triangle_count: usize,
    /// Number of declared vertices.
    pub vertex_count: usize,
    /// The axis-aligned bounding box.
    pub bounding_box: BoundingBox,
    /// Total surface area, in square metres.
    pub surface_area: f32,
    /// Signed volume, in cubic metres (negative when inside-out).
    pub volume: f32,
    /// The volume-weighted centre of mass, in metres.
    pub centre_of_mass: [f32; 3],
    /// True when every edge is shared by exactly two triangles.
    pub watertight: bool,
    /// Topological problems, one entry per kind that occurs.
    pub problems: Vec<Problem>,
    /// The Euler characteristic `V - E + F`.
    pub euler_characteristic: i64,
    /// Genus, when the mesh is a closed orientable surface.
    pub genus: Option<i64>,
    /// One entry per connected component.
    pub components: Vec<Component>,
    /// True when the signed volume is negative (faces point inward).
    pub inside_out: bool,
}

impl Report {
    /// Measure a mesh. Fails only on malformed input, never on size.
    ///
    /// # Errors
    /// Returns [`MeasureError`] when the mesh is empty, malformed or has non-finite coordinates.
    pub fn of(mesh: &Mesh) -> Result<Self, MeasureError> {
        let triangle_count = mesh.validate()?;
        let mut acc = Accumulator::new();
        for t in 0..triangle_count {
            acc.add(mesh, t);
        }
        let topology = acc.topology(mesh, triangle_count);
        let volume = acc.volume;
        let genus = if topology.watertight {
            let chi = 2 * topology.components.len() as i64 - topology.euler;
            (chi % 2 == 0).then_some(chi / 2)
        } else {
            None
        };
        Ok(Self {
            triangle_count,
            vertex_count: mesh.positions.len(),
            bounding_box: acc.bounding_box,
            surface_area: acc.area,
            volume,
            centre_of_mass: acc.centre_of_mass().to_array(),
            watertight: topology.watertight,
            problems: topology.problems,
            euler_characteristic: topology.euler,
            genus,
            components: topology.components,
            inside_out: volume < -VOLUME_EPSILON,
        })
    }

    /// A short plain-text report, at most 12 lines, for a language model.
    ///
    /// Units are metres. One fact per line, and the last line is either `ok` or the first problem
    /// with a suggested fix.
    #[must_use]
    pub fn summary(&self) -> String {
        let size = self.bounding_box.size();
        let centre = self.bounding_box.centre();
        let com = self.centre_of_mass;
        let mut facts = vec![
            format!(
                "triangles: {}, vertices: {}",
                self.triangle_count, self.vertex_count
            ),
            format!("size: {:.4} x {:.4} x {:.4} m", size[0], size[1], size[2]),
            format!(
                "centre: ({:.4}, {:.4}, {:.4}) m",
                centre[0], centre[1], centre[2]
            ),
            format!("area: {:.6} m2", self.surface_area),
            format!("volume: {:.6} m3", self.volume),
            format!(
                "centre of mass: ({:.4}, {:.4}, {:.4}) m",
                com[0], com[1], com[2]
            ),
            format!("watertight: {}", if self.watertight { "yes" } else { "no" }),
            format!("components: {}", self.components.len()),
            match self.genus {
                Some(g) => format!("genus: {g}"),
                None => "genus: unknown (open or non-manifold)".to_string(),
            },
            if self.problems.is_empty() {
                "topology problems: none".to_string()
            } else {
                let list = self
                    .problems
                    .iter()
                    .map(|p| format!("{} {}", p.count, p.kind.label()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("topology problems: {list}")
            },
            if self.inside_out {
                "orientation: inside-out (negative volume)".to_string()
            } else {
                "orientation: outward".to_string()
            },
        ];
        if facts.len() > 11 {
            facts.truncate(11);
        }
        facts.push(self.verdict());
        facts.join("\n")
    }

    fn verdict(&self) -> String {
        if let Some(p) = self.problems.first() {
            return match p.example {
                Some(e) => format!(
                    "problem: {} {} near ({:.3}, {:.3}, {:.3}): {}",
                    p.count,
                    p.kind.label(),
                    e[0],
                    e[1],
                    e[2],
                    p.kind.advice()
                ),
                None => format!(
                    "problem: {} {}: {}",
                    p.count,
                    p.kind.label(),
                    p.kind.advice()
                ),
            };
        }
        if self.inside_out {
            return "problem: the mesh is inside-out: flip its faces (poly.flip)".to_string();
        }
        "ok".to_string()
    }
}

/// How one undirected edge is used.
struct EdgeUse {
    uses: u32,
    midpoint: Vec3,
    length: f32,
}

struct Accumulator {
    bounding_box: BoundingBox,
    area: f32,
    volume: f32,
    mass: Vec3,
    edges: BTreeMap<(u32, u32), EdgeUse>,
}

impl Accumulator {
    fn new() -> Self {
        Self {
            bounding_box: BoundingBox::empty(),
            area: 0.0,
            volume: 0.0,
            mass: Vec3::ZERO,
            edges: BTreeMap::new(),
        }
    }

    fn add(&mut self, mesh: &Mesh, t: usize) {
        let (a, b, c) = mesh.corners(t);
        self.bounding_box.include(a);
        self.bounding_box.include(b);
        self.bounding_box.include(c);
        self.area += 0.5 * (b - a).cross(c - a).length();
        let tet = a.dot(b.cross(c)) / 6.0;
        self.volume += tet;
        self.mass += tet * (a + b + c) * 0.25;
        let [ia, ib, ic] = mesh.triangle(t);
        for (i, j) in [(ia, ib), (ib, ic), (ic, ia)] {
            let key = (i.min(j), i.max(j));
            let length = (mesh.position(i) - mesh.position(j)).length();
            self.edges
                .entry(key)
                .and_modify(|e| e.uses += 1)
                .or_insert(EdgeUse {
                    uses: 1,
                    midpoint: (mesh.position(i) + mesh.position(j)) * 0.5,
                    length,
                });
        }
    }

    fn centre_of_mass(&self) -> Vec3 {
        if self.volume.abs() > VOLUME_EPSILON {
            self.mass / self.volume
        } else {
            self.bounding_box.centre_vec()
        }
    }

    fn topology(&self, mesh: &Mesh, triangles: usize) -> Topology {
        let mut problems = self.edge_problems();
        let mut degenerate = 0;
        let mut degenerate_at = None;
        let mut used = vec![false; mesh.positions.len()];
        for &index in mesh.indices {
            used[index as usize] = true;
        }
        for t in 0..triangles {
            let (a, b, c) = mesh.corners(t);
            if 0.5 * (b - a).cross(c - a).length() < AREA_EPSILON {
                degenerate += 1;
                degenerate_at.get_or_insert((a + b + c) / 3.0);
            }
        }
        if degenerate > 0 {
            problems.push(Problem {
                kind: ProblemKind::DegenerateTriangle,
                count: degenerate,
                example: degenerate_at.map(|p| p.to_array()),
            });
        }
        if let Some(duplicates) = duplicate_vertices(mesh) {
            problems.push(duplicates);
        }
        let euler =
            used.iter().filter(|&&u| u).count() as i64 - self.edges.len() as i64 + triangles as i64;
        let components = components(mesh, triangles, &used);
        let watertight = problems
            .iter()
            .all(|p| !matches!(p.kind, ProblemKind::OpenEdge | ProblemKind::NonManifoldEdge));
        Topology {
            problems,
            euler,
            components,
            watertight,
        }
    }

    fn edge_problems(&self) -> Vec<Problem> {
        let mut open = (0usize, None);
        let mut non_manifold = (0usize, None);
        let mut zero_length = (0usize, None);
        for use_ in self.edges.values() {
            if use_.uses == 1 {
                open = (open.0 + 1, open.1.or(Some(use_.midpoint)));
            } else if use_.uses > 2 {
                non_manifold = (non_manifold.0 + 1, non_manifold.1.or(Some(use_.midpoint)));
            }
            if use_.length < LENGTH_EPSILON {
                zero_length = (zero_length.0 + 1, zero_length.1.or(Some(use_.midpoint)));
            }
        }
        let mut problems = Vec::new();
        push_problem(&mut problems, ProblemKind::OpenEdge, open);
        push_problem(&mut problems, ProblemKind::NonManifoldEdge, non_manifold);
        push_problem(&mut problems, ProblemKind::ZeroLengthEdge, zero_length);
        problems
    }
}

fn duplicate_vertices(mesh: &Mesh) -> Option<Problem> {
    let mut seen: HashSet<[u32; 3]> = HashSet::new();
    let mut count = 0;
    let mut example = None;
    for position in mesh.positions {
        let key = position.map(canonical_bits);
        if !seen.insert(key) {
            count += 1;
            example.get_or_insert(*position);
        }
    }
    (count > 0).then_some(Problem {
        kind: ProblemKind::DuplicateVertex,
        count,
        example,
    })
}

fn components(mesh: &Mesh, triangles: usize, used: &[bool]) -> Vec<Component> {
    let mut dsu = Dsu::new(mesh.positions.len());
    for t in 0..triangles {
        let [a, b, c] = mesh.triangle(t);
        dsu.union(a, b);
        dsu.union(b, c);
    }
    let mut index_of: HashMap<u32, usize> = HashMap::new();
    let mut components = Vec::new();
    for (i, &used) in used.iter().enumerate() {
        if !used {
            continue;
        }
        let root = dsu.find(i as u32);
        let idx = *index_of.entry(root).or_insert_with(|| {
            components.push(Component {
                bounding_box: BoundingBox::empty(),
                volume: 0.0,
                triangles: 0,
            });
            components.len() - 1
        });
        components[idx]
            .bounding_box
            .include(mesh.position(i as u32));
    }
    for t in 0..triangles {
        let (a, b, c) = mesh.corners(t);
        let idx = index_of[&dsu.find(mesh.triangle(t)[0])];
        components[idx].volume += a.dot(b.cross(c)) / 6.0;
        components[idx].triangles += 1;
    }
    components
}

struct Topology {
    problems: Vec<Problem>,
    euler: i64,
    components: Vec<Component>,
    watertight: bool,
}

fn push_problem(
    problems: &mut Vec<Problem>,
    kind: ProblemKind,
    (count, example): (usize, Option<Vec3>),
) {
    if count > 0 {
        problems.push(Problem {
            kind,
            count,
            example: example.map(|p| p.to_array()),
        });
    }
}

/// The bit pattern of a coordinate, mapping `-0.0` to `0.0` so the two compare equal.
fn canonical_bits(x: f32) -> u32 {
    if x.to_bits() == 0x8000_0000 {
        0.0f32.to_bits()
    } else {
        x.to_bits()
    }
}
