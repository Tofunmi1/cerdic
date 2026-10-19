//! Automatic UV seams: where to cut a polygon mesh so every island can be flattened.
//!
//! Edges whose faces meet at more than an angle are cut (the corners of a box, the rim of a plate). A piece
//! that is still a closed surface (a sphere) or a tube with several boundary loops is cut along shortest
//! edge paths until it can lie flat. Pieces with handles (a torus) are left alone; the unwrap reports them.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{GeometryError, PolyMesh, Topology};

type Edge = (u32, u32);

/// Newell normal of every face, normalised (zero for a degenerate face).
fn face_normals(mesh: &PolyMesh) -> Vec<[f64; 3]> {
    let positions = mesh.positions();
    let mut at = 0_usize;
    mesh.face_sizes()
        .iter()
        .map(|&size| {
            let corners = &mesh.corners()[at..at + size as usize];
            at += size as usize;
            let mut n = [0.0_f64; 3];
            for (k, &a) in corners.iter().enumerate() {
                let p = positions[a as usize].map(f64::from);
                let q = positions[corners[(k + 1) % corners.len()] as usize].map(f64::from);
                n[0] += (p[1] - q[1]) * (p[2] + q[2]);
                n[1] += (p[2] - q[2]) * (p[0] + q[0]);
                n[2] += (p[0] - q[0]) * (p[1] + q[1]);
            }
            let length = n.iter().map(|v| v * v).sum::<f64>().sqrt();
            if length > 0.0 {
                n.map(|v| v / length)
            } else {
                n
            }
        })
        .collect()
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

/// One flat-able piece: its faces' edges and how they connect.
struct Island {
    faces: usize,
    /// Edges used by the island, with how many of its faces use each.
    edges: BTreeMap<Edge, usize>,
}

impl Island {
    fn vertices(&self) -> BTreeSet<u32> {
        self.edges.keys().flat_map(|&(a, b)| [a, b]).collect()
    }

    fn euler(&self) -> i64 {
        let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
        count(self.vertices().len()) - count(self.edges.len()) + count(self.faces)
    }

    /// Groups of vertices joined by edges only one face of the island uses (its outline).
    fn boundary_loops(&self) -> Vec<BTreeSet<u32>> {
        let verts: Vec<u32> = self.vertices().into_iter().collect();
        let index: BTreeMap<u32, usize> = verts.iter().enumerate().map(|(i, &v)| (v, i)).collect();
        let mut parent: Vec<usize> = (0..verts.len()).collect();
        let mut on_boundary = BTreeSet::new();
        for (&(a, b), &uses) in &self.edges {
            if uses == 1 {
                on_boundary.insert(a);
                on_boundary.insert(b);
                let (ra, rb) = (find(&mut parent, index[&a]), find(&mut parent, index[&b]));
                parent[ra] = rb;
            }
        }
        let mut loops: BTreeMap<usize, BTreeSet<u32>> = BTreeMap::new();
        for &v in &on_boundary {
            loops
                .entry(find(&mut parent, index[&v]))
                .or_default()
                .insert(v);
        }
        loops.into_values().collect()
    }

    fn neighbours(&self) -> BTreeMap<u32, Vec<u32>> {
        let mut map: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for &(a, b) in self.edges.keys() {
            map.entry(a).or_default().push(b);
            map.entry(b).or_default().push(a);
        }
        map
    }
}

/// Shortest edge path (fewest edges, lowest vertex ids first) from any of `from` to the nearest of `to`.
fn shortest_path(
    neighbours: &BTreeMap<u32, Vec<u32>>,
    from: &BTreeSet<u32>,
    to: &BTreeSet<u32>,
) -> Vec<u32> {
    let mut parent: BTreeMap<u32, u32> = BTreeMap::new();
    let mut seen: BTreeSet<u32> = from.clone();
    let mut queue: VecDeque<u32> = from.iter().copied().collect();
    while let Some(v) = queue.pop_front() {
        if to.contains(&v) && !from.contains(&v) {
            let mut path = vec![v];
            let mut at = v;
            while let Some(&p) = parent.get(&at) {
                path.push(p);
                at = p;
            }
            return path;
        }
        for &n in neighbours.get(&v).map_or(&[][..], Vec::as_slice) {
            if seen.insert(n) {
                parent.insert(n, v);
                queue.push_back(n);
            }
        }
    }
    Vec::new()
}

/// The vertex farthest (in edges) from `start`, ties to the lowest id.
fn farthest(neighbours: &BTreeMap<u32, Vec<u32>>, start: u32) -> u32 {
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([(start, 0_usize)]);
    let (mut best, mut best_distance) = (start, 0);
    while let Some((v, d)) = queue.pop_front() {
        if d > best_distance {
            (best, best_distance) = (v, d);
        }
        for &n in neighbours.get(&v).map_or(&[][..], Vec::as_slice) {
            if seen.insert(n) {
                queue.push_back((n, d + 1));
            }
        }
    }
    best
}

fn push_path(seams: &mut BTreeSet<Edge>, path: &[u32]) {
    for pair in path.windows(2) {
        seams.insert((pair[0].min(pair[1]), pair[0].max(pair[1])));
    }
}

/// Cuts one island open if it is a closed surface or a tube of genus zero.
fn open_island(mesh: &PolyMesh, island: &Island, seams: &mut BTreeSet<Edge>) {
    let loops = island.boundary_loops();
    let boundaries = i64::try_from(loops.len()).unwrap_or(i64::MAX);
    if island.euler() != 2 - boundaries {
        return; // a handle: not something a path cut fixes
    }
    let neighbours = island.neighbours();
    if loops.is_empty() {
        // Closed: a path from the lowest vertex to the one farthest from it turns a sphere into a disc.
        let positions = mesh.positions();
        let lowest = island.vertices().into_iter().min_by(|&a, &b| {
            let (p, q) = (positions[a as usize], positions[b as usize]);
            (p[1], p[0], p[2], a)
                .partial_cmp(&(q[1], q[0], q[2], b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if let Some(start) = lowest {
            let end = farthest(&neighbours, start);
            push_path(
                seams,
                &shortest_path(
                    &neighbours,
                    &BTreeSet::from([start]),
                    &BTreeSet::from([end]),
                ),
            );
        }
        return;
    }
    // Several outlines: join each to the first by a short path so the piece becomes one outline.
    let mut joined = loops[0].clone();
    for other in &loops[1..] {
        let path = shortest_path(&neighbours, &joined, other);
        push_path(seams, &path);
        joined.extend(other.iter().copied());
        joined.extend(path);
    }
}

/// Splits the faces into charts that each stay within a cone of `angle` radians around their first face's
/// normal (the chart id is that face's index). Charts grow across edges shared by exactly two faces, in face
/// order, so the result is deterministic. A smooth closed shape becomes several nearly flat patches, which
/// flatten with little stretch, instead of one piece that must be cut open.
fn grow_charts(topology: &Topology, normals: &[[f64; 3]], faces: usize, angle: f64) -> Vec<usize> {
    let mut neighbours: Vec<Vec<usize>> = vec![Vec::new(); faces];
    for info in topology.edges().values() {
        if let [one, two] = info.uses() {
            neighbours[one.face].push(two.face);
            neighbours[two.face].push(one.face);
        }
    }
    let limit = angle.cos();
    let mut chart = vec![usize::MAX; faces];
    for seed in 0..faces {
        if chart[seed] != usize::MAX {
            continue;
        }
        chart[seed] = seed;
        let mut queue = VecDeque::from([seed]);
        while let Some(face) = queue.pop_front() {
            for &other in &neighbours[face] {
                let (a, b) = (normals[seed], normals[other]);
                if chart[other] == usize::MAX && a[0] * b[0] + a[1] * b[1] + a[2] * b[2] >= limit {
                    chart[other] = seed;
                    queue.push_back(other);
                }
            }
        }
    }
    chart
}

/// Seams for `mesh`: chart borders (see [`grow_charts`]; `angle` in radians is the chart's cone), plus the cuts
/// that open pieces that are still closed or tube-shaped.
///
/// # Errors
/// Cancellation, or a mesh the topology cannot be built for.
pub fn auto_seams(
    mesh: &PolyMesh,
    angle: f64,
    context: &ExecutionContext,
) -> Result<Vec<(u32, u32)>, GeometryError> {
    let topology = Topology::build(mesh, context)?;
    let normals = face_normals(mesh);
    let chart = grow_charts(&topology, &normals, mesh.face_count(), angle);
    let mut seams: BTreeSet<Edge> = BTreeSet::new();
    for (&edge, info) in topology.edges() {
        match info.uses() {
            [one, two] => {
                if chart[one.face] != chart[two.face] {
                    seams.insert(edge);
                }
            }
            [_] => {}
            _ => {
                seams.insert(edge); // three or more faces share it: always a cut
            }
        }
    }
    check_cancelled(context)?;
    let mut islands: BTreeMap<usize, Island> = BTreeMap::new();
    for &root in &chart {
        islands
            .entry(root)
            .or_insert(Island {
                faces: 0,
                edges: BTreeMap::new(),
            })
            .faces += 1;
    }
    for (&edge, info) in topology.edges() {
        for use_ in info.uses() {
            if let Some(island) = islands.get_mut(&chart[use_.face]) {
                *island.edges.entry(edge).or_insert(0) += 1;
            }
        }
    }
    for island in islands.values() {
        // An edge shared by two faces of different islands is a seam: its island sees one use, as an outline.
        open_island(mesh, island, &mut seams);
        check_cancelled(context)?;
    }
    Ok(seams.into_iter().collect())
}
