//! Fixed numbers for the atmosphere reference, from an independent integration (200,000 midpoint steps in Python), so the
//! implementation is checked against more than itself.
#![expect(
    clippy::float_cmp,
    reason = "two checks compare constants and a zero result that are exact by construction"
)]
use ve_sky::Atmosphere;

const GROUND: f64 = 6_360_000.0;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1.0e-3 + 0.01 * b.abs()
}

fn assert_close(label: &str, got: [f64; 3], want: [f64; 3]) {
    for channel in 0..3 {
        assert!(
            close(got[channel], want[channel]),
            "{label}: channel {channel} is {} and should be about {}",
            got[channel],
            want[channel]
        );
    }
}

#[test]
fn the_earth_constants_are_the_documented_ones() {
    let earth = Atmosphere::earth();
    assert!(
        (earth.bottom_radius - 6_360_000.0).abs() < 1e-6
            && (earth.top_radius - 6_460_000.0).abs() < 1e-6
    );
    assert_eq!(earth.rayleigh_scattering, [5.802e-6, 13.558e-6, 33.1e-6]);
    assert!((earth.mie_asymmetry - 0.8).abs() < 1e-12);
}

#[test]
fn extinction_at_sea_level_adds_rayleigh_mie_and_no_ozone() {
    let at_ground = Atmosphere::earth().extinction(0.0);
    let mie = 3.996e-6 + 4.44e-6;
    assert_close(
        "sea level",
        at_ground,
        [5.802e-6 + mie, 13.558e-6 + mie, 33.1e-6 + mie],
    );
    // At 25 km the ozone layer peaks and Rayleigh has thinned by e^(25/8); Mie is almost gone (e^(-25000/1200)).
    let high = Atmosphere::earth().extinction(25_000.0);
    let rayleigh = (-25_000.0_f64 / 8_000.0).exp();
    assert!(
        (high[0] - (5.802e-6 * rayleigh + 0.65e-6)).abs() < 1e-8,
        "red at 25 km: {}",
        high[0]
    );
    assert!(
        (high[1] - (13.558e-6 * rayleigh + 1.881e-6)).abs() < 1e-8,
        "green at 25 km: {}",
        high[1]
    );
    // Above the top of the ozone layer (45 km) it contributes nothing.
    let above = Atmosphere::earth().extinction(45_000.0);
    assert!((above[1] - 13.558e-6 * (-45_000.0_f64 / 8_000.0).exp()).abs() < 1e-9);
}

#[test]
fn distances_to_the_top_and_the_ground_follow_the_geometry() {
    let earth = Atmosphere::earth();
    assert!(
        (earth.distance_to_top(GROUND, 1.0) - 100_000.0).abs() < 1.0,
        "straight up from the ground is 100 km"
    );
    assert!(
        (earth.distance_to_top(GROUND, 0.0) - 1_132_254.388).abs() < 5.0,
        "along the horizon"
    );
    assert!(
        earth.distance_to_ground(GROUND, 1.0).is_none(),
        "up never meets the ground"
    );
    assert!(
        earth.distance_to_ground(GROUND, 0.0).is_none(),
        "a grazing ray from the ground goes away from it"
    );
    let down = earth
        .distance_to_ground(GROUND + 10_000.0, -0.3)
        .unwrap_or(f64::NAN);
    assert!(
        (down - 33_602.599).abs() < 2.0,
        "from 10 km down at mu = -0.3: {down}"
    );
    // A ray that passes the planet entirely: from 100 km up, nearly horizontal and aimed away.
    assert!(
        earth
            .distance_to_ground(GROUND + 100_000.0, -0.05)
            .is_none()
    );
}

#[test]
fn transmittance_to_the_top_matches_the_reference_integration() {
    let earth = Atmosphere::earth();
    assert_close(
        "zenith from the ground",
        earth.transmittance_to_top(GROUND, 1.0),
        [0.935_860, 0.863_465, 0.758_663],
    );
    assert_close(
        "mu 0.5 from the ground",
        earth.transmittance_to_top(GROUND, 0.5),
        [0.876_343, 0.746_667, 0.576_728],
    );
    assert_close(
        "the horizon from the ground",
        earth.transmittance_to_top(GROUND, 0.0),
        [0.068_712, 0.006_185, 0.000_034],
    );
    assert_close(
        "mu 0.1 from 1 km",
        earth.transmittance_to_top(GROUND + 1_000.0, 0.1),
        [0.613_565, 0.324_652, 0.114_201],
    );
}

#[test]
fn transmittance_between_two_points_matches_the_reference_too() {
    let earth = Atmosphere::earth();
    let down = earth.transmittance(GROUND + 10_000.0, -0.3, 33_602.599);
    assert_close(
        "from 10 km down to the ground",
        down,
        [0.864_430, 0.744_761, 0.511_643],
    );
    assert_close(
        "optical depth of the first km straight up",
        earth.optical_depth(GROUND, 1.0, 1_000.0),
        [0.011_178, 0.018_469, 0.036_838],
    );
    // exp(-optical depth) is the transmittance.
    let od = earth.optical_depth(GROUND, 0.5, 40_000.0);
    let t = earth.transmittance(GROUND, 0.5, 40_000.0);
    for channel in 0..3 {
        assert!((t[channel] - (-od[channel]).exp()).abs() < 1e-9);
    }
}

#[test]
fn transmittance_is_a_share_that_only_falls_with_distance_and_blue_falls_fastest() {
    let earth = Atmosphere::earth();
    let mut last = [1.0_f64; 3];
    for km in [0.0, 1.0, 5.0, 20.0, 60.0, 150.0, 400.0] {
        let t = earth.transmittance(GROUND, 0.05, km * 1_000.0);
        for channel in 0..3 {
            assert!(
                (0.0..=1.0).contains(&t[channel]),
                "{km} km channel {channel}: {}",
                t[channel]
            );
            assert!(
                t[channel] <= last[channel] + 1e-12,
                "it must not rise with distance"
            );
        }
        assert!(
            t[0] >= t[1] && t[1] >= t[2],
            "red passes more than green, green more than blue at {km} km"
        );
        last = t;
    }
    assert!(
        (earth.transmittance(GROUND, 0.5, 0.0)[1] - 1.0).abs() < 1e-12,
        "no distance, no loss"
    );
}

fn up() -> [f64; 3] {
    [0.0, 1.0, 0.0]
}

fn direction(elevation_degrees: f64, azimuth_degrees: f64) -> [f64; 3] {
    let (e, a) = (elevation_degrees.to_radians(), azimuth_degrees.to_radians());
    [e.cos() * a.sin(), e.sin(), -e.cos() * a.cos()]
}

fn sun_light() -> [f64; 3] {
    [1.0, 1.0, 1.0]
}

fn sky(sun_elevation: f64, view_elevation: f64, view_azimuth: f64) -> [f64; 3] {
    Atmosphere::earth().single_scattering(
        GROUND + 1.0,
        direction(view_elevation, view_azimuth),
        direction(sun_elevation, 0.0),
        sun_light(),
    )
}

#[test]
fn a_clear_noon_sky_is_blue() {
    // Sun high, looking 60 degrees up and well away from the sun.
    let c = sky(60.0, 60.0, 180.0);
    assert!(c.iter().all(|v| v.is_finite() && *v > 0.0), "{c:?}");
    assert!(
        c[2] > c[1] && c[1] > c[0],
        "blue over green over red: {c:?}"
    );
}

#[test]
fn looking_toward_a_low_sun_is_red_and_away_from_it_is_dim_and_blue() {
    let toward = sky(2.0, 5.0, 0.0);
    let away = sky(2.0, 30.0, 180.0);
    assert!(
        toward[0] > toward[2],
        "toward a sun at 2 degrees red leads blue: {toward:?}"
    );
    assert!(
        toward[0] > away[0],
        "the sky next to the sun is brighter than the opposite sky"
    );
    assert!(
        away[2] > away[0],
        "the opposite sky is still bluer than red: {away:?}"
    );
}

#[test]
fn a_sun_below_the_horizon_leaves_a_dark_sky() {
    let noon = sky(60.0, 60.0, 180.0);
    let night = sky(-12.0, 60.0, 180.0);
    for channel in 0..3 {
        assert!(
            night[channel] < 0.02 * noon[channel],
            "channel {channel}: night {} against noon {}",
            night[channel],
            noon[channel]
        );
    }
}

#[test]
fn scattered_light_is_linear_in_the_sun_and_zero_without_it() {
    let earth = Atmosphere::earth();
    let (view, sun) = (direction(40.0, 70.0), direction(35.0, 0.0));
    let one = earth.single_scattering(GROUND + 100.0, view, sun, [1.0, 2.0, 3.0]);
    let two = earth.single_scattering(GROUND + 100.0, view, sun, [2.0, 4.0, 6.0]);
    for channel in 0..3 {
        assert!(
            one[channel] > 0.0
                && (two[channel] - 2.0 * one[channel]).abs() <= 1e-9 * one[channel].abs().max(1.0)
        );
    }
    assert_eq!(
        earth.single_scattering(GROUND + 100.0, view, sun, [0.0; 3]),
        [0.0; 3]
    );
}

#[test]
fn looking_down_at_the_ground_sees_only_the_air_between_and_the_view_is_symmetric_about_the_sun() {
    let earth = Atmosphere::earth();
    // A viewer on a 10 km mountain looking steeply down: finite, non-negative, and less than looking at the horizon.
    let down = earth.single_scattering(
        GROUND + 10_000.0,
        direction(-60.0, 0.0),
        direction(45.0, 90.0),
        sun_light(),
    );
    let horizon = earth.single_scattering(
        GROUND + 10_000.0,
        direction(0.0, 0.0),
        direction(45.0, 90.0),
        sun_light(),
    );
    assert!(down.iter().all(|v| v.is_finite() && *v >= 0.0));
    assert!(
        down[2] < horizon[2],
        "less air below than along the horizon: {down:?} against {horizon:?}"
    );
    // Mirroring the view about the plane that holds the sun and the vertical changes nothing.
    let left = earth.single_scattering(
        GROUND + 5.0,
        direction(30.0, -50.0),
        direction(40.0, 0.0),
        sun_light(),
    );
    let right = earth.single_scattering(
        GROUND + 5.0,
        direction(30.0, 50.0),
        direction(40.0, 0.0),
        sun_light(),
    );
    for channel in 0..3 {
        assert!((left[channel] - right[channel]).abs() <= 1e-9 * left[channel].abs().max(1e-12));
    }
    assert!((up()[1] - 1.0).abs() < 1e-12);
}
