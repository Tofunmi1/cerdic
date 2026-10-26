//! Seeded area sampling, floor clearance, and camera fallback.
use super::{
    SurveyCamera, SurveyOptions,
    geometry::{Scene, add, length, number, scale, sub, unit},
};
use std::collections::BTreeMap;

pub(super) struct Random(pub u64);
impl Random {
    pub(super) fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        let bits = u32::try_from((z ^ (z >> 31)) >> 32).unwrap_or_default();
        f64::from(bits) / 4_294_967_296.0
    }
}

pub(super) struct Sample {
    pub position: [f64; 3],
    pub normal: [f64; 3],
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "positive finite area counts use saturating float conversion"
)]
fn count(area: f64, density: f64, rng: &mut Random) -> usize {
    let n = area * density;
    (n.floor() as usize).saturating_add(usize::from(rng.unit() < n.fract()))
}

fn point(t: super::Triangle, rng: &mut Random) -> [f64; 3] {
    let mut a = rng.unit();
    let mut b = rng.unit();
    if a + b > 1.0 {
        a = 1.0 - a;
        b = 1.0 - b;
    }
    add(
        t[0],
        add(scale(sub(t[1], t[0]), a), scale(sub(t[2], t[0]), b)),
    )
}

pub(super) fn surface(scene: &Scene, density: f64, rng: &mut Random) -> Vec<Sample> {
    let mut out = Vec::new();
    for face in &scene.faces {
        for _ in 0..count(face.area, density, rng) {
            out.push(Sample {
                position: point(face.triangle, rng),
                normal: face.normal,
            });
        }
    }
    out
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "grid coordinates use saturating float-to-integer conversion"
)]
fn cell(v: f64, spacing: f64) -> i64 {
    (v / spacing).floor() as i64
}

pub(super) fn standing(scene: &Scene, o: &SurveyOptions, rng: &mut Random) -> Vec<[f64; 3]> {
    let mut out: Vec<[f64; 3]> = Vec::new();
    let mut grid: BTreeMap<(i64, i64), Vec<[f64; 3]>> = BTreeMap::new();
    for face in &scene.faces {
        if face.normal[1] < 0.7 {
            continue;
        }
        for _ in 0..count(face.area, o.samples_per_m2 * 0.5, rng) {
            let floor = point(face.triangle, rng);
            let eye = add(floor, [0.0, o.eye_height, 0.0]);
            if scene
                .ray(
                    add(floor, [0.0, 1e-5, 0.0]),
                    [0.0, 1.0, 0.0],
                    o.eye_height + 0.3 - 1e-5,
                )
                .is_some()
            {
                continue;
            }
            if !scene
                .ray(eye, [0.0, -1.0, 0.0], o.eye_height + 0.1)
                .is_some_and(|d| (d - o.eye_height).abs() <= 0.1)
            {
                continue;
            }
            let key = (cell(eye[0], o.spacing), cell(eye[2], o.spacing));
            let mut clash = false;
            for dx in -1..=1 {
                for dz in -1..=1 {
                    if let Some(neighbours) =
                        grid.get(&(key.0.saturating_add(dx), key.1.saturating_add(dz)))
                    {
                        clash |= neighbours.iter().any(|q| {
                            (eye[0] - q[0]).hypot(eye[2] - q[2]) < o.spacing
                                && (eye[1] - q[1]).abs() < 2.0
                        });
                    }
                }
            }
            if !clash {
                grid.entry(key).or_default().push(eye);
                out.push(eye);
            }
        }
    }
    out
}

pub(super) fn aerial(scene: &Scene, budget: usize) -> Vec<SurveyCamera> {
    let centre = scale(add(scene.min, scene.max), 0.5);
    let diagonal = length(sub(scene.max, scene.min));
    let low_count = budget.div_ceil(2);
    let high_count = budget / 2;
    (0..budget)
        .map(|i| {
            let high = i >= low_count;
            let (index, ring_count) = if high {
                (i - low_count, high_count)
            } else {
                (i, low_count)
            };
            let angle = std::f64::consts::TAU * number(index) / number(ring_count) + 0.3;
            let position = [
                centre[0] + angle.sin() * 1.2 * diagonal,
                if high {
                    scene.max[1] + diagonal / 6.0
                } else {
                    centre[1]
                },
                centre[2] + angle.cos() * 1.2 * diagonal,
            ];
            SurveyCamera {
                position,
                target: add(position, unit(sub(centre, position))),
                label: format!("aerial_{}", i + 1),
            }
        })
        .collect()
}
