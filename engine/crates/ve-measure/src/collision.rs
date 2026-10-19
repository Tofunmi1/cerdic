//! A simple deterministic convex decomposition into bounded convex parts.

use glam::Vec3;

use crate::bvh::Bvh;
use crate::error::MeasureError;
use crate::hull::{Hull, convex_hull};
use crate::mesh::Mesh;

/// Cells per side of the occupancy grid used by [`convex_decomposition`].
const VOXELS: usize = 12;
/// The most vertices a decomposition part may have (matches physics engine limits).
const MAX_PART_VERTICES: usize = 64;
/// How many ray crossings the inside test walks before giving up.
const MAX_CROSSINGS: usize = 4096;

/// One part of a [`ConvexDecomposition`], as a hull.
#[derive(Clone, Debug, PartialEq)]
pub struct ConvexPart {
    /// The part's convex hull.
    pub hull: Hull,
}

/// A mesh split into convex parts, each a hull of at most 64 vertices.
#[derive(Clone, Debug, PartialEq)]
pub struct ConvexDecomposition {
    /// The parts, in split order.
    pub parts: Vec<ConvexPart>,
    /// The largest vertex count among the parts (at most 64).
    pub max_vertices: usize,
}

/// Split a mesh into at most `max_parts` convex hulls.
///
/// The method is deliberately simple and bounded: the mesh's bounding box is voxelised at
/// [`VOXELS`]³ by parity of ray crossings, the occupied cells are repeatedly halved along their
/// longest axis until there are `max_parts` clusters, and each cluster's cell corners are hulled
/// (falling back to the cluster's bounding box when a hull would exceed [`MAX_PART_VERTICES`]
/// vertices). The error is therefore the voxel resolution: features thinner than one cell are lost,
/// touching parts may not be disjoint, and the union of the parts is a box-ish approximation of the
/// surface rather than an exact decomposition.
///
/// # Errors
/// Returns [`MeasureError`] when the mesh is malformed, `max_parts` is zero, or voxelisation finds
/// no interior (for example an open or flat surface).
pub fn convex_decomposition(
    mesh: &Mesh,
    max_parts: usize,
) -> Result<ConvexDecomposition, MeasureError> {
    mesh.validate()?;
    if max_parts == 0 {
        return Err(MeasureError::Degenerate("max_parts must be at least one"));
    }
    let bounds = mesh.bounds();
    let min = Vec3::from_array(bounds.min);
    let size = Vec3::from_array(bounds.size());
    let cell = Vec3::new(
        (size.x / VOXELS as f32).max(1e-6),
        (size.y / VOXELS as f32).max(1e-6),
        (size.z / VOXELS as f32).max(1e-6),
    );
    let bvh = Bvh::build(mesh)?;
    let mut occupied = Vec::new();
    for z in 0..VOXELS {
        for y in 0..VOXELS {
            for x in 0..VOXELS {
                let centre =
                    min + (Vec3::new(x as f32, y as f32, z as f32) + Vec3::splat(0.5)) * cell;
                if inside(mesh, &bvh, centre) {
                    occupied.push([x as u32, y as u32, z as u32]);
                }
            }
        }
    }
    if occupied.is_empty() {
        return Err(MeasureError::Degenerate("the mesh has no interior"));
    }
    let clusters = split_clusters(vec![occupied], max_parts);
    let mut parts = Vec::new();
    for cluster in clusters {
        parts.push(ConvexPart {
            hull: hull_of(&cluster, min, cell),
        });
    }
    let max_vertices = parts
        .iter()
        .map(|p| p.hull.vertex_count())
        .max()
        .unwrap_or(0);
    Ok(ConvexDecomposition {
        parts,
        max_vertices,
    })
}

fn hull_of(cluster: &[[u32; 3]], min: Vec3, cell: Vec3) -> Hull {
    let points = corner_points(cluster, min, cell);
    match convex_hull(&points) {
        Ok(hull) if hull.vertex_count() <= MAX_PART_VERTICES => hull,
        _ => {
            let (lo, hi) = corner_bounds(cluster, min, cell);
            Hull::box_hull(lo, hi)
        }
    }
}

fn corner_points(cluster: &[[u32; 3]], min: Vec3, cell: Vec3) -> Vec<[f32; 3]> {
    let mut points = Vec::with_capacity(cluster.len() * 8);
    for c in cluster {
        for dx in 0..2 {
            for dy in 0..2 {
                for dz in 0..2 {
                    let corner = min
                        + Vec3::new((c[0] + dx) as f32, (c[1] + dy) as f32, (c[2] + dz) as f32)
                            * cell;
                    points.push(corner.to_array());
                }
            }
        }
    }
    points
}

fn corner_bounds(cluster: &[[u32; 3]], min: Vec3, cell: Vec3) -> (Vec3, Vec3) {
    let mut lo = Vec3::splat(f32::INFINITY);
    let mut hi = Vec3::splat(f32::NEG_INFINITY);
    for c in cluster {
        for dx in 0..2 {
            for dy in 0..2 {
                for dz in 0..2 {
                    let corner = min
                        + Vec3::new((c[0] + dx) as f32, (c[1] + dy) as f32, (c[2] + dz) as f32)
                            * cell;
                    lo = lo.min(corner);
                    hi = hi.max(corner);
                }
            }
        }
    }
    (lo, hi)
}

/// True when the point is inside the mesh, by parity of ray crossings along +X.
fn inside(mesh: &Mesh, bvh: &Bvh, point: Vec3) -> bool {
    let dir = Vec3::X.to_array();
    let mut origin = point;
    let mut crossings = 0;
    for _ in 0..MAX_CROSSINGS {
        let Some(hit) = bvh.raycast(mesh, origin.to_array(), dir) else {
            break;
        };
        crossings += 1;
        origin = Vec3::from_array(hit.point) + Vec3::X * 1e-5;
    }
    crossings % 2 == 1
}

fn split_clusters(mut clusters: Vec<Vec<[u32; 3]>>, max_parts: usize) -> Vec<Vec<[u32; 3]>> {
    while clusters.len() < max_parts {
        let Some(index) = largest_splittable(&clusters) else {
            break;
        };
        let mut cluster = clusters.swap_remove(index);
        let axis = longest_axis(&cluster);
        cluster.sort_unstable_by(|a, b| key(a).cmp(&key(b)).then_with(|| a[axis].cmp(&b[axis])));
        let right = cluster.split_off(cluster.len() / 2);
        clusters.push(cluster);
        clusters.push(right);
    }
    clusters
}

fn largest_splittable(clusters: &[Vec<[u32; 3]>]) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for (i, cluster) in clusters.iter().enumerate() {
        if cluster.len() < 2 {
            continue;
        }
        let (lo, hi) = cell_bounds(cluster);
        let volume = ((hi[0] - lo[0]) as f32) * ((hi[1] - lo[1]) as f32) * ((hi[2] - lo[2]) as f32);
        if best.is_none_or(|(_, v)| volume > v) {
            best = Some((i, volume));
        }
    }
    best.map(|(i, _)| i)
}

fn key(cell: &[u32; 3]) -> (u32, u32, u32) {
    (cell[0], cell[1], cell[2])
}

fn cell_bounds(cluster: &[[u32; 3]]) -> ([u32; 3], [u32; 3]) {
    let mut lo = [u32::MAX; 3];
    let mut hi = [0u32; 3];
    for c in cluster {
        for k in 0..3 {
            lo[k] = lo[k].min(c[k]);
            hi[k] = hi[k].max(c[k]);
        }
    }
    (lo, hi)
}

fn longest_axis(cluster: &[[u32; 3]]) -> usize {
    let (lo, hi) = cell_bounds(cluster);
    let size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    if size[0] >= size[1] && size[0] >= size[2] {
        0
    } else if size[1] >= size[2] {
        1
    } else {
        2
    }
}
