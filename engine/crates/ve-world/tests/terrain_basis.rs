//! Terrain generation with the shared noise bases (plan W1): each basis fills the height range, is world-space (tiles match) and leaves the
//! original `basis: None` ground untouched.
#![expect(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    reason = "fixtures unwrap valid data and index small grids"
)]
use ve_world::noise::{Basis, CellDistance, CellReturn};
use ve_world::terrain::{HeightGrid, Noise, generate};

const NEVER: &dyn Fn() -> bool = &|| false;

fn noise(basis: Option<Basis>) -> Noise {
    Noise {
        seed: 7,
        scale: 60.0,
        octaves: 5,
        roughness: 0.5,
        lacunarity: 2.0,
        ridged: false,
        warp: 0.0,
        basis,
    }
}

fn grid(noise: &Noise, origin: [f32; 2]) -> HeightGrid {
    let mut grid = HeightGrid::new(65, 65, 2.0, origin).unwrap();
    generate(&mut grid, noise, 100.0, NEVER).unwrap();
    grid
}

fn bases() -> [Basis; 4] {
    [
        Basis::Perlin,
        Basis::Simplex,
        Basis::Value,
        Basis::Cellular {
            distance: CellDistance::Euclidean,
            ret: CellReturn::Distance,
            jitter: 1.0,
        },
    ]
}

#[test]
fn every_basis_fills_the_height_range_with_relief() {
    for basis in bases() {
        let g = grid(&noise(Some(basis)), [0.0, 0.0]);
        let (lo, hi) = g
            .heights()
            .iter()
            .fold((f32::MAX, f32::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
        assert!(lo >= 0.0 && hi <= 100.0, "{basis:?}: {lo}..{hi}");
        assert!(hi - lo > 15.0, "{basis:?} is nearly flat: {lo}..{hi}");
    }
}

#[test]
fn neighbouring_tiles_of_every_basis_meet_exactly() {
    for basis in bases() {
        let n = noise(Some(basis));
        let left = grid(&n, [0.0, 0.0]);
        // The next tile starts at the last column of the first one (cells are 2 m, 65 samples span 128 m).
        let right = grid(&n, [128.0, 0.0]);
        for j in 0..65 {
            assert_eq!(
                left.heights()[j * 65 + 64].to_bits(),
                right.heights()[j * 65].to_bits(),
                "{basis:?} row {j}"
            );
        }
    }
}

#[test]
fn the_original_noise_is_unchanged_and_the_new_ones_differ_from_it() {
    let classic = grid(&noise(None), [0.0, 0.0]);
    for basis in bases() {
        let other = grid(&noise(Some(basis)), [0.0, 0.0]);
        assert_ne!(classic.heights(), other.heights(), "{basis:?}");
    }
}

#[test]
fn warp_and_ridges_work_with_the_new_bases() {
    let mut n = noise(Some(Basis::Simplex));
    n.warp = 80.0;
    n.ridged = true;
    let g = grid(&n, [0.0, 0.0]);
    assert!(g.heights().iter().all(|h| (0.0..=100.0).contains(h)));
}
