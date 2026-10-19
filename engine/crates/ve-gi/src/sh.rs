//! First-order spherical harmonics for the probes: the sky's radiance, projecting radiance samples onto a probe, evaluating a
//! probe for a normal, and filling probes that sit inside solids.

use crate::{GridSpec, PI, ProbeSh, Sky};

/// The sky's radiance looking along the unit direction `d`: linear in its y between the horizon and the zenith above it, and
/// between the horizon and the ground below it.
pub(crate) fn sky_radiance(s: Sky, d: [f32; 3]) -> [f32; 3] {
    if d[1] >= 0.0 {
        mix(s.horizon, s.zenith, d[1])
    } else {
        mix(s.horizon, s.ground, -d[1])
    }
}

/// The sky's average radiance over the whole sphere, the answer for a lookup that has no usable probe.
pub(crate) fn sky_irradiance(s: Sky) -> [f32; 3] {
    // Each hemisphere averages its end colour over half its height range: zenith / 4 and ground / 4 with the horizon's half.
    std::array::from_fn(|c| 0.5 * s.horizon[c] + 0.25 * (s.zenith[c] + s.ground[c]))
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] * (1.0 - t) + b[i] * t)
}

/// Projects radiance samples `(unit direction, rgb radiance)`, taken evenly over the sphere, onto the probe's irradiance-over-pi
/// coefficients. With radiance SH `L00` (times `Y0`) and `L1m` (times `Y1` and the direction's component), the cosine lobe
/// scales the constant term by 1 (pi over pi) and the linear terms by 2/3 (2 pi / 3 over pi): uniform radiance L gives `dc = L`.
#[expect(
    clippy::cast_precision_loss,
    reason = "a sample count is far below 2^23"
)]
pub(crate) fn project(samples: &[([f32; 3], [f32; 3])]) -> ProbeSh {
    const Y0: f32 = 0.282_095;
    const Y1: f32 = 0.488_603;
    let weight = 4.0 * PI / samples.len().max(1) as f32;
    let (constant, linear) = (Y0 * Y0, Y1 * Y1 * 2.0 / 3.0);
    let mut sh = ProbeSh::default();
    for (direction, radiance) in samples {
        for (channel, value) in radiance.iter().enumerate() {
            let l = value * weight;
            sh.dc[channel] += l * constant;
            sh.x[channel] += l * direction[0] * linear;
            sh.y[channel] += l * direction[1] * linear;
            sh.z[channel] += l * direction[2] * linear;
        }
    }
    sh
}

/// The irradiance over pi a probe gives a surface facing `n`, never below zero.
pub(crate) fn eval(p: ProbeSh, n: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| (p.dc[i] + p.x[i] * n[0] + p.y[i] * n[1] + p.z[i] * n[2]).max(0.0))
}

/// Fills each invalid probe from its known face neighbours (the average of them), spreading inward one cell per round for up to
/// four rounds so a probe deep inside a solid still gets light from the nearest open air. A probe that reaches no known
/// neighbour is set to `open_sky`.
pub(crate) fn fix_invalid(
    probes: &mut [ProbeSh],
    valid: &[bool],
    spec: GridSpec,
    open_sky: ProbeSh,
) {
    const FACES: [(isize, isize, isize); 6] = [
        (-1, 0, 0),
        (1, 0, 0),
        (0, -1, 0),
        (0, 1, 0),
        (0, 0, -1),
        (0, 0, 1),
    ];
    let [nx, ny, nz] = spec.dims.map(|d| d as usize);
    let mut known = valid.to_vec();
    for _ in 0..4 {
        let before = probes.to_vec();
        let mut learned = Vec::new();
        for index in (0..probes.len()).filter(|&i| !known[i]) {
            let (x, y, z) = (index % nx, index / nx % ny, index / (nx * ny));
            let mut sum = ProbeSh::default();
            let mut count = 0.0;
            for (dx, dy, dz) in FACES {
                let at = (
                    x.checked_add_signed(dx),
                    y.checked_add_signed(dy),
                    z.checked_add_signed(dz),
                );
                if let (Some(a), Some(b), Some(c)) = at
                    && a < nx
                    && b < ny
                    && c < nz
                {
                    let neighbour = a + nx * (b + ny * c);
                    if known[neighbour] {
                        add_sh(&mut sum, before[neighbour]);
                        count += 1.0;
                    }
                }
            }
            if count > 0.0 {
                scale_sh(&mut sum, 1.0 / count);
                probes[index] = sum;
                learned.push(index);
            }
        }
        if learned.is_empty() {
            break;
        }
        for index in learned {
            known[index] = true;
        }
    }
    for index in (0..probes.len()).filter(|&i| !known[i]) {
        probes[index] = open_sky;
    }
}

fn add_sh(a: &mut ProbeSh, b: ProbeSh) {
    for i in 0..3 {
        a.dc[i] += b.dc[i];
        a.x[i] += b.x[i];
        a.y[i] += b.y[i];
        a.z[i] += b.z[i];
    }
}

fn scale_sh(a: &mut ProbeSh, s: f32) {
    for i in 0..3 {
        a.dc[i] *= s;
        a.x[i] *= s;
        a.y[i] *= s;
        a.z[i] *= s;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn red(dc: f32) -> ProbeSh {
        ProbeSh {
            dc: [dc, 0.0, 0.0],
            ..ProbeSh::default()
        }
    }

    fn row(count: u32) -> GridSpec {
        GridSpec {
            origin: [0.0; 3],
            spacing: 1.0,
            dims: [count, 1, 1],
        }
    }

    #[test]
    fn invalid_probes_inherit_from_known_ones_spreading_inward_a_cell_per_round() {
        // Two valid ends (1 and 3) around three invalid probes: the first round fills the ones next to a valid probe, the second
        // the middle one from those two.
        let mut probes = vec![red(1.0), red(9.0), red(9.0), red(9.0), red(3.0)];
        fix_invalid(
            &mut probes,
            &[true, false, false, false, true],
            row(5),
            red(0.0),
        );
        let values: Vec<f32> = probes.iter().map(|p| p.dc[0]).collect();
        assert_eq!(values, [1.0, 1.0, 2.0, 3.0, 3.0]);
    }

    #[test]
    fn valid_probes_are_left_alone() {
        let mut probes = vec![red(1.0), red(2.0), red(3.0)];
        fix_invalid(&mut probes, &[true; 3], row(3), red(0.0));
        assert_eq!(probes, [red(1.0), red(2.0), red(3.0)]);
    }

    #[test]
    fn a_probe_that_reaches_no_known_neighbour_takes_the_open_sky() {
        // Eight invalid probes in a row with the only valid one seven cells away: four rounds reach four cells, so the far
        // ones get the open sky, whatever garbage their own trace produced.
        let mut probes = vec![red(7.0); 8];
        probes[0] = red(1.0);
        let mut valid = vec![false; 8];
        valid[0] = true;
        fix_invalid(&mut probes, &valid, row(8), red(0.25));
        let values: Vec<f32> = probes.iter().map(|p| p.dc[0]).collect();
        assert_eq!(values, [1.0, 1.0, 1.0, 1.0, 1.0, 0.25, 0.25, 0.25]);
    }

    #[test]
    fn a_uniform_sky_projects_to_its_own_colour_and_no_direction() {
        let dirs = crate::baker::sphere_directions(256);
        let samples: Vec<_> = dirs.iter().map(|d| (*d, [0.3, 0.6, 0.9])).collect();
        let sh = project(&samples);
        for (channel, expected) in [0.3, 0.6, 0.9].into_iter().enumerate() {
            assert!((sh.dc[channel] - expected).abs() < 0.005 * expected);
            assert!(
                sh.x[channel].abs() < 0.005
                    && sh.y[channel].abs() < 0.005
                    && sh.z[channel].abs() < 0.005
            );
        }
    }

    #[test]
    fn the_sky_is_linear_in_height_on_each_side_of_the_horizon() {
        let sky = Sky {
            zenith: [1.0; 3],
            horizon: [0.5; 3],
            ground: [0.0; 3],
        };
        assert!((sky_radiance(sky, [0.0, 1.0, 0.0])[0] - 1.0).abs() < 1e-6);
        assert!((sky_radiance(sky, [1.0, 0.0, 0.0])[0] - 0.5).abs() < 1e-6);
        assert!(sky_radiance(sky, [0.0, -1.0, 0.0])[0].abs() < 1e-6);
        assert!((sky_radiance(sky, [0.0, 0.5, 0.0])[0] - 0.75).abs() < 1e-6);
        // Its average over the sphere is (zenith + ground) / 4 + horizon / 2.
        assert!((sky_irradiance(sky)[0] - 0.5).abs() < 1e-6);
    }
}
