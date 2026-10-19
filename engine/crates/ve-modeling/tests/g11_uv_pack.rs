//! `poly.pack_uv` measured on a box: islands do not overlap, they keep their margin, coverage is high, rotation and density options hold.
//!
//! Islands are found from the evaluated part: triangles that share a UV corner belong to one island. Overlap and gaps are measured on a
//! rasterised grid, so the assertions do not depend on how the packer works inside.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "test fixtures are valid, and grid indices are small"
)]
use std::collections::BTreeMap;

use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Part, Quality};

fn part(text: &str) -> Part {
    let batch = parse_text(&builtin_registry(), text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_uv_pack").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    let evaluation = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|e| panic!("{}: {}", e.code, e.message));
    evaluation.parts[0].clone()
}

const BOX: &str = "box = solid.box@1(size=[1, 2, 3])\ntris = poly.from_solid@1(box)\nmesh = poly.merge_coplanar@1(tris)\nuv = poly.unwrap_uv@1(mesh)\n";

fn unpacked() -> Part {
    part(&format!("{BOX}solid = poly.to_solid@1(uv)\noutput solid"))
}

fn packed(options: &str) -> Part {
    part(&format!(
        "{BOX}packed = poly.pack_uv@1(uv, {options})\nsolid = poly.to_solid@1(packed)\noutput solid"
    ))
}

/// One triangle: its three corner UVs and its three corner positions.
struct Tri {
    uv: [[f64; 2]; 3],
    pos: [[f64; 3]; 3],
}

fn triangles(part: &Part) -> Vec<Tri> {
    // UVs are one per triangle corner (parallel to `indices`); positions are indexed.
    assert_eq!(part.uvs.len(), part.indices.len(), "uvs per corner");
    part.indices
        .chunks_exact(3)
        .enumerate()
        .map(|(t, corner)| Tri {
            uv: [0, 1, 2].map(|k| part.uvs[t * 3 + k].map(f64::from)),
            pos: [0, 1, 2].map(|k| part.positions[corner[k] as usize].map(f64::from)),
        })
        .collect()
}

/// Island number of every triangle: triangles that share a UV corner are one island.
fn islands(tris: &[Tri]) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..tris.len()).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    let mut owner: BTreeMap<(u64, u64), usize> = BTreeMap::new();
    for (t, tri) in tris.iter().enumerate() {
        for corner in tri.uv {
            let key = (corner[0].to_bits(), corner[1].to_bits());
            match owner.get(&key) {
                Some(&other) => {
                    let (a, b) = (find(&mut parent, t), find(&mut parent, other));
                    parent[a] = b;
                }
                None => {
                    owner.insert(key, t);
                }
            }
        }
    }
    let roots: Vec<usize> = (0..tris.len()).map(|t| find(&mut parent, t)).collect();
    let mut number: BTreeMap<usize, usize> = BTreeMap::new();
    roots
        .iter()
        .map(|root| {
            let next = number.len();
            *number.entry(*root).or_insert(next)
        })
        .collect()
}

/// A square grid of the unit UV square: the island that covers each cell centre, and how many islands claimed a cell.
struct Raster {
    size: usize,
    cell: Vec<Option<usize>>,
    overlapping: usize,
}

fn raster(tris: &[Tri], island: &[usize], size: usize) -> Raster {
    let mut cell = vec![None; size * size];
    let mut overlapping = 0;
    for (tri, &id) in tris.iter().zip(island) {
        let [a, b, c] = tri.uv;
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
        if area.abs() < 1.0e-12 {
            continue;
        }
        let (lo, hi) = |axis: usize| -> (f64, f64) {
            (
                a[axis].min(b[axis]).min(c[axis]),
                a[axis].max(b[axis]).max(c[axis]),
            )
        }(0);
        let (lov, hiv) = (a[1].min(b[1]).min(c[1]), a[1].max(b[1]).max(c[1]));
        let range = |lo: f64, hi: f64| {
            let first = (lo * size as f64).floor().max(0.0) as usize;
            let last = ((hi * size as f64).ceil() as usize).min(size - 1);
            first..=last
        };
        for y in range(lov, hiv) {
            for x in range(lo, hi) {
                let p = [
                    (x as f64 + 0.5) / size as f64,
                    (y as f64 + 0.5) / size as f64,
                ];
                let w0 = ((b[0] - p[0]) * (c[1] - p[1]) - (c[0] - p[0]) * (b[1] - p[1])) / area;
                let w1 = ((c[0] - p[0]) * (a[1] - p[1]) - (a[0] - p[0]) * (c[1] - p[1])) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    match cell[y * size + x] {
                        Some(other) if other != id => overlapping += 1,
                        _ => cell[y * size + x] = Some(id),
                    }
                }
            }
        }
    }
    Raster {
        size,
        cell,
        overlapping,
    }
}

impl Raster {
    fn coverage(&self) -> f64 {
        self.cell.iter().filter(|c| c.is_some()).count() as f64 / (self.size * self.size) as f64
    }

    /// The smallest gap between two different islands in UV units, looking up to `reach` cells away (None if none is that close).
    fn min_gap(&self, reach: usize) -> Option<f64> {
        let size = self.size as isize;
        let mut best: Option<usize> = None;
        for y in 0..size {
            for x in 0..size {
                let Some(me) = self.cell[(y * size + x) as usize] else {
                    continue;
                };
                for dy in -(reach as isize)..=(reach as isize) {
                    for dx in -(reach as isize)..=(reach as isize) {
                        let (nx, ny) = (x + dx, y + dy);
                        if nx < 0 || ny < 0 || nx >= size || ny >= size {
                            continue;
                        }
                        if let Some(other) = self.cell[(ny * size + nx) as usize]
                            && other != me
                        {
                            let d = dx.unsigned_abs().max(dy.unsigned_abs());
                            best = Some(best.map_or(d, |b| b.min(d)));
                        }
                    }
                }
            }
        }
        // Two cells d apart (centre to centre) leave d - 1 empty cells between their edges.
        best.map(|d| (d.saturating_sub(1)) as f64 / self.size as f64)
    }
}

/// Per island: bounding-box aspect (width / height) of its UVs, sorted so islands match before and after packing.
fn aspects(tris: &[Tri], island: &[usize]) -> Vec<f64> {
    let count = island.iter().max().map_or(0, |m| m + 1);
    let mut boxes = vec![
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY
        ];
        count
    ];
    for (tri, &id) in tris.iter().zip(island) {
        for c in tri.uv {
            boxes[id][0] = boxes[id][0].min(c[0]);
            boxes[id][1] = boxes[id][1].min(c[1]);
            boxes[id][2] = boxes[id][2].max(c[0]);
            boxes[id][3] = boxes[id][3].max(c[1]);
        }
    }
    let mut out: Vec<f64> = boxes
        .iter()
        .map(|b| (b[2] - b[0]) / (b[3] - b[1]))
        .collect();
    out.sort_by(f64::total_cmp);
    out
}

/// Per island: UV area divided by 3D area.
fn densities(tris: &[Tri], island: &[usize]) -> Vec<f64> {
    let count = island.iter().max().map_or(0, |m| m + 1);
    let (mut uv, mut world) = (vec![0.0; count], vec![0.0; count]);
    for (tri, &id) in tris.iter().zip(island) {
        let [a, b, c] = tri.uv;
        uv[id] += 0.5 * ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs();
        let [p, q, r] = tri.pos;
        let (u, v) = (
            [q[0] - p[0], q[1] - p[1], q[2] - p[2]],
            [r[0] - p[0], r[1] - p[1], r[2] - p[2]],
        );
        let cross = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        world[id] += 0.5 * (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    }
    uv.iter().zip(&world).map(|(u, w)| u / w).collect()
}

#[test]
fn a_box_unwraps_into_several_islands() {
    let tris = triangles(&unpacked());
    let island = islands(&tris);
    let count = island.iter().max().unwrap() + 1;
    assert!(
        count >= 2,
        "islands {count}: the packing tests need more than one"
    );
}

#[test]
fn islands_do_not_overlap_and_cover_more_than_half() {
    let tris = triangles(&packed("margin=0.02"));
    let island = islands(&tris);
    let grid = raster(&tris, &island, 256);
    assert_eq!(
        grid.overlapping,
        0,
        "{} cells claimed by two islands ({} islands)",
        grid.overlapping,
        island.iter().max().unwrap() + 1
    );
    assert!(grid.coverage() > 0.5, "coverage {}", grid.coverage());
}

#[test]
fn a_larger_margin_leaves_a_larger_gap() {
    let measure = |margin: &str| {
        let tris = triangles(&packed(&format!("margin={margin}")));
        let island = islands(&tris);
        raster(&tris, &island, 256).min_gap(40)
    };
    let (small, large) = (measure("0.01"), measure("0.05"));
    let (small, large) = (small.unwrap_or(0.0), large.unwrap_or(1.0));
    assert!(
        large > small,
        "gap at margin 0.05 = {large}, at 0.01 = {small}"
    );
    assert!(
        large >= 0.03,
        "gap at margin 0.05 was {large}, wanted at least 0.03"
    );
}

#[test]
fn rotate_false_keeps_each_islands_orientation() {
    let before = {
        let tris = triangles(&unpacked());
        aspects(&tris, &islands(&tris))
    };
    let after = {
        let tris = triangles(&packed("rotate=false"));
        aspects(&tris, &islands(&tris))
    };
    assert_eq!(before.len(), after.len(), "island count changed");
    for (b, a) in before.iter().zip(&after) {
        assert!(
            (b - a).abs() / b < 0.02,
            "aspects before {before:?} after {after:?}"
        );
    }
}

#[test]
fn keep_density_gives_every_island_the_same_texel_density() {
    let tris = triangles(&packed("keep_density=true"));
    let density = densities(&tris, &islands(&tris));
    let (lo, hi) = density
        .iter()
        .fold((f64::INFINITY, 0.0_f64), |(lo, hi), d| {
            (lo.min(*d), hi.max(*d))
        });
    assert!((hi - lo) / hi < 0.05, "island densities {density:?}");
}
