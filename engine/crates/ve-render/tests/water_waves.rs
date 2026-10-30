//! `ve_render::water_waves`: the wave field of procedural water (ADR 0070 port 3, the idea of RedGPU's lake with its two normal-map bitmaps replaced by
//! maths). A sum of travelling sine waves with deep-water dispersion; the shader reads the same table the CPU builds, so the two agree by construction.
#![expect(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    reason = "fixtures; sample counts are small"
)]
use std::f32::consts::PI;
use ve_render::water_waves::{GRAVITY, WAVES, Wave, WaveField, wave_table};

fn field(ripple: f32) -> WaveField {
    WaveField::new(7, ripple, 1.0, 0.0)
}

/// Points spread over a few hundred metres and some seconds.
fn samples(count: usize) -> Vec<(f32, f32, f32)> {
    (0..count)
        .map(|i| {
            let f = i as f32;
            (
                (f * 12.9898).sin() * 80.0,
                (f * 78.233).cos() * 80.0,
                (f * 0.37).fract() * 20.0,
            )
        })
        .collect()
}

#[test]
fn no_ripple_is_a_flat_sheet() {
    let f = field(0.0);
    for (x, z, t) in samples(50) {
        assert!(f.height(x, z, t).abs() < 1e-9);
        let n = f.normal(x, z, t);
        assert!(
            (n[0].abs() + n[2].abs()) < 1e-6 && (n[1] - 1.0).abs() < 1e-6,
            "{n:?}"
        );
    }
}

#[test]
fn the_normal_is_the_gradient_of_the_height() {
    let f = field(1.0);
    let e = 1e-3;
    for (x, z, t) in samples(200) {
        let dx = (f.height(x + e, z, t) - f.height(x - e, z, t)) / (2.0 * e);
        let dz = (f.height(x, z + e, t) - f.height(x, z - e, t)) / (2.0 * e);
        let len = (dx * dx + 1.0 + dz * dz).sqrt();
        let expected = [-dx / len, 1.0 / len, -dz / len];
        let n = f.normal(x, z, t);
        for k in 0..3 {
            assert!(
                (n[k] - expected[k]).abs() < 3e-3,
                "at ({x},{z},{t}) component {k}: {n:?} against {expected:?}"
            );
        }
        let unit = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
        assert!((unit - 1.0).abs() < 1e-4, "{unit}");
    }
}

#[test]
fn the_height_stays_inside_its_bound_and_the_bound_scales_with_ripple() {
    let full = field(1.0);
    let half = field(0.5);
    assert!(
        full.max_height() > 0.01 && full.max_height() < 0.15,
        "{}",
        full.max_height()
    );
    assert!((half.max_height() - full.max_height() * 0.5).abs() < 1e-6);
    for (x, z, t) in samples(500) {
        assert!(full.height(x, z, t).abs() <= full.max_height() + 1e-6);
    }
}

#[test]
fn the_surface_is_visibly_wavy_but_never_folds_over() {
    let f = field(1.0);
    let mut steepest = 0.0_f32;
    for (x, z, t) in samples(1000) {
        let n = f.normal(x, z, t);
        let slope = (n[0] * n[0] + n[2] * n[2]).sqrt() / n[1];
        steepest = steepest.max(slope);
    }
    assert!(steepest > 0.1, "too calm: steepest slope {steepest}");
    assert!(steepest < 0.9, "folds over: steepest slope {steepest}");
}

#[test]
fn the_same_seed_gives_the_same_field_and_another_seed_a_different_one() {
    let a = WaveField::new(7, 1.0, 1.0, 0.0);
    let b = WaveField::new(7, 1.0, 1.0, 0.0);
    let c = WaveField::new(8, 1.0, 1.0, 0.0);
    let mut differs = false;
    for (x, z, t) in samples(100) {
        assert_eq!(a.height(x, z, t).to_bits(), b.height(x, z, t).to_bits());
        differs |= (a.height(x, z, t) - c.height(x, z, t)).abs() > 1e-4;
    }
    assert!(differs);
}

#[test]
fn the_table_has_the_waves_the_shader_loops_over() {
    let table = wave_table(7);
    assert_eq!(table.len(), WAVES);
    assert!((8..=16).contains(&WAVES), "{WAVES}");
    for w in &table {
        assert!((0.3..=12.0).contains(&w.wavelength), "{w:?}");
        assert!(w.amplitude > 0.0 && w.amplitude.is_finite());
    }
    let shortest = table.iter().map(|w| w.wavelength).fold(f32::MAX, f32::min);
    let longest = table.iter().map(|w| w.wavelength).fold(0.0, f32::max);
    assert!(
        longest / shortest > 8.0,
        "a spread of sizes: {shortest} to {longest}"
    );
}

#[test]
fn every_wave_runs_within_seventy_degrees_of_the_main_direction() {
    for direction in [0.0_f32, 90.0, -135.0] {
        for w in wave_table_with_direction(direction) {
            let angle = w.direction_deg - direction;
            let wrapped = (angle + 180.0).rem_euclid(360.0) - 180.0;
            assert!(wrapped.abs() <= 70.0, "{direction}: {w:?}");
        }
    }
}

fn wave_table_with_direction(direction: f32) -> Vec<Wave> {
    WaveField::new(7, 1.0, 1.0, direction).waves().to_vec()
}

#[test]
fn a_single_wave_travels_at_its_deep_water_phase_speed() {
    let wave = Wave {
        direction_deg: 0.0,
        wavelength: 4.0,
        amplitude: 0.05,
        phase: 0.3,
    };
    let f = WaveField::from_waves(&[wave], 1.0);
    let k = 2.0 * PI / wave.wavelength;
    let speed = (GRAVITY / k).sqrt();
    let dt = 0.7;
    for (x, z, t) in samples(40) {
        let moved = f.height(x + speed * dt, z, t + dt);
        assert!((moved - f.height(x, z, t)).abs() < 1e-4, "{x} {z} {t}");
    }
}

#[test]
fn longer_waves_are_faster_by_the_square_root_of_the_ratio_and_flow_scales_speed() {
    let long = Wave {
        direction_deg: 0.0,
        wavelength: 8.0,
        amplitude: 0.05,
        phase: 0.0,
    };
    let short = Wave {
        direction_deg: 0.0,
        wavelength: 2.0,
        amplitude: 0.05,
        phase: 0.0,
    };
    let speed = |w: Wave, flow: f32| {
        let f = WaveField::from_waves(&[w], flow);
        // The crest at x = 0, t = 0 (phase 0 puts a zero crossing there); find how far the field moves in dt by matching the height.
        let dt = 0.1;
        let mut best = (f32::MAX, 0.0);
        // Search only half a wavelength: beyond that a crest one or more wavelengths away matches just as well.
        for i in 0..((w.wavelength / 2.0) / 0.002) as usize {
            let d = i as f32 * 0.002;
            let err = (f.height(d, 0.0, dt) - f.height(0.0, 0.0, 0.0)).abs()
                + (f.height(d + 0.01, 0.0, dt) - f.height(0.01, 0.0, 0.0)).abs();
            if err < best.0 {
                best = (err, d);
            }
        }
        best.1 / dt
    };
    let ratio = speed(long, 1.0) / speed(short, 1.0);
    assert!((ratio - 2.0).abs() < 0.1, "{ratio}");
    let doubled = speed(long, 2.0) / speed(long, 1.0);
    assert!((doubled - 2.0).abs() < 0.1, "{doubled}");
}

#[test]
fn flow_zero_freezes_the_sheet() {
    let f = WaveField::new(7, 1.0, 0.0, 0.0);
    for (x, z, _) in samples(30) {
        assert!((f.height(x, z, 0.0) - f.height(x, z, 9.0)).abs() < 1e-6);
    }
}

#[test]
fn bad_numbers_are_clamped_not_propagated() {
    let f = WaveField::new(7, f32::NAN, f32::INFINITY, f32::NAN);
    assert!(f.height(1.0, 2.0, 3.0).is_finite());
    assert!(f.normal(1.0, 2.0, 3.0).iter().all(|v| v.is_finite()));
    let big = WaveField::new(7, 1000.0, 1000.0, 0.0);
    assert!(
        big.max_height() <= WaveField::new(7, 2.0, 1.0, 0.0).max_height() + 1e-6,
        "ripple is clamped to 2"
    );
}

#[test]
fn the_shader_file_declares_the_same_wave_count_and_the_entry_helpers() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/shaders/water_waves.wgsl"
    ))
    .unwrap();
    assert!(
        source.contains(&format!("const WAVE_COUNT: u32 = {WAVES}u;")),
        "WAVE_COUNT must equal WAVES"
    );
    assert!(
        source.contains("fn water_wave_height"),
        "missing fn water_wave_height"
    );
    assert!(
        source.contains("fn water_wave_normal"),
        "missing fn water_wave_normal"
    );
}
