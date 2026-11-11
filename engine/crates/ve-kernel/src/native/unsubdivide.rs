//! Unsubdivide: the inverse of one level of quad subdivision.
//!
//! In a subdivided quad mesh every quad reads, going round, (original, edge point, face point, edge point). The
//! vertices are labelled so that this holds for every quad (the labelling is forced once one quad is chosen, and
//! the labelling with the fewest originals wins); the quads around each face point then merge into one coarse face
//! whose corners are the originals. Anything that is not a pure quad mesh of that shape declines.

use std::collections::BTreeMap;

use crate::native::edit::{NO_ORIGIN, Soup};
use crate::{EditedMesh, KernelError, MeshOp, MeshSelection, PolygonMesh};

const ORIGINAL: u8 = 1;
const EDGE: u8 = 2;
const FACE: u8 = 3;
/// The labels of one quad, going round: original, edge, face, edge.
const PATTERN: [u8; 4] = [ORIGINAL, EDGE, FACE, EDGE];

/// Runs the operator natively, or returns `None` when it does not apply.
#[must_use]
pub fn edit(mesh: &PolygonMesh<'_>, _selection: &MeshSelection<'_>, op: &MeshOp) -> Option<Result<EditedMesh, KernelError>> {
    let MeshOp::Unsubdivide { iterations } = *op else {
        return None;
    };
    if !(1..=4).contains(&iterations) {
        return Some(Err(KernelError::InvalidArgument("rejected by the kernel")));
    }
    let mut soup = match Soup::from_mesh(mesh) {
        Ok(soup) => soup,
        Err(error) => return Some(Err(error)),
    };
    let mut done = 0;
    for _ in 0..iterations {
        match one_level(&soup) {
            Some(coarser) => {
                soup = coarser;
                done += 1;
            }
            None => break,
        }
    }
    (done > 0).then(|| Ok(soup.finish()))
}

/// Which vertices lie on an open boundary.
fn boundary_vertices(s: &Soup) -> Vec<bool> {
    let mut flags = vec![false; s.positions.len()];
    for (&(a, b), users) in &s.edge_faces() {
        if users.len() == 1 {
            flags[a as usize] = true;
            flags[b as usize] = true;
        }
    }
    flags
}

/// The labelling (original / edge / face per vertex) forced by putting the face point at corner `seed` of quad 0,
/// or `None` if it contradicts itself.
fn label(s: &Soup, on_boundary: &[bool], seed: usize) -> Option<Vec<u8>> {
    let mut labels = vec![0_u8; s.positions.len()];
    let candidates = |labels: &[u8], quad: &[u32]| -> Vec<usize> {
        (0..4)
            .filter(|&r| {
                (0..4).all(|i| {
                    let want = PATTERN[(i + r) % 4];
                    let vertex = quad[i] as usize;
                    (labels[vertex] == 0 || labels[vertex] == want) && !(want == FACE && on_boundary[vertex])
                })
            })
            .collect()
    };
    // Put the face point at corner `seed` of quad 0: PATTERN[(i + r) % 4] is FACE where (i + r) % 4 == 2.
    let r = (2 + 4 - seed) % 4;
    for i in 0..4 {
        labels[s.faces[0][i] as usize] = PATTERN[(i + r) % 4];
    }
    if candidates(&labels, &s.faces[0]).is_empty() {
        return None;
    }
    loop {
        let mut changed = false;
        for quad in &s.faces {
            let options = candidates(&labels, quad);
            match options.len() {
                0 => return None,
                1 => {
                    let r = options[0];
                    for i in 0..4 {
                        let want = PATTERN[(i + r) % 4];
                        if labels[quad[i] as usize] == 0 {
                            labels[quad[i] as usize] = want;
                            changed = true;
                        }
                    }
                }
                _ => {}
            }
        }
        if !changed {
            break;
        }
    }
    // Quads the propagation could not pin down (a piece joined only through a vertex): take the first option.
    for quad in &s.faces {
        let options = candidates(&labels, quad);
        let r = *options.first()?;
        for i in 0..4 {
            let vertex = quad[i] as usize;
            if labels[vertex] == 0 {
                labels[vertex] = PATTERN[(i + r) % 4];
            }
        }
    }
    // Every quad must now read exactly as the pattern.
    s.faces
        .iter()
        .all(|quad| {
            (0..4).any(|r| (0..4).all(|i| labels[quad[i] as usize] == PATTERN[(i + r) % 4]))
        })
        .then_some(labels)
}

fn one_level(s: &Soup) -> Option<Soup> {
    if s.faces.is_empty() || s.faces.iter().any(|f| f.len() != 4) {
        return None;
    }
    let on_boundary = boundary_vertices(s);
    // Corner counts per vertex: a face point with four quads around it gives a coarse quad.
    let mut valence = vec![0_usize; s.positions.len()];
    for face in &s.faces {
        for &v in face {
            valence[v as usize] += 1;
        }
    }
    // Several labellings can be valid (a cube's eight corners could be taken as face points, giving an octahedron):
    // prefer the one with the most coarse quads, then the fewest originals.
    let best = (0..4)
        .filter_map(|seed| label(s, &on_boundary, seed))
        .max_by_key(|labels| {
            let quads = (0..labels.len()).filter(|&v| labels[v] == FACE && valence[v] == 4).count();
            let originals = labels.iter().filter(|&&l| l == ORIGINAL).count();
            (quads, std::cmp::Reverse(originals))
        })?;
    // Each quad belongs to the face point it has; walk the quads around every face point.
    let mut directed: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    for (f, face) in s.faces.iter().enumerate() {
        for i in 0..4 {
            directed.insert((face[i], face[(i + 1) % 4]), f);
        }
    }
    let face_corner = |f: usize| (0..4).find(|&i| best[s.faces[f][i] as usize] == FACE);
    let mut coarse: Vec<Vec<u32>> = Vec::new();
    let mut seen = vec![false; s.faces.len()];
    for start in 0..s.faces.len() {
        if seen[start] {
            continue;
        }
        let mut ring = Vec::new();
        let mut f = start;
        loop {
            seen[f] = true;
            let p = face_corner(f)?;
            let quad = &s.faces[f];
            ring.push(quad[(p + 2) % 4]);
            // The next quad around the face point lies across the edge from the face point to the following edge point.
            let (face_point, edge_point) = (quad[p], quad[(p + 1) % 4]);
            f = *directed.get(&(edge_point, face_point))?;
            if f == start {
                break;
            }
            if seen[f] || ring.len() > 16 {
                return None;
            }
        }
        // Walking this way meets the coarse corners against the winding.
        ring.reverse();
        if ring.len() < 3 {
            return None;
        }
        coarse.push(ring);
    }
    let mut out = Soup {
        positions: s.positions.clone(),
        vertex_origin: s.vertex_origin.clone(),
        ..Soup::default()
    };
    for ring in coarse {
        out.add_face(ring, NO_ORIGIN, 0);
    }
    Some(out)
}
