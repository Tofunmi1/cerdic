//! The layout of a modern district as pure logic: an irregular street network, the blocks between the streets, and the lots each block is cut into, each with a height.
//! Parish and Mueller's pipeline (streets, blocks, lots, buildings) with a warped grid for the streets and recursive splitting for the lots. Written before the body; do not weaken.
use ve_world::city::{CityError, CityParams, Layout, layout};

fn params() -> CityParams {
    CityParams::default()
}

fn area(polygon: &[[f32; 2]]) -> f32 {
    let mut sum = 0.0;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        sum += a[0] * b[1] - b[0] * a[1];
    }
    sum * 0.5
}

fn bounds(polygon: &[[f32; 2]]) -> ([f32; 2], [f32; 2]) {
    polygon
        .iter()
        .fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| {
            (
                [lo[0].min(p[0]), lo[1].min(p[1])],
                [hi[0].max(p[0]), hi[1].max(p[1])],
            )
        })
}

fn segment_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (ab, ap) = ([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
    let length2 = (ab[0] * ab[0] + ab[1] * ab[1]).max(1e-12);
    let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / length2).clamp(0.0, 1.0);
    let q = [a[0] + t * ab[0], a[1] + t * ab[1]];
    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2)).sqrt()
}

/// The smallest distance between the outlines of two convex polygons (0 when they cross or touch).
fn polygon_distance(a: &[[f32; 2]], b: &[[f32; 2]]) -> f32 {
    let inside = |p: [f32; 2], poly: &[[f32; 2]]| {
        (0..poly.len()).all(|i| {
            let (u, v) = (poly[i], poly[(i + 1) % poly.len()]);
            (v[0] - u[0]) * (p[1] - u[1]) - (v[1] - u[1]) * (p[0] - u[0]) >= -1e-4
        })
    };
    if a.iter().any(|p| inside(*p, b)) || b.iter().any(|p| inside(*p, a)) {
        return 0.0;
    }
    let mut best = f32::MAX;
    for p in a {
        for i in 0..b.len() {
            best = best.min(segment_distance(*p, b[i], b[(i + 1) % b.len()]));
        }
    }
    for p in b {
        for i in 0..a.len() {
            best = best.min(segment_distance(*p, a[i], a[(i + 1) % a.len()]));
        }
    }
    best
}

fn default_layout() -> Layout {
    layout(&params()).unwrap()
}

#[test]
fn the_same_seed_gives_the_same_district_and_another_seed_a_different_one() {
    let a = layout(&CityParams {
        seed: 7,
        ..params()
    })
    .unwrap();
    let b = layout(&CityParams {
        seed: 7,
        ..params()
    })
    .unwrap();
    let c = layout(&CityParams {
        seed: 8,
        ..params()
    })
    .unwrap();
    assert_eq!(a, b, "bit-identical for one seed");
    assert_ne!(a.lots.len() + a.lots[0].footprint.len(), 0);
    assert_ne!(
        a.lots[0].footprint, c.lots[0].footprint,
        "another seed lays it out differently"
    );
}

#[test]
fn every_lot_is_a_proper_polygon_inside_the_district() {
    let p = params();
    let l = default_layout();
    let half = [p.size[0] * 0.5, p.size[1] * 0.5];
    assert!(!l.lots.is_empty());
    for (i, lot) in l.lots.iter().enumerate() {
        assert!(
            lot.footprint.len() >= 3 && lot.footprint.len() <= 8,
            "lot {i} has {} corners",
            lot.footprint.len()
        );
        assert!(
            area(&lot.footprint) > 0.0,
            "lot {i} winds counter-clockwise (positive area)"
        );
        let (lo, hi) = bounds(&lot.footprint);
        assert!(
            lo[0] >= -half[0] - 1e-3
                && hi[0] <= half[0] + 1e-3
                && lo[1] >= -half[1] - 1e-3
                && hi[1] <= half[1] + 1e-3,
            "lot {i} sticks out of the district: {lo:?} {hi:?}"
        );
        assert!(lot.footprint.iter().flatten().all(|c| c.is_finite()));
    }
}

#[test]
fn lots_are_small_enough_to_be_buildings_and_large_enough_to_stand_in() {
    let p = CityParams {
        lot_min: 7.0,
        lot_max: 22.0,
        ..params()
    };
    let l = layout(&p).unwrap();
    let mut areas = Vec::new();
    for lot in &l.lots {
        let (lo, hi) = bounds(&lot.footprint);
        let (w, d) = (hi[0] - lo[0], hi[1] - lo[1]);
        assert!(
            w.min(d) >= p.lot_min * 0.5,
            "a lot only {} m across",
            w.min(d)
        );
        assert!(w.max(d) <= p.lot_max * 2.0, "a lot {} m long", w.max(d));
        areas.push(area(&lot.footprint));
    }
    let mean = areas.iter().sum::<f32>() / areas.len() as f32;
    assert!(
        mean > p.lot_min * p.lot_min * 0.8 && mean < p.lot_max * p.lot_max,
        "mean lot area {mean} m2"
    );
}

#[test]
fn lots_never_overlap() {
    let l = default_layout();
    let boxes: Vec<_> = l.lots.iter().map(|lot| bounds(&lot.footprint)).collect();
    for i in 0..l.lots.len() {
        for j in i + 1..l.lots.len() {
            let (a, b) = (&boxes[i], &boxes[j]);
            if a.1[0] <= b.0[0] || b.1[0] <= a.0[0] || a.1[1] <= b.0[1] || b.1[1] <= a.0[1] {
                continue;
            }
            // the boxes overlap: the lots may only touch (shared walls between lots of one block are fine), never cross
            let (pa, pb) = (&l.lots[i].footprint, &l.lots[j].footprint);
            let shrink = |poly: &[[f32; 2]]| {
                let c = poly.iter().fold([0.0, 0.0], |s, p| {
                    [
                        s[0] + p[0] / poly.len() as f32,
                        s[1] + p[1] / poly.len() as f32,
                    ]
                });
                poly.iter()
                    .map(|p| [c[0] + (p[0] - c[0]) * 0.98, c[1] + (p[1] - c[1]) * 0.98])
                    .collect::<Vec<_>>()
            };
            assert!(
                polygon_distance(&shrink(pa), &shrink(pb)) > 0.0,
                "lots {i} and {j} overlap"
            );
        }
    }
}

#[test]
fn streets_keep_their_width_between_blocks() {
    let p = CityParams {
        street_width: 9.0,
        ..params()
    };
    let l = layout(&p).unwrap();
    let mut by_block = std::collections::BTreeMap::<u32, Vec<usize>>::new();
    for (i, lot) in l.lots.iter().enumerate() {
        by_block.entry(lot.block).or_default().push(i);
    }
    assert!(
        by_block.len() >= 12,
        "a district has many blocks: {}",
        by_block.len()
    );
    let blocks: Vec<_> = by_block.values().collect();
    let mut closest = f32::MAX;
    for i in 0..blocks.len() {
        for j in i + 1..blocks.len() {
            for a in blocks[i] {
                for b in blocks[j] {
                    let (ba, bb) = (bounds(&l.lots[*a].footprint), bounds(&l.lots[*b].footprint));
                    if ba.0[0] > bb.1[0] + 20.0
                        || bb.0[0] > ba.1[0] + 20.0
                        || ba.0[1] > bb.1[1] + 20.0
                        || bb.0[1] > ba.1[1] + 20.0
                    {
                        continue;
                    }
                    closest = closest.min(polygon_distance(
                        &l.lots[*a].footprint,
                        &l.lots[*b].footprint,
                    ));
                }
            }
        }
    }
    assert!(
        closest >= p.street_width * 0.45,
        "two blocks come within {closest} m of each other, on streets {} m wide",
        p.street_width
    );
    assert!(
        closest <= p.street_width * 1.6,
        "the narrowest street gap is {closest} m for streets {} m wide: streets are too wide or blocks float apart",
        p.street_width
    );
}

#[test]
fn the_street_network_is_one_connected_graph() {
    let l = default_layout();
    assert!(
        l.street_nodes.len() >= 20 && l.street_edges.len() >= l.street_nodes.len(),
        "a grid of streets has loops"
    );
    let n = l.street_nodes.len();
    let mut seen = vec![false; n];
    let mut stack = vec![0usize];
    seen[0] = true;
    while let Some(v) = stack.pop() {
        for (a, b) in &l.street_edges {
            let (a, b) = (*a as usize, *b as usize);
            assert!(a < n && b < n, "edge names a missing node");
            for (from, to) in [(a, b), (b, a)] {
                if from == v && !seen[to] {
                    seen[to] = true;
                    stack.push(to);
                }
            }
        }
    }
    assert!(
        seen.iter().all(|s| *s),
        "every street node can be reached from every other"
    );
}

#[test]
fn the_middle_is_tall_and_the_edge_is_low_with_a_few_towers() {
    let p = params();
    let l = default_layout();
    let radius = p.size[0].min(p.size[1]) * 0.5;
    let centre = |lot: &ve_world::city::Lot| {
        let c = lot.footprint.iter().fold([0.0, 0.0], |s, q| {
            [
                s[0] + q[0] / lot.footprint.len() as f32,
                s[1] + q[1] / lot.footprint.len() as f32,
            ]
        });
        ((c[0] - p.centre[0]).powi(2) + (c[1] - p.centre[1]).powi(2)).sqrt()
    };
    let (mut inner, mut outer) = (Vec::new(), Vec::new());
    for lot in &l.lots {
        assert!(
            lot.height >= 4.0 && lot.height <= 260.0,
            "height {}",
            lot.height
        );
        if centre(lot) < radius * 0.35 {
            inner.push(lot.height);
        } else if centre(lot) > radius * 0.8 {
            outer.push(lot.height);
        }
    }
    let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
    assert!(inner.len() > 10 && outer.len() > 10);
    assert!(
        mean(&inner) > mean(&outer) * 2.0,
        "downtown {} m against the fringe {} m",
        mean(&inner),
        mean(&outer)
    );
    let towers = l.lots.iter().filter(|lot| lot.height >= 60.0).count();
    assert!(
        towers >= 3 && towers * 12 <= l.lots.len(),
        "{towers} towers among {} lots",
        l.lots.len()
    );
    let tallest = l.lots.iter().map(|lot| lot.height).fold(0.0_f32, f32::max);
    assert!(tallest >= 90.0, "the tallest tower is {tallest} m");
}

#[test]
fn the_default_district_is_a_walkable_size() {
    let l = default_layout();
    assert!(
        (250..=1400).contains(&l.lots.len()),
        "{} lots",
        l.lots.len()
    );
    let blocks = l
        .lots
        .iter()
        .map(|lot| lot.block)
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    assert!(blocks >= 20 && blocks <= 200, "{blocks} blocks");
}

#[test]
fn irregularity_bends_the_streets_and_zero_keeps_a_square_grid() {
    let straight = layout(&CityParams {
        irregularity: 0.0,
        ..params()
    })
    .unwrap();
    let bent = layout(&CityParams {
        irregularity: 1.0,
        ..params()
    })
    .unwrap();
    let off_axis = |l: &Layout| {
        l.street_edges
            .iter()
            .filter(|(a, b)| {
                let (p, q) = (l.street_nodes[*a as usize], l.street_nodes[*b as usize]);
                (p[0] - q[0]).abs() > 0.5 && (p[1] - q[1]).abs() > 0.5
            })
            .count()
    };
    assert_eq!(
        off_axis(&straight),
        0,
        "zero irregularity: every street runs along an axis"
    );
    assert!(
        off_axis(&bent) > straight.street_edges.len() / 4,
        "full irregularity bends many streets"
    );
}

#[test]
fn bad_parameters_are_refused_with_the_name() {
    for (p, name) in [
        (
            CityParams {
                size: [10.0, 10.0],
                ..params()
            },
            "size",
        ),
        (
            CityParams {
                size: [5000.0, 5000.0],
                ..params()
            },
            "size",
        ),
        (
            CityParams {
                block: 5.0,
                ..params()
            },
            "block",
        ),
        (
            CityParams {
                street_width: 0.5,
                ..params()
            },
            "street_width",
        ),
        (
            CityParams {
                lot_min: 2.0,
                ..params()
            },
            "lot_min",
        ),
        (
            CityParams {
                lot_min: 30.0,
                lot_max: 20.0,
                ..params()
            },
            "lot_max",
        ),
        (
            CityParams {
                irregularity: 2.0,
                ..params()
            },
            "irregularity",
        ),
    ] {
        let error: CityError = layout(&p).expect_err(name);
        assert!(error.to_string().contains(name), "{name}: {error}");
    }
}
