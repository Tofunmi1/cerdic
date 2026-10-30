//! `ve_render::stochastic`: non-repeating texture sampling (ADR 0070 port 2, the idea of RedGPU's `stochasticSampling.wgsl`, after Heitz and Neyret / Mikkelsen's
//! hex-tiling). The plane is cut into a triangular grid; a point blends three samples of the same texture, each taken at the point shifted by a random offset
//! that belongs to the grid vertex. The CPU function here is the reference the shader must agree with.
#![expect(clippy::cast_precision_loss, reason = "sample counts are small")]
use std::f32::consts::TAU;
use ve_render::stochastic::{stochastic_uvs, vertex_offset};

fn grid(n: usize, extent: f32) -> Vec<[f32; 2]> {
    let mut out = Vec::with_capacity(n * n);
    for i in 0..n {
        for j in 0..n {
            out.push([
                (i as f32 + 0.37) * extent / n as f32,
                (j as f32 + 0.61) * extent / n as f32,
            ]);
        }
    }
    out
}

/// A texture with period 1 in both directions and zero mean.
fn texture(uv: [f32; 2]) -> f32 {
    (TAU * uv[0]).sin() * (TAU * uv[1] * 2.0).cos() + 0.5 * (TAU * (uv[0] + uv[1])).sin()
}

fn blend(uv: [f32; 2], scale: f32, seed: u32) -> f32 {
    stochastic_uvs(uv, scale, seed)
        .iter()
        .map(|s| s.weight * texture(s.uv))
        .sum()
}

#[test]
fn the_three_weights_are_not_negative_and_add_to_one() {
    for p in grid(60, 9.0) {
        let s = stochastic_uvs(p, 3.0, 5);
        assert!(s.iter().all(|x| x.weight >= -1e-6), "{s:?}");
        let sum: f32 = s.iter().map(|x| x.weight).sum();
        assert!((sum - 1.0).abs() < 1e-5, "{sum} at {p:?}");
    }
}

#[test]
fn the_blend_has_no_seams_it_is_continuous_across_triangle_edges() {
    // Walk a fine line across many triangles; neighbouring steps may not jump.
    let step = 1e-3;
    let mut worst = 0.0_f32;
    let mut previous = blend([0.123, 0.456], 4.0, 3);
    let mut x = 0.123;
    while x < 6.0 {
        x += step;
        let value = blend([x, 0.456 + x * 0.31], 4.0, 3);
        worst = worst.max((value - previous).abs());
        previous = value;
    }
    // The texture itself changes by at most about TAU * 3 per unit of uv; the blend with scale 4 may change faster, but never with a jump.
    assert!(worst < 0.2, "largest step {worst}");
}

#[test]
fn the_weights_move_continuously_too() {
    for p in grid(40, 7.0) {
        let a = stochastic_uvs(p, 3.0, 9);
        let b = stochastic_uvs([p[0] + 1e-4, p[1] + 1e-4], 3.0, 9);
        // Match by weight sorted: the set of three weights moves by a tiny amount.
        let mut wa: Vec<f32> = a.iter().map(|s| s.weight).collect();
        let mut wb: Vec<f32> = b.iter().map(|s| s.weight).collect();
        wa.sort_by(f32::total_cmp);
        wb.sort_by(f32::total_cmp);
        for k in 0..3 {
            assert!(
                (wa[k] - wb[k]).abs() < 5e-3,
                "{wa:?} against {wb:?} at {p:?}"
            );
        }
    }
}

#[test]
fn the_offsets_are_deterministic_per_vertex_and_seed_and_inside_the_unit_square() {
    for cell in [[0, 0], [3, -2], [-7, 11], [100, 100]] {
        let a = vertex_offset(cell, 5);
        assert_eq!(a, vertex_offset(cell, 5));
        assert!(
            (0.0..1.0).contains(&a[0]) && (0.0..1.0).contains(&a[1]),
            "{a:?}"
        );
        assert_ne!(a, vertex_offset(cell, 6), "another seed shifts differently");
    }
    assert_ne!(vertex_offset([0, 0], 5), vertex_offset([1, 0], 5));
}

#[test]
fn repetition_is_broken_a_plain_texture_repeats_every_unit_and_the_blend_does_not() {
    let mut plain = 0.0_f32;
    let mut stochastic = 0.0_f32;
    let points = grid(50, 12.0);
    for p in &points {
        plain += (texture(*p) - texture([p[0] + 1.0, p[1]])).abs();
        stochastic += (blend(*p, 1.5, 7) - blend([p[0] + 1.0, p[1]], 1.5, 7)).abs();
    }
    let count = points.len() as f32;
    assert!(
        plain / count < 1e-4,
        "the plain texture repeats exactly: {}",
        plain / count
    );
    assert!(
        stochastic / count > 0.15,
        "the blend should differ one period away: {}",
        stochastic / count
    );
}

#[test]
fn the_mean_is_kept_a_zero_mean_texture_blends_to_about_zero_mean() {
    let points = grid(120, 30.0);
    let mean: f32 = points.iter().map(|p| blend(*p, 2.0, 11)).sum::<f32>() / points.len() as f32;
    assert!(mean.abs() < 0.05, "{mean}");
}

#[test]
fn a_larger_scale_means_smaller_cells_so_the_patches_change_faster() {
    // Along a line the dominant vertex changes more often at a higher scale.
    let changes = |scale: f32| {
        let mut last = None;
        let mut n = 0;
        let mut x = 0.0_f32;
        while x < 10.0 {
            let s = stochastic_uvs([x, 0.3], scale, 1);
            let best = s
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.weight.total_cmp(&b.1.weight))
                .map(|(_, v)| (v.uv[0] - x).to_bits());
            if last.is_some() && last != best {
                n += 1;
            }
            last = best;
            x += 0.01;
        }
        n
    };
    assert!(
        changes(4.0) > changes(1.0) * 2,
        "{} against {}",
        changes(4.0),
        changes(1.0)
    );
}

#[test]
fn bad_numbers_give_finite_samples() {
    for (uv, scale) in [
        ([f32::NAN, 0.0], 1.0),
        ([0.0, f32::INFINITY], 1.0),
        ([1.0, 1.0], f32::NAN),
        ([1.0, 1.0], 0.0),
        ([1.0e9, -1.0e9], 3.0),
    ] {
        let s = stochastic_uvs(uv, scale, 1);
        assert!(
            s.iter()
                .all(|x| x.weight.is_finite() && x.uv[0].is_finite() && x.uv[1].is_finite()),
            "{uv:?} {scale}: {s:?}"
        );
        let sum: f32 = s.iter().map(|x| x.weight).sum();
        assert!((sum - 1.0).abs() < 1e-4, "{sum}");
    }
}

#[test]
fn the_shader_file_has_the_same_function_names() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/shaders/stochastic.wgsl"
    ))
    .unwrap_or_default();
    assert!(
        source.contains("fn stochastic_cells"),
        "missing fn stochastic_cells"
    );
    assert!(
        source.contains("fn vertex_offset"),
        "missing fn vertex_offset"
    );
}
