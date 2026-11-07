//! Long thin blocks (Manhattan's are about 80 by 270 m): `block` is the spacing between streets along X and `block_depth` the spacing along Z, 0 meaning
//! the same as `block`, so every existing district is unchanged. Written before the code; do not weaken.
use std::collections::BTreeSet;
use ve_world::city::{CityError, CityParams, layout};

fn params(depth: f32) -> CityParams {
    CityParams {
        size: [240.0, 240.0],
        block: 40.0,
        block_depth: depth,
        ..CityParams::default()
    }
}

fn blocks(p: &CityParams) -> usize {
    layout(p)
        .unwrap()
        .lots
        .iter()
        .map(|lot| lot.block)
        .collect::<BTreeSet<_>>()
        .len()
}

#[test]
fn a_deeper_block_depth_makes_fewer_longer_blocks() {
    let square = blocks(&params(0.0));
    let long = blocks(&params(120.0));
    assert!(
        long * 2 <= square,
        "{long} long blocks against {square} square ones"
    );
}

#[test]
fn depth_zero_is_the_square_grid() {
    assert_eq!(
        layout(&params(0.0)).unwrap(),
        layout(&params(40.0)).unwrap()
    );
}

#[test]
fn the_long_blocks_still_fill_the_district_with_lots() {
    let l = layout(&params(120.0)).unwrap();
    assert!(l.lots.len() > 20, "{} lots", l.lots.len());
    assert!(
        l.lots
            .iter()
            .flat_map(|lot| lot.footprint.iter())
            .all(|c| c[0].abs() <= 120.5 && c[1].abs() <= 120.5),
        "inside the district"
    );
}

#[test]
fn a_block_depth_that_is_neither_zero_nor_sensible_is_refused_by_name() {
    for bad in [-5.0, 3.0, 900.0] {
        match layout(&params(bad)) {
            Err(CityError::InvalidParameter { name, .. }) => assert_eq!(name, "block_depth"),
            other => panic!("block_depth {bad} was accepted: {other:?}"),
        }
    }
}
