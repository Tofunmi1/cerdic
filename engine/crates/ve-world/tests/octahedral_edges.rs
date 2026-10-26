//! Degenerate inputs, seam identity, and numerically stable round-trip checks.
use ve_world::lod::octahedral::{
    decode, encode, frame_direction, hemi_decode, hemi_encode, select_views,
};

#[test]
fn invalid_directions_use_the_same_frames_as_up() {
    for hemi in [false, true] {
        for dir in [[0.0; 3], [f64::NAN, 1.0, 0.0], [0.0, f64::INFINITY, 0.0]] {
            assert_eq!(
                select_views(dir, 8, hemi),
                select_views([0.0, 1.0, 0.0], 8, hemi)
            );
        }
    }
}

#[test]
fn finite_directions_survive_extreme_magnitudes() {
    for size in [f64::MAX, f64::MIN_POSITIVE, f64::from_bits(1)] {
        let dir = [size, size, -size];
        for (uv, back) in [
            (encode(dir), decode(encode(dir))),
            (hemi_encode(dir), hemi_decode(hemi_encode(dir))),
        ] {
            assert!(uv.iter().all(|v| (0.0..=1.0).contains(v)));
            let expected = 1.0 / 3.0_f64.sqrt();
            for (value, sign) in back.into_iter().zip([1.0, 1.0, -1.0]) {
                assert!((value - expected * sign).abs() < 1e-15);
            }
        }
    }
}

#[test]
fn all_four_copies_of_the_down_pole_keep_their_frame_identity() {
    for col in [0, 8] {
        for row in [0, 8] {
            let dir = frame_direction(col, row, 9, false);
            let views = select_views(dir, 9, false);
            assert!(
                views
                    .iter()
                    .any(|v| v.col == col && v.row == row && v.weight > 1.0 - 1e-14)
            );
        }
    }
}

#[test]
fn small_grids_and_invalid_uvs_have_documented_fallbacks() {
    for grid in [0, 1] {
        for hemi in [false, true] {
            let dir = frame_direction(u32::MAX, u32::MAX, grid, hemi);
            assert!((dir[1] - 1.0).abs() < f64::EPSILON);
            let views = select_views(dir, grid, hemi);
            assert_eq!((views[0].col, views[0].row), (0, 0));
            assert!((views[0].weight - 1.0).abs() < f64::EPSILON);
        }
    }
    for uv in [[f64::NAN, 0.0], [0.0, f64::INFINITY]] {
        assert!((decode(uv)[1] - 1.0).abs() < f64::EPSILON);
        assert!((hemi_decode(uv)[1] - 1.0).abs() < f64::EPSILON);
    }
}

#[test]
fn round_trip_error_is_small_using_a_stable_angle_measure() {
    for i in 0..4000 {
        let y = 1.0 - 2.0 * (f64::from(i) + 0.5) / 4000.0;
        let radius = (1.0 - y * y).sqrt();
        let azimuth = f64::from(i) * 2.399_963_229_728_653;
        let dir = [radius * azimuth.cos(), y, radius * azimuth.sin()];
        for hemi in [false, true] {
            if hemi && y < 0.0 {
                continue;
            }
            let back = if hemi {
                hemi_decode(hemi_encode(dir))
            } else {
                decode(encode(dir))
            };
            let cross = [
                dir[1] * back[2] - dir[2] * back[1],
                dir[2] * back[0] - dir[0] * back[2],
                dir[0] * back[1] - dir[1] * back[0],
            ];
            let sine = cross[0].hypot(cross[1]).hypot(cross[2]);
            let cosine = dir.iter().zip(back).map(|(a, b)| a * b).sum::<f64>();
            assert!(sine.atan2(cosine).abs() < 1e-14, "{dir:?} -> {back:?}");
        }
    }
}
