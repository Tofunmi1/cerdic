//! Small deterministic vector operations for LOD meshing.
pub(in crate::lod::mesh) fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub(in crate::lod::mesh) fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(in crate::lod::mesh) fn scale(a: [f32; 3], factor: f32) -> [f32; 3] {
    [a[0] * factor, a[1] * factor, a[2] * factor]
}
pub(super) fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(super) fn length(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

pub(in crate::lod::mesh) fn unit(a: [f32; 3]) -> [f32; 3] {
    let magnitude = length(a);
    if magnitude > 1.0e-8 {
        scale(a, 1.0 / magnitude)
    } else {
        [0.0, 1.0, 0.0]
    }
}

pub(in crate::lod::mesh) fn rotate(vector: [f32; 3], axis: [f32; 3], angle: f32) -> [f32; 3] {
    let axis = unit(axis);
    let (sine, cosine) = sin_cos(angle);
    let cross = [
        axis[1] * vector[2] - axis[2] * vector[1],
        axis[2] * vector[0] - axis[0] * vector[2],
        axis[0] * vector[1] - axis[1] * vector[0],
    ];
    unit(add(
        add(scale(vector, cosine), scale(cross, sine)),
        scale(axis, dot(axis, vector) * (1.0 - cosine)),
    ))
}

pub(in crate::lod::mesh) fn perpendicular(vector: [f32; 3]) -> [f32; 3] {
    unit(if vector[1].abs() < 0.9 {
        [vector[2], 0.0, -vector[0]]
    } else {
        [1.0, 0.0, 0.0]
    })
}

pub(in crate::lod::mesh) fn sin_cos(angle: f32) -> (f32, f32) {
    let tau = core::f32::consts::TAU;
    let mut x = angle - (angle / tau).floor() * tau;
    if x > core::f32::consts::PI {
        x -= tau;
    }
    let sign = if x.abs() > core::f32::consts::FRAC_PI_2 {
        -1.0
    } else {
        1.0
    };
    if x > core::f32::consts::FRAC_PI_2 {
        x = core::f32::consts::PI - x;
    } else if x < -core::f32::consts::FRAC_PI_2 {
        x = -core::f32::consts::PI - x;
    }
    let x2 = x * x;
    let sine = x
        * (1.0
            + x2 * (-0.166_666_67
                + x2 * (0.008_333_331 + x2 * (-0.000_198_409 + x2 * 0.000_002_752_6))));
    let cosine = sign
        * (1.0
            + x2 * (-0.5 + x2 * (0.041_666_638 + x2 * (-0.001_388_837_8 + x2 * 0.000_024_760_9))));
    (sine, cosine)
}
