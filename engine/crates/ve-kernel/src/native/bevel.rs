//! Native edge bevel.
//!
//! Every beveled edge becomes a strip of `segments` quads between two profile curves (one at each end), cut along
//! offset lines in the two faces beside the edge. At each end vertex the faces around it are split into sectors by
//! the beveled edges; a sector contributes one boundary vertex (a miter point when it is a single face, a point on
//! the shared plain edge otherwise). Vertices where three or more beveled edges meet are filled with rings of quads
//! (a spherical cap for round profiles); two beveled edges are joined by a zipper.
//!
//! Configurations this module does not handle return `None`, so the legacy build still serves them: open fans
//! (boundary vertices), inconsistent winding, a single beveled edge at a vertex of valence four or more, and
//! sectors with more than two plain edges in them.

#![expect(
    clippy::many_single_char_names,
    clippy::manual_midpoint,
    reason = "vector maths keeps its usual short names, and midpoint on f64 would only obscure the formulas"
)]

use std::collections::{BTreeMap, BTreeSet};

use super::edit::Soup;
use crate::{BevelOffset, BevelOptions, BeveledMesh, KernelError, PolygonMesh};

type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn mul(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: V3) -> f64 {
    dot(a, a).sqrt()
}
fn unit(a: V3) -> Option<V3> {
    let n = norm(a);
    (n > 1e-12 && n.is_finite()).then(|| mul(a, 1.0 / n))
}
fn p3(p: [f32; 3]) -> V3 {
    p.map(f64::from)
}
#[expect(
    clippy::cast_possible_truncation,
    reason = "the kernel stores f32 positions"
)]
fn to_f32(v: V3) -> [f32; 3] {
    [v[0] as f32, v[1] as f32, v[2] as f32]
}
fn key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

/// Bevels the edges natively, or returns `None` when this module does not implement the request yet.
#[must_use]
pub fn bevel_edges(
    mesh: &PolygonMesh<'_>,
    edges: &[[u32; 2]],
    options: &BevelOptions,
) -> Option<Result<BeveledMesh, KernelError>> {
    match run(mesh, edges, options) {
        Ok(Some(done)) => Some(Ok(done)),
        Ok(None) => None,
        Err(error) => Some(Err(error)),
    }
}

/// What a beveled edge knows about the two faces beside it.
struct EdgeInfo {
    /// The face that walks the edge from its low to its high vertex, and the one that walks it back.
    faces: [usize; 2],
    /// Offset distance in each of the two faces (unscaled).
    offset: [f64; 2],
    /// In-face unit direction, perpendicular to the edge, pointing into each face.
    inward: [V3; 2],
}

struct Model {
    soup: Soup,
    normals: Vec<V3>,
    edges: BTreeMap<(u32, u32), EdgeInfo>,
    /// Faces around each affected vertex.
    segments: u32,
    exponent: f64,
    round: bool,
}

/// The faces around one vertex, in walking order, and the vertex each one walks to next.
struct Fan {
    faces: Vec<usize>,
    next: Vec<u32>,
    /// A vertex on the mesh boundary: the faces do not close into a ring, and `start_prev` is the vertex before this
    /// vertex in the first face.
    open: bool,
    start_prev: u32,
}

impl Fan {
    fn len(&self) -> usize {
        self.faces.len()
    }
    fn at(&self, i: usize) -> usize {
        i % self.len()
    }
}

/// One stretch of faces between two beveled edges, with its boundary vertices.
struct Sector {
    first: usize,
    count: usize,
    positions: Vec<V3>,
    /// Several coplanar faces that act as one: a single miter point replaces the vertex in all of them.
    flat: bool,
}

struct VertexGeometry {
    fan: Fan,
    beveled: Vec<usize>,
    sectors: Vec<Sector>,
}

struct Layout {
    vertices: BTreeMap<u32, VertexGeometry>,
    plain_use: BTreeMap<(u32, u32), f64>,
    bevel_use: BTreeMap<(u32, u32), f64>,
}

#[expect(
    clippy::too_many_lines,
    reason = "one pass validates, builds the model and emits the mesh"
)]
fn run(
    mesh: &PolygonMesh<'_>,
    edges: &[[u32; 2]],
    options: &BevelOptions,
) -> Result<Option<BeveledMesh>, KernelError> {
    if !(options.width.is_finite() && options.width > 0.0) {
        return Err(KernelError::InvalidArgument(
            "the bevel width must be positive",
        ));
    }
    if options.segments == 0 || options.segments > 64 {
        return Err(KernelError::InvalidArgument("segments must be 1 to 64"));
    }
    if !(options.profile.is_finite() && (0.1..=1.0).contains(&options.profile)) {
        return Err(KernelError::InvalidArgument("the profile must be 0.1 to 1"));
    }
    if edges.is_empty() {
        return Err(KernelError::InvalidArgument(
            "at least one edge is required",
        ));
    }
    for pair in edges {
        if pair.iter().any(|&v| v as usize >= mesh.positions.len()) {
            return Err(KernelError::OutOfBounds);
        }
        if pair[0] == pair[1] {
            return Err(KernelError::InvalidArgument("rejected by the kernel"));
        }
    }
    let soup = Soup::from_mesh(mesh)?;
    for face in &soup.faces {
        let distinct: BTreeSet<u32> = face.iter().copied().collect();
        if distinct.len() != face.len() {
            return Err(KernelError::InvalidArgument("a face repeats a vertex"));
        }
    }
    if soup.faces.len() > 1_000_000
        || edges.len().saturating_mul(options.segments as usize) > 2_000_000
    {
        return Err(KernelError::TooLarge);
    }
    let edge_faces = soup.edge_faces();
    let mut selected: BTreeSet<(u32, u32)> = BTreeSet::new();
    for pair in edges {
        let k = key(pair[0], pair[1]);
        match edge_faces.get(&k).map(Vec::len) {
            None => return Err(KernelError::InvalidArgument("rejected by the kernel")),
            Some(2) => {
                selected.insert(k);
            }
            Some(_) => return Err(KernelError::NotManifold),
        }
    }

    // Directed edges: each must belong to one face only, or the winding is not consistent.
    let mut directed: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    for (f, face) in soup.faces.iter().enumerate() {
        for i in 0..face.len() {
            if directed
                .insert((face[i], face[(i + 1) % face.len()]), f)
                .is_some()
            {
                return Ok(None);
            }
        }
    }
    let mut normals = Vec::with_capacity(soup.faces.len());
    for f in 0..soup.faces.len() {
        match unit(soup.face_normal(f)) {
            Some(n) => normals.push(n),
            None => return Ok(None),
        }
    }

    let width = f64::from(options.width);
    let mut infos = BTreeMap::new();
    for &(lo, hi) in &selected {
        let (Some(&fwd), Some(&back)) = (directed.get(&(lo, hi)), directed.get(&(hi, lo))) else {
            return Ok(None);
        };
        let (p_lo, p_hi) = (
            p3(soup.positions[lo as usize]),
            p3(soup.positions[hi as usize]),
        );
        let Some(along) = unit(sub(p_hi, p_lo)) else {
            return Ok(None);
        };
        let (Some(d0), Some(d1)) = (
            unit(cross(normals[fwd], along)),
            unit(cross(normals[back], mul(along, -1.0))),
        ) else {
            return Ok(None);
        };
        let c = dot(d0, d1).clamp(-1.0, 1.0);
        let (sin_half, cos_half) = (((1.0 - c) / 2.0).sqrt(), ((1.0 + c) / 2.0).sqrt());
        let offset = match options.offset_type {
            BevelOffset::Offset => [width, width],
            BevelOffset::Width => {
                if sin_half < 1e-4 {
                    return Ok(None);
                }
                [width / (2.0 * sin_half); 2]
            }
            BevelOffset::Depth => {
                if cos_half < 1e-4 {
                    return Ok(None);
                }
                [width / cos_half; 2]
            }
            BevelOffset::Percent => [
                width / 100.0 * adjacent_length(&soup, fwd, lo, hi),
                width / 100.0 * adjacent_length(&soup, back, hi, lo),
            ],
        };
        if offset.iter().any(|o| !(o.is_finite() && *o > 0.0)) {
            return Ok(None);
        }
        infos.insert(
            (lo, hi),
            EdgeInfo {
                faces: [fwd, back],
                offset,
                inward: [d0, d1],
            },
        );
    }

    let q = f64::from(options.profile).ln() / 0.5_f64.ln();
    let model = Model {
        soup,
        normals,
        edges: infos,
        segments: options.segments,
        exponent: q,
        round: (f64::from(options.profile) - 0.5).abs() < 0.05,
    };

    // Faces around every affected vertex.
    let affected: BTreeSet<u32> = selected.iter().flat_map(|&(a, b)| [a, b]).collect();
    let mut around: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (f, face) in model.soup.faces.iter().enumerate() {
        for &v in face {
            if affected.contains(&v) {
                around.entry(v).or_default().push(f);
            }
        }
    }
    let mut fans = BTreeMap::new();
    for &v in &affected {
        let Some(fan) = build_fan(
            &model.soup,
            &directed,
            v,
            around.get(&v).map_or(0, Vec::len),
        ) else {
            return Ok(None);
        };
        fans.insert(v, fan);
    }

    let Some(mut layout) = model.layout(&fans, &selected, 1.0) else {
        return Ok(None);
    };
    let scale = model.clamp_scale(&layout);
    if scale < 1.0 {
        match model.layout(&fans, &selected, scale) {
            Some(again) => layout = again,
            None => return Ok(None),
        }
    }
    model.build(layout, &selected)
}

/// Average length of the two edges of `face` that touch the directed edge `a -> b` at its ends.
fn adjacent_length(soup: &Soup, face: usize, a: u32, b: u32) -> f64 {
    let f = &soup.faces[face];
    let n = f.len();
    let Some(i) = f.iter().position(|&v| v == a) else {
        return 1.0;
    };
    let prev = f[(i + n - 1) % n];
    let next = f[(i + 2) % n];
    let p = |v: u32| p3(soup.positions[v as usize]);
    let _ = b;
    (norm(sub(p(a), p(prev))) + norm(sub(p(f[(i + 1) % n]), p(next)))) / 2.0
}

/// Walks the faces around `v`; `None` when the fan is open or not consistently wound.
fn build_fan(
    soup: &Soup,
    directed: &BTreeMap<(u32, u32), usize>,
    v: u32,
    expected: usize,
) -> Option<Fan> {
    let start = soup.faces.iter().position(|f| f.contains(&v))?;
    let (mut faces, mut next) = (Vec::new(), Vec::new());
    let mut f = start;
    let mut closed = true;
    loop {
        let face = &soup.faces[f];
        let i = face.iter().position(|&x| x == v)?;
        let n = face[(i + 1) % face.len()];
        faces.push(f);
        next.push(n);
        let Some(&following) = directed.get(&(n, v)) else {
            closed = false;
            break;
        };
        f = following;
        if f == start {
            break;
        }
        if faces.len() > expected {
            return None;
        }
    }
    if closed {
        return (faces.len() == expected && faces.len() >= 3).then_some(Fan {
            faces,
            next,
            open: false,
            start_prev: 0,
        });
    }
    // On the boundary: walk again from the first face of the strip (the one with nothing before it).
    let mut first = None;
    for (fi, face) in soup.faces.iter().enumerate() {
        let Some(i) = face.iter().position(|&x| x == v) else { continue };
        let prev = face[(i + face.len() - 1) % face.len()];
        if !directed.contains_key(&(v, prev)) {
            if first.is_some() {
                return None;
            }
            first = Some((fi, prev));
        }
    }
    let (mut f, start_prev) = first?;
    let (mut faces, mut next) = (Vec::new(), Vec::new());
    loop {
        let face = &soup.faces[f];
        let i = face.iter().position(|&x| x == v)?;
        let n = face[(i + 1) % face.len()];
        faces.push(f);
        next.push(n);
        match directed.get(&(n, v)) {
            Some(&following) if faces.len() <= expected => f = following,
            Some(_) => return None,
            None => break,
        }
    }
    (faces.len() == expected && faces.len() == 2).then_some(Fan {
        faces,
        next,
        open: true,
        start_prev,
    })
}

impl Model {
    fn side(&self, edge: (u32, u32), face: usize, scale: f64) -> Option<(f64, V3)> {
        let info = self.edges.get(&edge)?;
        let s = usize::from(info.faces[0] != face);
        (info.faces[s] == face).then(|| (info.offset[s] * scale, info.inward[s]))
    }

    /// Whether the `count` faces of a sector (counting from beveled edge `i`) all lie in one plane.
    fn run_is_flat(&self, fan: &Fan, i: usize, count: usize) -> bool {
        let first = self.normals[fan.faces[fan.at(i + 1)]];
        (2..=count).all(|a| dot(first, self.normals[fan.faces[fan.at(i + a)]]) > 0.99999)
    }

    fn p(&self, v: u32) -> V3 {
        p3(self.soup.positions[v as usize])
    }

    /// Boundary vertices of every sector at every affected vertex, for the given width scale.
    fn layout(
        &self,
        fans: &BTreeMap<u32, Fan>,
        selected: &BTreeSet<(u32, u32)>,
        scale: f64,
    ) -> Option<Layout> {
        let mut vertices = BTreeMap::new();
        let mut plain_use: BTreeMap<(u32, u32), f64> = BTreeMap::new();
        let mut bevel_use: BTreeMap<(u32, u32), f64> = BTreeMap::new();
        for (&v, fan) in fans {
            let d = fan.len();
            let beveled: Vec<usize> = (0..d)
                .filter(|&i| selected.contains(&key(v, fan.next[i])))
                .collect();
            let k = beveled.len();
            let mut sectors = Vec::with_capacity(k);
            for t in 0..k {
                let i = beveled[t];
                let j = beveled[(t + 1) % k];
                let count = if k == 1 { d } else { (j + d - i) % d };
                // Any sector size works where two or more edges are beveled; a single beveled edge needs the faces
                // to close round it (or a boundary vertex with two faces).
                let allowed = match k {
                    1 if fan.open => d == 2,
                    1 => d >= 3,
                    _ => true,
                };
                if !allowed {
                    return None;
                }
                let positions = self.sector_positions(v, fan, i, count, scale, &mut plain_use)?;
                let flat = k >= 2 && count >= 3 && self.run_is_flat(fan, i, count);
                sectors.push(Sector {
                    first: i + 1,
                    count,
                    positions,
                    flat,
                });
            }
            for (t, &i) in beveled.iter().enumerate() {
                let other = fan.next[i];
                let along = unit(sub(self.p(other), self.p(v)))?;
                let before = &sectors[(t + k - 1) % k];
                let after = &sectors[t];
                let (Some(s), Some(e)) = (before.positions.last(), after.positions.first()) else {
                    return None;
                };
                let shift = dot(sub(*s, self.p(v)), along)
                    .max(dot(sub(*e, self.p(v)), along))
                    .max(0.0);
                *bevel_use.entry(key(v, other)).or_insert(0.0) += shift;
            }
            vertices.insert(
                v,
                VertexGeometry {
                    fan: Fan {
                        faces: fan.faces.clone(),
                        next: fan.next.clone(),
                        open: fan.open,
                        start_prev: fan.start_prev,
                    },
                    beveled,
                    sectors,
                },
            );
        }
        Some(Layout {
            vertices,
            plain_use,
            bevel_use,
        })
    }

    fn sector_positions(
        &self,
        v: u32,
        fan: &Fan,
        i: usize,
        count: usize,
        scale: f64,
        plain_use: &mut BTreeMap<(u32, u32), f64>,
    ) -> Option<Vec<V3>> {
        let pv = self.p(v);
        let e_start = key(v, fan.next[fan.at(i)]);
        let e_end = key(v, fan.next[fan.at(i + count)]);
        let first = fan.faces[fan.at(i + 1)];
        let last = fan.faces[fan.at(i + count)];
        // Two or more beveled edges with only coplanar faces between them (a flat patch cut into triangles): the whole
        // run behaves as one face, so one miter point in its plane stands in for the vertex.
        let flat = scale > 0.0 && count >= 3 && self.run_is_flat(fan, i, count);
        if count == 1 || flat {
            let (oa, da) = self.side(e_start, first, scale)?;
            let (ob, db) = self.side(e_end, if flat { last } else { first }, scale)?;
            let c = dot(da, db);
            let det = 1.0 - c * c;
            let x = if det < 1e-9 {
                mul(da, oa)
            } else {
                let (l, m) = ((oa - ob * c) / det, (ob - oa * c) / det);
                let x = add(mul(da, l), mul(db, m));
                let cap = 8.0 * oa.max(ob);
                if norm(x) > cap {
                    mul(x, cap / norm(x))
                } else {
                    x
                }
            };
            return Some(vec![add(pv, x)]);
        }
        let on_plain = |edge_vertex: u32,
                        off: f64,
                        dir: V3,
                        usage: &mut BTreeMap<(u32, u32), f64>|
         -> Option<(V3, f64)> {
            let uu = unit(sub(self.p(edge_vertex), pv))?;
            let cos = dot(uu, dir);
            if cos < 1e-4 {
                return None;
            }
            let t = off / cos;
            let _ = usage;
            Some((uu, t))
        };
        if fan.open {
            // A beveled edge that ends on the boundary: the offset line of each side's face meets that face's
            // boundary edge, so the two sides get one vertex each and the profile joins them.
            let plain_a = fan.next[fan.at(i + 1)];
            let plain_b = fan.start_prev;
            let (oa, da) = self.side(e_start, first, scale)?;
            let (ob, db) = self.side(e_end, last, scale)?;
            let (ua, ta) = on_plain(plain_a, oa, da, plain_use)?;
            let (ub, tb) = on_plain(plain_b, ob, db, plain_use)?;
            *plain_use.entry(key(v, plain_a)).or_insert(0.0) += ta;
            *plain_use.entry(key(v, plain_b)).or_insert(0.0) += tb;
            return Some(vec![add(pv, mul(ua, ta)), add(pv, mul(ub, tb))]);
        }
        if count == 2 {
            let plain = fan.next[fan.at(i + 1)];
            let (oa, da) = self.side(e_start, first, scale)?;
            let (ob, db) = self.side(e_end, last, scale)?;
            let (uu, t1) = on_plain(plain, oa, da, plain_use)?;
            let (_, t2) = on_plain(plain, ob, db, plain_use)?;
            let t = (t1 + t2) / 2.0;
            *plain_use.entry(key(v, plain)).or_insert(0.0) += t;
            return Some(vec![add(pv, mul(uu, t))]);
        }
        // Three faces (a single beveled edge at a valence-three vertex).
        let plain_a = fan.next[fan.at(i + 1)];
        let plain_b = fan.next[fan.at(i + count - 1)];
        let (oa, da) = self.side(e_start, first, scale)?;
        let (ob, db) = self.side(e_end, last, scale)?;
        let (ua, ta) = on_plain(plain_a, oa, da, plain_use)?;
        let (ub, tb) = on_plain(plain_b, ob, db, plain_use)?;
        *plain_use.entry(key(v, plain_a)).or_insert(0.0) += ta;
        *plain_use.entry(key(v, plain_b)).or_insert(0.0) += tb;
        Some(vec![add(pv, mul(ua, ta)), add(pv, mul(ub, tb))])
    }

    /// A width scale at most one that keeps every cut inside the edges it runs along.
    fn clamp_scale(&self, layout: &Layout) -> f64 {
        let mut scale = 1.0_f64;
        for (&(a, b), &used) in &layout.plain_use {
            let length = norm(sub(self.p(a), self.p(b)));
            if used > 1e-12 {
                scale = scale.min(0.98 * length / used);
            }
        }
        for (&(a, b), &used) in &layout.bevel_use {
            let length = norm(sub(self.p(a), self.p(b)));
            if used > 1e-12 {
                scale = scale.min(0.98 * length / used);
            }
        }
        // A face must stay wide enough for the cuts on its sides: each offset line may use at most 45% of the face's
        // narrowest extent across its edge (a third for a triangle, whose three cuts meet at the incentre).
        for (&(a, b), info) in &self.edges {
            let (pa, pb) = (self.p(a), self.p(b));
            let Some(along) = unit(sub(pb, pa)) else { continue };
            for side in 0..2 {
                let face = &self.soup.faces[info.faces[side]];
                let narrowest = face
                    .iter()
                    .filter(|&&v| v != a && v != b)
                    .map(|&v| norm(cross(sub(self.p(v), pa), along)))
                    .fold(f64::INFINITY, f64::min);
                let share = if face.len() == 3 { 0.33 } else { 0.45 };
                if narrowest.is_finite() && info.offset[side] > 1e-12 {
                    scale = scale.min(share * narrowest / info.offset[side]);
                }
            }
        }
        scale.clamp(1e-6, 1.0)
    }

    /// Profile points strictly between `s` and `t` around the corner `e`.
    fn profile(&self, s: V3, t: V3, e: V3) -> Vec<V3> {
        let n = self.segments as usize;
        let o = sub(add(s, t), e);
        let (a, b) = (sub(e, t), sub(e, s));
        (1..n)
            .map(|j| {
                #[expect(clippy::cast_precision_loss, reason = "segment counts are at most 64")]
                let angle = j as f64 / n as f64 * std::f64::consts::FRAC_PI_2;
                let c = angle.cos().max(0.0).powf(self.exponent);
                let sn = angle.sin().max(0.0).powf(self.exponent);
                add(o, add(mul(a, c), mul(b, sn)))
            })
            .collect()
    }

    fn add_vertex(&mut self, position: V3, origin: u32) -> u32 {
        self.soup.add_vertex(to_f32(position), origin)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "vertex patches and strips are emitted in one deterministic pass"
    )]
    fn build(
        mut self,
        layout: Layout,
        selected: &BTreeSet<(u32, u32)>,
    ) -> Result<Option<BeveledMesh>, KernelError> {
        // Per (vertex, beveled index): the chain of vertex ids from the face before the edge to the face after it.
        let mut chains: BTreeMap<(u32, usize), Vec<u32>> = BTreeMap::new();
        let mut replacements: BTreeMap<(u32, usize), Vec<u32>> = BTreeMap::new();
        let mut extra_faces: Vec<(Vec<u32>, usize)> = Vec::new();
        for (&v, geometry) in &layout.vertices {
            let fan = &geometry.fan;
            let k = geometry.beveled.len();
            let pv = self.p(v);
            // Sector vertex ids.
            let mut sector_ids: Vec<Vec<u32>> = Vec::with_capacity(k);
            for sector in &geometry.sectors {
                sector_ids.push(
                    sector
                        .positions
                        .iter()
                        .map(|&p| self.add_vertex(p, v))
                        .collect(),
                );
            }
            // Profile chains, one per beveled edge.
            let mut chain_positions: Vec<Vec<V3>> = Vec::with_capacity(k);
            let mut ends: Vec<(V3, V3)> = Vec::with_capacity(k);
            for (t, &i) in geometry.beveled.iter().enumerate() {
                let other = fan.next[i];
                let (Some(&s), Some(&e)) = (
                    geometry.sectors[(t + k - 1) % k].positions.last(),
                    geometry.sectors[t].positions.first(),
                ) else {
                    return Ok(None);
                };
                let Some(along) = unit(sub(self.p(other), pv)) else {
                    return Ok(None);
                };
                let mid = mul(add(s, e), 0.5);
                let corner = add(pv, mul(along, dot(sub(mid, pv), along)));
                chain_positions.push(self.profile(s, e, corner));
                ends.push((s, e));
            }
            // Two chains that lie on top of each other (a straight run through the vertex) are one curve.
            let shared = k == 2 && same_curve(&chain_positions[0], &chain_positions[1]);
            for (t, positions) in chain_positions.iter().enumerate() {
                let before = &sector_ids[(t + k - 1) % k];
                let after = &sector_ids[t];
                let (Some(&first), Some(&last)) = (before.last(), after.first()) else {
                    return Ok(None);
                };
                let mut ids = vec![first];
                if shared && t == 1 {
                    let mut again = chains[&(v, 0)].clone();
                    again.reverse();
                    chains.insert((v, t), again);
                    continue;
                }
                for &p in positions {
                    ids.push(self.add_vertex(p, v));
                }
                ids.push(last);
                chains.insert((v, t), ids);
            }
            let _ = ends;

            // What each face around the vertex uses in place of it.
            // Cut-point pairs of sectors in which the vertex itself stays (see below).
            let mut persist: Vec<(u32, u32)> = Vec::new();
            for (t, sector) in geometry.sectors.iter().enumerate() {
                let ids = &sector_ids[t];
                for a in 0..sector.count {
                    let face = fan.faces[fan.at(sector.first + a)];
                    let list = if fan.open {
                        vec![ids[usize::from(a != 0)]]
                    } else if sector.count <= 2 || a == 0 || sector.flat {
                        vec![ids[0]]
                    } else if a + 1 == sector.count {
                        vec![ids[1]]
                    } else if k == 1 && sector.count == 3 {
                        let mut run = chains[&(v, 0)].clone();
                        run.reverse();
                        run
                    } else if sector.count >= 4 {
                        // Four or more faces in the sector: the vertex stays, joined to the two cut points by the
                        // first and last of the middle faces, and a cap covers what is left (the profile for one
                        // beveled edge, a triangle to the patch otherwise).
                        let mut list = Vec::new();
                        if a == 1 {
                            list.push(ids[0]);
                        }
                        list.push(v);
                        if a + 2 == sector.count {
                            list.push(ids[1]);
                        }
                        list
                    } else {
                        vec![ids[0], ids[1]]
                    };
                    replacements.insert((v, face), list);
                }
                if k == 1 && sector.count >= 4 && !fan.open {
                    let mut cap = vec![v];
                    let mut run = chains[&(v, 0)].clone();
                    run.reverse();
                    cap.extend(run);
                    extra_faces.push((cap, fan.faces[0]));
                } else if k >= 2 && sector.count >= 4 && !sector.flat {
                    persist.push((ids[0], ids[1]));
                }
            }

            // The patch between three or more chains, or the zipper between two.
            if k >= 2 && !shared {
                let n = self.segments as usize;
                let mut ring = Vec::with_capacity(k * n);
                for t in 0..k {
                    ring.extend(sector_ids[t].iter().copied());
                    let chain = &chains[&(v, (t + 1) % k)];
                    ring.extend(chain[1..chain.len() - 1].iter().copied());
                }
                // One boundary vertex per sector gives the regular ring the patches below are built for; sectors
                // with two (cut points on the first and last plain edge) make it irregular.
                let regular = ring.len() == k * n;
                let mut expected = [0.0; 3];
                for &f in &fan.faces {
                    expected = add(expected, self.normals[f]);
                }
                let parent = fan.faces[0];
                let positions: Vec<V3> = ring
                    .iter()
                    .map(|&id| p3(self.soup.positions[id as usize]))
                    .collect();
                // The patch must walk every chain edge against the strip beside it. The strip is wound by its two
                // faces, so work out which way it walks the first chain at this vertex; the ring runs along the chain.
                let strip_forward = {
                    let first = geometry.beveled[0];
                    let other = fan.next[first];
                    let local = add(self.normals[fan.faces[first]], self.normals[fan.faces[fan.at(first + 1)]]);
                    let chain = &chains[&(v, 0)];
                    let (p0, p1) = (
                        p3(self.soup.positions[chain[0] as usize]),
                        p3(self.soup.positions[chain[1] as usize]),
                    );
                    let along = unit(sub(self.p(other), self.p(v)));
                    along.map(|along| dot(cross(sub(p1, p0), along), local) > 0.0)
                };
                match strip_forward {
                    Some(true) => ring[1..].reverse(),
                    Some(false) => {}
                    None => {
                        if ring_normal(&positions).is_some_and(|ring_n| dot(ring_n, expected) < 0.0) {
                            ring[1..].reverse();
                        }
                    }
                }
                // A sector in which the vertex stays leaves a triangle to the patch: the patch must walk its edge
                // against the triangle, so the ring runs from the last cut point to the first.
                if let Some(&(first, last)) = persist.first() {
                    let at = ring.iter().position(|&id| id == first);
                    if at.is_some_and(|i| ring[(i + 1) % ring.len()] == last) {
                        ring.reverse();
                    }
                }
                for &(first, last) in &persist {
                    extra_faces.push((vec![v, first, last], parent));
                }
                let sphere = self.sphere(geometry, &sector_ids, v);
                let faces = if !regular {
                    // An irregular ring is capped with a fan from its middle.
                    let points: Vec<V3> = ring
                        .iter()
                        .map(|&id| p3(self.soup.positions[id as usize]))
                        .collect();
                    #[expect(clippy::cast_precision_loss, reason = "ring sizes are small")]
                    let mean = mul(points.iter().fold([0.0; 3], |a, &q| add(a, q)), 1.0 / points.len() as f64);
                    // The mean of the ring lies inside it, so the cap never pokes past the original shape.
                    let hub = self.add_vertex(mean, v);
                    (0..ring.len())
                        .map(|i| vec![ring[i], ring[(i + 1) % ring.len()], hub])
                        .collect()
                } else if k == 2 {
                    zipper(&ring, n)
                } else {
                    self.fill(&ring, k, n, v, sphere)
                };
                for face in faces {
                    extra_faces.push((face, parent));
                }
            }
        }

        // Rewrite the original faces.
        for (f, face) in self.soup.faces.iter_mut().enumerate() {
            let mut rewritten = Vec::with_capacity(face.len() + 2);
            for &v in face.iter() {
                match replacements.get(&(v, f)) {
                    Some(list) => rewritten.extend(list.iter().copied()),
                    None => rewritten.push(v),
                }
            }
            *face = rewritten;
        }
        // Strips along the beveled edges.
        let n = self.segments as usize;
        for &(lo, hi) in selected {
            let Some(info) = self.edges.get(&(lo, hi)) else {
                return Ok(None);
            };
            let faces = info.faces;
            let expected = add(self.normals[faces[0]], self.normals[faces[1]]);
            let mut ends: Vec<Vec<u32>> = Vec::with_capacity(2);
            for v in [lo, hi] {
                let Some(geometry) = layout.vertices.get(&v) else {
                    return Ok(None);
                };
                let other = if v == lo { hi } else { lo };
                let Some(t) = geometry
                    .beveled
                    .iter()
                    .position(|&i| geometry.fan.next[i] == other)
                else {
                    return Ok(None);
                };
                let Some(chain) = chains.get(&(v, t)) else {
                    return Ok(None);
                };
                let before = geometry.fan.faces[geometry.beveled[t]];
                let mut ids = chain.clone();
                if before != faces[0] {
                    ids.reverse();
                }
                ends.push(ids);
            }
            for j in 0..n {
                let mut quad = vec![ends[0][j], ends[0][j + 1], ends[1][j + 1], ends[1][j]];
                let positions: Vec<V3> = quad
                    .iter()
                    .map(|&id| p3(self.soup.positions[id as usize]))
                    .collect();
                if ring_normal(&positions).is_some_and(|q| dot(q, expected) < 0.0) {
                    quad.reverse();
                }
                extra_faces.push((quad, faces[0]));
            }
        }
        for (corners, parent) in extra_faces {
            let parent = self.soup.face_parent[parent];
            self.soup.add_face(corners, parent, 0);
        }
        Ok(Some(self.soup.finish()))
    }

    /// A sphere (centre, radius) touching every face of a round corner patch, when one fits.
    fn sphere(
        &self,
        geometry: &VertexGeometry,
        sector_ids: &[Vec<u32>],
        v: u32,
    ) -> Option<(V3, f64)> {
        if !self.round || geometry.beveled.len() < 3 {
            return None;
        }
        let mut centre = [0.0; 3];
        for (t, sector) in geometry.sectors.iter().enumerate() {
            let face = geometry.fan.faces[geometry.fan.at(sector.first)];
            let point = p3(self.soup.positions[sector_ids[t][0] as usize]);
            let edge = key(v, geometry.fan.next[geometry.fan.at(sector.first - 1)]);
            let (offset, _) = self.side(edge, face, 1.0)?;
            centre = add(centre, sub(point, mul(self.normals[face], offset)));
        }
        #[expect(clippy::cast_precision_loss, reason = "a handful of sectors")]
        let centre = mul(centre, 1.0 / geometry.sectors.len() as f64);
        let radius = geometry
            .sectors
            .iter()
            .enumerate()
            .map(|(t, _)| {
                norm(sub(
                    p3(self.soup.positions[sector_ids[t][0] as usize]),
                    centre,
                ))
            })
            .sum::<f64>()
            / geometry.sectors.len() as f64;
        (radius > 1e-9).then_some((centre, radius))
    }

    /// Fills a ring of `k * m` vertices (corners every `m`) with concentric rings of quads.
    fn fill(
        &mut self,
        ring: &[u32],
        k: usize,
        m: usize,
        origin: u32,
        sphere: Option<(V3, f64)>,
    ) -> Vec<Vec<u32>> {
        let mut faces = Vec::new();
        let mut ring = ring.to_vec();
        let mut m = m;
        loop {
            if m == 1 {
                faces.push(ring);
                return faces;
            }
            let positions: Vec<V3> = ring
                .iter()
                .map(|&id| p3(self.soup.positions[id as usize]))
                .collect();
            #[expect(clippy::cast_precision_loss, reason = "ring sizes are small")]
            let centroid = mul(
                positions.iter().fold([0.0; 3], |a, &p| add(a, p)),
                1.0 / positions.len() as f64,
            );
            #[expect(clippy::cast_precision_loss, reason = "ring sizes are small")]
            let shrink = (m - 2) as f64 / m as f64;
            // The cap bulges toward the original corner but never past it: keep points inside the box spanned by the
            // ring and the corner.
            let corner = self.p(origin);
            let (mut low, mut high) = (corner, corner);
            for p in &positions {
                for k in 0..3 {
                    low[k] = low[k].min(p[k]);
                    high[k] = high[k].max(p[k]);
                }
            }
            let place = |p: V3| {
                let flat = add(centroid, mul(sub(p, centroid), shrink));
                let mut point = match sphere {
                    Some((c, r)) => match unit(sub(flat, c)) {
                        Some(dir) => add(c, mul(dir, r)),
                        None => flat,
                    },
                    None => flat,
                };
                for k in 0..3 {
                    point[k] = point[k].clamp(low[k], high[k]);
                }
                point
            };
            if m == 2 {
                let centre = self.add_vertex(place(centroid), origin);
                for i in 0..k {
                    let n = ring.len();
                    faces.push(vec![
                        ring[(2 * i + n - 1) % n],
                        ring[2 * i],
                        ring[(2 * i + 1) % n],
                        centre,
                    ]);
                }
                return faces;
            }
            let per = m - 2;
            let mut inner = Vec::with_capacity(k * per);
            for i in 0..k {
                for a in 0..per {
                    #[expect(clippy::cast_precision_loss, reason = "ring sizes are small")]
                    let at = a as f64 * m as f64 / per as f64;
                    let lo = at.floor();
                    let frac = at - lo;
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "at is a small non-negative index"
                    )]
                    let lo = lo as usize;
                    let n = positions.len();
                    let p0 = positions[(i * m + lo) % n];
                    let p1 = positions[(i * m + lo + 1) % n];
                    let sample = add(mul(p0, 1.0 - frac), mul(p1, frac));
                    inner.push(self.add_vertex(place(sample), origin));
                }
            }
            let n = ring.len();
            let kk = inner.len();
            for i in 0..k {
                faces.push(vec![
                    ring[(i * m + n - 1) % n],
                    ring[i * m],
                    ring[i * m + 1],
                    inner[i * per],
                ]);
                for t in 1..=per {
                    faces.push(vec![
                        ring[i * m + t],
                        ring[(i * m + t + 1) % n],
                        inner[(i * per + t) % kk],
                        inner[i * per + t - 1],
                    ]);
                }
            }
            ring = inner;
            m = per;
        }
    }
}

/// Newell normal of a polygon given by points.
fn ring_normal(points: &[V3]) -> Option<V3> {
    let mut n = [0.0; 3];
    for i in 0..points.len() {
        let (a, b) = (points[i], points[(i + 1) % points.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    unit(n)
}

fn same_curve(a: &[V3], b: &[V3]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter().rev())
            .all(|(&p, &q)| norm(sub(p, q)) < 1e-5 * (1.0 + norm(p)))
}

/// Joins a ring made of two profile chains (corners at `0` and `n`) with triangles and quads.
fn zipper(ring: &[u32], n: usize) -> Vec<Vec<u32>> {
    let len = ring.len();
    let mut faces = Vec::new();
    if n < 2 {
        return faces;
    }
    faces.push(vec![ring[0], ring[1], ring[len - 1]]);
    for t in 1..n - 1 {
        faces.push(vec![ring[t], ring[t + 1], ring[len - t - 1], ring[len - t]]);
    }
    faces.push(vec![ring[n - 1], ring[n], ring[n + 1]]);
    faces
}
