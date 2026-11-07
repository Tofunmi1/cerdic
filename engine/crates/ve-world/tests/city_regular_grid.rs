//! Irregularity 0 means what the node's doc says, a square (here: perfectly regular) grid: the streets are evenly spaced, as Manhattan's are. Any irregularity above 0
//! keeps the jittered streets it always had, so no existing district moves. Written before the fix; do not weaken.
use ve_world::city::{CityParams, layout};

fn distinct(values: impl Iterator<Item = f32>) -> Vec<f32> {
    let mut v: Vec<f32> = values.collect();
    v.sort_by(f32::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
    v
}

fn gaps(v: &[f32]) -> Vec<f32> {
    v.windows(2).map(|w| w[1] - w[0]).collect()
}

fn params(irregularity: f32) -> CityParams {
    CityParams {
        size: [240.0, 240.0],
        block: 40.0,
        irregularity,
        ..CityParams::default()
    }
}

#[test]
fn zero_irregularity_spaces_the_streets_evenly() {
    let l = layout(&params(0.0)).unwrap();
    for axis in 0..2 {
        let lines = distinct(l.street_nodes.iter().map(|n| n[axis]));
        let g = gaps(&lines);
        let first = g[0];
        assert!(
            g.iter().all(|x| (x - first).abs() < 1e-2),
            "axis {axis} is not evenly spaced: {g:?}"
        );
    }
}

#[test]
fn some_irregularity_still_jitters_the_streets() {
    let l = layout(&params(0.5)).unwrap();
    let lines = distinct(l.street_nodes.iter().map(|n| n[0]));
    let g = gaps(&lines);
    let spread =
        g.iter().cloned().fold(f32::MIN, f32::max) - g.iter().cloned().fold(f32::MAX, f32::min);
    assert!(spread > 0.5, "the spacing varies: {g:?}");
}
