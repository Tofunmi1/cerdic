//! What the host binds to a program's `input`s: a mesh (rest points and which points are joined) and the poses of bones.
use std::sync::Arc;

/// A mesh a program can read: rest points and, for every point, the points joined to it by an edge with the rest length of that edge.
#[derive(Debug)]
pub struct MeshData {
    points: Vec<[f32; 3]>,
    /// Compressed rows: the neighbours of point `v` are `joined[starts[v]..starts[v + 1]]`, ascending by point number.
    starts: Vec<usize>,
    joined: Vec<(u32, f32)>,
}

impl MeshData {
    /// Builds the mesh from rest `points` and edges (pairs of point numbers; repeats and edges to nowhere are dropped).
    #[must_use]
    pub fn new(points: Vec<[f32; 3]>, edges: &[(u32, u32)]) -> Arc<Self> {
        let n = points.len();
        let mut rows: Vec<Vec<u32>> = vec![Vec::new(); n];
        for &(a, b) in edges {
            let (a, b) = (a as usize, b as usize);
            if a < n && b < n && a != b {
                rows[a].push(b as u32);
                rows[b].push(a as u32);
            }
        }
        let mut starts = Vec::with_capacity(n + 1);
        let mut joined = Vec::new();
        for (v, row) in rows.iter_mut().enumerate() {
            row.sort_unstable();
            row.dedup();
            starts.push(joined.len());
            for &j in row.iter() {
                let (p, q) = (points[v], points[j as usize]);
                let len = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
                joined.push((j, len));
            }
        }
        starts.push(joined.len());
        Arc::new(Self { points, starts, joined })
    }

    /// How many points.
    #[must_use]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether there are no points.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// The mesh as the words a compute shader reads: `[n, starts, joined, lengths, rest]` (the four offsets into the words), then the
    /// `n + 1` row starts, the neighbour numbers, the edge rest lengths (f32 bits) and the rest points (three f32 bits each).
    #[must_use]
    pub fn gpu_words(&self) -> Vec<u32> {
        let n = self.points.len();
        let edges = self.joined.len();
        let starts_at = 5;
        let joined_at = starts_at + n + 1;
        let lengths_at = joined_at + edges;
        let rest_at = lengths_at + edges;
        let word = |x: usize| u32::try_from(x).unwrap_or(u32::MAX);
        let mut words = vec![word(n), word(starts_at), word(joined_at), word(lengths_at), word(rest_at)];
        words.extend(self.starts.iter().map(|&s| word(s)));
        words.extend(self.joined.iter().map(|&(j, _)| j));
        words.extend(self.joined.iter().map(|&(_, len)| len.to_bits()));
        words.extend(self.points.iter().flatten().map(|x| x.to_bits()));
        words
    }

    pub(super) fn point(&self, v: usize) -> Option<[f32; 3]> {
        self.points.get(v).copied()
    }

    pub(super) fn neighbours(&self, v: usize) -> &[(u32, f32)] {
        match (self.starts.get(v), self.starts.get(v + 1)) {
            (Some(&a), Some(&b)) => &self.joined[a..b],
            _ => &[],
        }
    }
}

/// The identity pose.
pub const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// The bound inputs of one simulation, by input number.
#[derive(Debug, Default)]
pub(super) struct Inputs {
    pub(super) bones: Vec<[f32; 16]>,
    pub(super) meshes: Vec<Option<Arc<MeshData>>>,
}

impl Inputs {
    /// Carries a point (`w` = 1) or direction (`w` = 0) by the pose of a bone (a column-major matrix).
    pub(super) fn carry(&self, input: usize, v: [f32; 3], w: f32) -> [f32; 3] {
        let m = self.bones.get(input).copied().unwrap_or(IDENTITY);
        [
            m[0] * v[0] + m[4] * v[1] + m[8] * v[2] + m[12] * w,
            m[1] * v[0] + m[5] * v[1] + m[9] * v[2] + m[13] * w,
            m[2] * v[0] + m[6] * v[1] + m[10] * v[2] + m[14] * w,
        ]
    }
}
