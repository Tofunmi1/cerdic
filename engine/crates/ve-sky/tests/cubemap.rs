//! Fixed expectations for the procedural environment cubemaps.
#![expect(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::needless_range_loop,
    reason = "fixtures use exact endpoints, small integers and index loops over cube faces"
)]
use std::f32::consts::PI;
use ve_sky::{
    CubeFaces, SkyColors, gradient_cubemap, irradiance_cubemap, prefiltered_mips, sky_radiance,
    texel_direction, texel_solid_angle,
};

const ZENITH: [f32; 3] = [0.1, 0.3, 0.9];
const HORIZON: [f32; 3] = [0.6, 0.7, 0.9];
const GROUND: [f32; 3] = [0.3, 0.2, 0.1];

fn sky() -> SkyColors {
    SkyColors {
        zenith: ZENITH,
        horizon: HORIZON,
        ground: GROUND,
        sun_direction: [0.0, 1.0, 0.0],
        sun_radiance: [0.0; 3],
        sun_angular_radius: 0.05,
    }
}

fn sunny() -> SkyColors {
    SkyColors {
        sun_direction: [1.0, 0.0, 0.0],
        sun_radiance: [50.0, 40.0, 30.0],
        sun_angular_radius: 0.1,
        ..sky()
    }
}

fn uniform(value: [f32; 3]) -> SkyColors {
    SkyColors {
        zenith: value,
        horizon: value,
        ground: value,
        ..sky()
    }
}

fn close(a: [f32; 3], b: [f32; 3], tolerance: f32) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() <= tolerance)
}

fn rgb(texel: [f32; 4]) -> [f32; 3] {
    [texel[0], texel[1], texel[2]]
}

fn texel(faces: &CubeFaces, face: usize, x: u32, y: u32) -> [f32; 3] {
    rgb(faces.faces[face][(y * faces.size + x) as usize])
}

#[test]
fn the_middle_texel_of_each_face_looks_along_its_axis() {
    let axes = [
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
    ];
    for (face, axis) in axes.iter().enumerate() {
        assert!(
            close(texel_direction(face, 3, 1, 1), *axis, 1e-6),
            "face {face}"
        );
    }
}

#[test]
fn texel_directions_are_unit_length() {
    for face in 0..6 {
        for (x, y) in [(0, 0), (3, 1), (7, 7), (2, 6)] {
            let d = texel_direction(face, 8, x, y);
            assert!((d[0] * d[0] + d[1] * d[1] + d[2] * d[2] - 1.0).abs() < 1e-5);
        }
    }
}

#[test]
fn the_top_left_texel_of_plus_x_is_up_and_toward_plus_z() {
    // Size 2: u = v = -0.5, so +X gives (1, 0.5, 0.5) before normalising.
    let n = (1.0_f32 + 0.25 + 0.25).sqrt();
    assert!(close(
        texel_direction(0, 2, 0, 0),
        [1.0 / n, 0.5 / n, 0.5 / n],
        1e-6
    ));
    // -Z top left: (-u, -v, -1) = (0.5, 0.5, -1).
    let m = (0.25_f32 + 0.25 + 1.0).sqrt();
    assert!(close(
        texel_direction(5, 2, 0, 0),
        [0.5 / m, 0.5 / m, -1.0 / m],
        1e-6
    ));
}

#[test]
fn the_texels_of_a_cube_cover_four_pi_steradians() {
    // The midpoint rule converges to 4 pi as the texels shrink: within 2% at 16 and 0.2% at 128.
    for (size, tolerance) in [(16_u32, 0.02), (128, 0.002)] {
        let mut total = 0.0_f64;
        for _ in 0..6 {
            for y in 0..size {
                for x in 0..size {
                    total += f64::from(texel_solid_angle(size, x, y));
                }
            }
        }
        let exact = 4.0 * f64::from(PI);
        assert!(
            (total - exact).abs() < tolerance * exact,
            "size {size}: {total}"
        );
    }
    // The centre texel of a 3 texel face has u = v = 0: 4 / 9.
    assert!((texel_solid_angle(3, 1, 1) - 4.0 / 9.0).abs() < 1e-6);
}

#[test]
fn the_sky_is_zenith_up_ground_down_and_horizon_between() {
    let colors = sky();
    assert!(close(sky_radiance(&colors, [0.0, 1.0, 0.0]), ZENITH, 1e-6));
    assert!(close(sky_radiance(&colors, [0.0, -1.0, 0.0]), GROUND, 1e-6));
    assert!(close(sky_radiance(&colors, [0.0, 0.0, 1.0]), HORIZON, 1e-6));
    // y = 0.25: sqrt = 0.5, halfway from the horizon to the zenith. The sun (straight up, radius 0.05) is far away.
    let halfway = [
        (HORIZON[0] + ZENITH[0]) * 0.5,
        (HORIZON[1] + ZENITH[1]) * 0.5,
        (HORIZON[2] + ZENITH[2]) * 0.5,
    ];
    let direction = [0.968_245_8, 0.25, 0.0];
    assert!(close(sky_radiance(&colors, direction), halfway, 1e-4));
    // y = -0.25: halfway from the horizon to the ground.
    let below = [
        (HORIZON[0] + GROUND[0]) * 0.5,
        (HORIZON[1] + GROUND[1]) * 0.5,
        (HORIZON[2] + GROUND[2]) * 0.5,
    ];
    assert!(close(
        sky_radiance(&colors, [0.968_245_8, -0.25, 0.0]),
        below,
        1e-4
    ));
}

#[test]
fn the_sun_disc_adds_its_radiance_inside_its_radius_only() {
    let colors = sunny();
    let base = sky_radiance(&uniform_free(&colors), [1.0, 0.0, 0.0]);
    let inside = sky_radiance(&colors, [1.0, 0.0, 0.0]);
    assert!(close(
        inside,
        [base[0] + 50.0, base[1] + 40.0, base[2] + 30.0],
        1e-4
    ));
    // 0.05 rad off the axis is inside a radius of 0.1; 0.2 rad off is outside.
    let near = [0.05_f32.cos(), 0.05_f32.sin(), 0.0];
    assert!(sky_radiance(&colors, near)[0] > 40.0);
    let far = [0.2_f32.cos(), 0.2_f32.sin(), 0.0];
    assert!(sky_radiance(&colors, far)[0] < 2.0);
}

/// The same sky with the sun switched off.
fn uniform_free(colors: &SkyColors) -> SkyColors {
    SkyColors {
        sun_radiance: [0.0; 3],
        ..*colors
    }
}

#[test]
fn the_gradient_cubemap_samples_the_sky_at_every_texel_centre() {
    let colors = sunny();
    let cube = gradient_cubemap(&colors, 5);
    assert_eq!(cube.size, 5);
    for face in 0..6 {
        assert_eq!(cube.faces[face].len(), 25);
        for y in 0..5 {
            for x in 0..5 {
                let got = cube.faces[face][(y * 5 + x) as usize];
                assert_eq!(got[3], 1.0);
                let want = sky_radiance(&colors, texel_direction(face, 5, x, y));
                assert!(close(rgb(got), want, 1e-5), "face {face} ({x}, {y})");
            }
        }
    }
}

#[test]
fn a_uniform_sky_stays_uniform_through_irradiance_and_every_mip() {
    let value = [0.25, 0.5, 0.75];
    let cube = gradient_cubemap(&uniform(value), 16);
    let irradiance = irradiance_cubemap(&cube, 8);
    assert_eq!(irradiance.size, 8);
    for face in 0..6 {
        assert_eq!(irradiance.faces[face].len(), 64);
        assert!(
            close(texel(&irradiance, face, 3, 4), value, 0.01 * 0.75),
            "irradiance face {face}"
        );
    }
    let mips = prefiltered_mips(&cube, 5);
    assert_eq!(mips.len(), 5);
    for (i, mip) in mips.iter().enumerate() {
        assert_eq!(mip.size, (16_u32 >> i).max(1), "mip {i}");
        for face in 0..6 {
            assert_eq!(mip.faces[face].len(), (mip.size * mip.size) as usize);
            let middle = mip.size / 2;
            assert!(
                close(texel(mip, face, middle, middle), value, 0.01 * 0.75),
                "mip {i} face {face}"
            );
        }
    }
}

#[test]
fn irradiance_follows_the_gradient_and_the_sun() {
    let cube = gradient_cubemap(&sky(), 32);
    let irradiance = irradiance_cubemap(&cube, 8);
    // Facing up sees mostly zenith and horizon, facing down mostly ground and horizon; blue is strongest overhead.
    let up = texel(&irradiance, 2, 4, 4);
    let down = texel(&irradiance, 3, 4, 4);
    assert!(up[2] > down[2], "up {up:?} down {down:?}");
    assert!(
        down[0] < HORIZON[0],
        "the ground side is darker in red than the horizon"
    );
    // Every value stays inside the range of the source colours.
    for value in [up, down] {
        for c in 0..3 {
            let low = ZENITH[c].min(HORIZON[c]).min(GROUND[c]);
            let high = ZENITH[c].max(HORIZON[c]).max(GROUND[c]);
            assert!(
                value[c] >= low - 1e-3 && value[c] <= high + 1e-3,
                "channel {c}: {value:?}"
            );
        }
    }
    // A sun at +X lights the +X facing texels more than the -X facing ones.
    let sun_cube = gradient_cubemap(&sunny(), 32);
    let sun_irradiance = irradiance_cubemap(&sun_cube, 8);
    let toward = texel(&sun_irradiance, 0, 4, 4)[0];
    let away = texel(&sun_irradiance, 1, 4, 4)[0];
    assert!(toward > away + 0.05, "toward {toward} away {away}");
}

#[test]
fn rougher_mips_spread_the_sun_out() {
    let cube = gradient_cubemap(&sunny(), 32);
    let mips = prefiltered_mips(&cube, 4);
    assert_eq!(mips[0], cube, "mip 0 is the source");
    let peak = |mip: &CubeFaces| {
        mip.faces
            .iter()
            .flatten()
            .map(|t| t[0])
            .fold(0.0_f32, f32::max)
    };
    let peaks: Vec<f32> = mips.iter().map(peak).collect();
    assert!(peaks[0] > 40.0, "{peaks:?}");
    assert!(
        peaks[0] > peaks[1] && peaks[1] > peaks[2] && peaks[2] > peaks[3],
        "{peaks:?}"
    );
    // The roughest mip averages over a whole hemisphere, so no texel is anywhere near the sun's peak.
    assert!(peaks[3] < 5.0, "{peaks:?}");
}

#[test]
fn a_single_texel_cube_has_one_texel_per_face() {
    let mips = prefiltered_mips(&gradient_cubemap(&sky(), 1), 1);
    assert_eq!(mips.len(), 1);
    assert_eq!(mips[0].faces[2].len(), 1);
}
