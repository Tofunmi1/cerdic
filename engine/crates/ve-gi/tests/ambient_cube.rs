//! Fixed expectations for the ambient-cube conversion (the layout is Bevy's irradiance volume format).
#![expect(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    reason = "the fixtures hold small whole and half numbers that are exact in f32"
)]
use ve_gi::{GridSpec, ProbeGrid, ProbeSh, SIDES, ambient_cube, volume_placement, volume_texels};

const MINUS_X: usize = 0;
const PLUS_X: usize = 1;
const MINUS_Y: usize = 2;
const PLUS_Y: usize = 3;
const MINUS_Z: usize = 4;
const PLUS_Z: usize = 5;

fn sh(dc: [f32; 3], x: [f32; 3], y: [f32; 3], z: [f32; 3]) -> ProbeSh {
    ProbeSh { dc, x, y, z }
}

#[test]
fn a_uniform_sky_gives_every_side_the_same_colour() {
    let cube = ambient_cube(&sh([0.2, 0.4, 0.8], [0.0; 3], [0.0; 3], [0.0; 3]));
    assert_eq!(SIDES, 6);
    for side in cube {
        assert_eq!(side, [0.2, 0.4, 0.8]);
    }
}

#[test]
fn each_axis_term_brightens_one_side_and_darkens_the_opposite() {
    let cube = ambient_cube(&sh(
        [1.0, 1.0, 1.0],
        [0.5, 0.0, -0.25],
        [0.0, 0.5, 0.0],
        [-0.5, 0.0, 0.25],
    ));
    assert_eq!(cube[PLUS_X], [1.5, 1.0, 0.75]);
    assert_eq!(cube[MINUS_X], [0.5, 1.0, 1.25]);
    assert_eq!(cube[PLUS_Y], [1.0, 1.5, 1.0]);
    assert_eq!(cube[MINUS_Y], [1.0, 0.5, 1.0]);
    assert_eq!(cube[PLUS_Z], [0.5, 1.0, 1.25]);
    assert_eq!(cube[MINUS_Z], [1.5, 1.0, 0.75]);
}

#[test]
fn a_side_that_would_be_negative_is_zero() {
    let cube = ambient_cube(&sh([0.2, 0.2, 0.2], [0.5, 0.5, 0.5], [0.0; 3], [0.0; 3]));
    assert_eq!(cube[PLUS_X], [0.7, 0.7, 0.7]);
    assert_eq!(
        cube[MINUS_X],
        [0.0, 0.0, 0.0],
        "dc - x = -0.3 is clamped, as the shader clamps the SH"
    );
}

#[test]
fn along_the_axes_the_cube_equals_the_sh_and_between_them_it_stays_close() {
    // Valve's blend: irradiance(n) = sum over axes of side(n_axis sign) * n_axis^2. Along an axis it is the SH exactly; on the
    // diagonal of two axes the difference is (x + y) * (1/sqrt 2 - 1/2), at most 0.21 of the linear terms.
    let probe = sh([1.0; 3], [0.4, 0.0, 0.0], [0.0, 0.3, 0.0], [0.0; 3]);
    let cube = ambient_cube(&probe);
    let blend = |n: [f32; 3], channel: usize| {
        let side = |axis: usize, minus: usize, plus: usize| {
            cube[if n[axis] < 0.0 { minus } else { plus }][channel] * n[axis] * n[axis]
        };
        side(0, MINUS_X, PLUS_X) + side(1, MINUS_Y, PLUS_Y) + side(2, MINUS_Z, PLUS_Z)
    };
    let evaluate = |n: [f32; 3]| {
        (probe.dc[0] + probe.x[0] * n[0] + probe.y[0] * n[1] + probe.z[0] * n[2]).max(0.0)
    };
    assert!((blend([1.0, 0.0, 0.0], 0) - evaluate([1.0, 0.0, 0.0])).abs() < 1e-6);
    assert!((blend([0.0, -1.0, 0.0], 0) - evaluate([0.0, -1.0, 0.0])).abs() < 1e-6);
    let d = std::f32::consts::FRAC_1_SQRT_2;
    let error = (blend([d, d, 0.0], 0) - evaluate([d, d, 0.0])).abs();
    assert!(error <= 0.21 * (0.4 + 0.3) + 1e-6, "diagonal error {error}");
}

/// A grid of `dims` probes whose probe number `n` has `dc = [n, 0, 0]`, `x = [0, 100, 0]` (so +X and -X differ) and zero y and z terms.
fn numbered_grid(dims: [u32; 3]) -> ProbeGrid {
    let count = (dims[0] * dims[1] * dims[2]) as usize;
    let probes = (0..count)
        .map(|n| {
            sh(
                [n as f32 + 1000.0, 0.0, 0.0],
                [0.0, 0.0, 0.0],
                [0.0; 3],
                [0.0; 3],
            )
        })
        .collect();
    ProbeGrid::from_parts(
        GridSpec {
            origin: [0.0; 3],
            spacing: 1.0,
            dims,
        },
        probes,
        vec![true; count],
        [0.0; 3],
    )
}

#[test]
fn the_volume_is_rx_by_2ry_by_3rz_with_one_texel_per_voxel_and_side() {
    let volume = volume_texels(&numbered_grid([2, 3, 4]));
    assert_eq!(volume.size, [2, 6, 12]);
    assert_eq!(volume.texels.len(), 2 * 6 * 12);
    assert!(volume.texels.iter().all(|t| t[3] == 1.0), "alpha is one");
}

#[test]
fn every_voxel_and_side_lands_where_bevys_layout_puts_it() {
    let dims = [2_u32, 3, 4];
    let grid = numbered_grid(dims);
    let volume = volume_texels(&grid);
    let (rx, ry, rz) = (dims[0], dims[1], dims[2]);
    let texel = |s: u32, t: u32, p: u32| volume.texels[(s + rx * (t + 2 * ry * p)) as usize];
    for z in 0..rz {
        for y in 0..ry {
            for x in 0..rx {
                let probe = (x + dims[0] * (y + dims[1] * z)) as usize;
                let expected = ambient_cube(&grid.probes[probe]);
                // s = x; t = y (+ Ry for the - sides: the shader reads the second half of the rows for a negative normal component);
                // p = z (+ Rz for the Y sides, + 2 Rz for the Z sides).
                let at = |side: usize, positive: bool, axis_offset: u32| {
                    let t = y + if positive { 0 } else { ry };
                    let p = z + axis_offset * rz;
                    let got = texel(x, t, p);
                    assert_eq!(
                        [got[0], got[1], got[2]],
                        expected[side],
                        "voxel ({x}, {y}, {z}) side {side}"
                    );
                };
                at(MINUS_X, false, 0);
                at(PLUS_X, true, 0);
                at(MINUS_Y, false, 1);
                at(PLUS_Y, true, 1);
                at(MINUS_Z, false, 2);
                at(PLUS_Z, true, 2);
            }
        }
    }
}

#[test]
fn different_probes_give_different_texels_so_the_order_matters() {
    let volume = volume_texels(&numbered_grid([2, 2, 2]));
    let mut reds: Vec<f32> = volume.texels.iter().map(|t| t[0]).collect();
    reds.sort_by(f32::total_cmp);
    reds.dedup();
    assert_eq!(
        reds.len(),
        8,
        "eight probes, eight distinct colours (their dc is constant over the six sides)"
    );
}

#[test]
fn the_placement_puts_voxel_centres_on_the_probes() {
    let spec = GridSpec {
        origin: [10.0, 0.0, -4.0],
        spacing: 0.5,
        dims: [4, 3, 2],
    };
    let (translation, scale) = volume_placement(&spec);
    assert_eq!(
        translation,
        [10.75, 0.5, -3.75],
        "origin + spacing * (dims - 1) / 2"
    );
    assert_eq!(scale, [2.0, 1.5, 1.0], "spacing * dims");
    // Voxel i of R sits at (i + 0.5) / R - 0.5 in the unit cube: that must be the probe's world position.
    for (axis, &r) in spec.dims.iter().enumerate() {
        for i in 0..r {
            let unit = (i as f32 + 0.5) / r as f32 - 0.5;
            let world = translation[axis] + unit * scale[axis];
            let probe = spec.origin[axis] + spec.spacing * i as f32;
            assert!(
                (world - probe).abs() < 1e-5,
                "axis {axis} voxel {i}: {world} against {probe}"
            );
        }
    }
}

#[test]
fn a_one_probe_grid_is_a_one_voxel_volume() {
    let volume = volume_texels(&numbered_grid([1, 1, 1]));
    assert_eq!(volume.size, [1, 2, 3]);
    assert_eq!(volume.texels.len(), 6);
}
