//! Small 3D vector helpers shared by the sweep and its overlap check.

pub(super) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(super) fn scale(v: [f64; 3], factor: f64) -> [f64; 3] {
    v.map(|component| component * factor)
}

pub(super) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(super) fn length(v: [f64; 3]) -> f64 {
    dot(v, v).sqrt()
}

pub(super) fn normalize(v: [f64; 3]) -> Option<[f64; 3]> {
    let size = length(v);
    (size > 1.0e-12).then(|| v.map(|component| component / size))
}
