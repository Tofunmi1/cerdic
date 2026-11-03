/// Translation, unit quaternion rotation (x, y, z, w), and scale for one pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trs {
    /// Position.
    pub translation: [f32; 3],
    /// Unit quaternion in x, y, z, w order.
    pub rotation: [f32; 4],
    /// Per-axis scale.
    pub scale: [f32; 3],
}

/// Blends poses by normalized weight, using normalized linear interpolation for rotation.
///
/// Nlerp is cheap and deterministic, exact at the midpoint of two rotations, and accurate for
/// the small angles between clips of one character.
#[must_use]
pub fn blend_trs(items: &[(f32, Trs)]) -> Option<Trs> {
    if items
        .iter()
        .any(|(weight, _)| !weight.is_finite() || *weight < 0.0)
    {
        return None;
    }
    let max_weight = items
        .iter()
        .map(|(weight, _)| *weight)
        .fold(0.0_f32, f32::max);
    if max_weight == 0.0 {
        return None;
    }

    let mut total = 0.0_f32;
    for (weight, pose) in items.iter().filter(|(weight, _)| *weight > 0.0) {
        if pose
            .translation
            .iter()
            .chain(&pose.scale)
            .chain(&pose.rotation)
            .any(|v| !v.is_finite())
        {
            return None;
        }
        if pose.rotation.iter().all(|component| *component == 0.0) {
            return None;
        }
        total += *weight / max_weight;
    }
    if total == 0.0 || !total.is_finite() {
        return None;
    }

    let reference = normalize(items.iter().find(|(weight, _)| *weight > 0.0)?.1.rotation)?;
    let mut translation = [0.0; 3];
    let mut scale = [0.0; 3];
    let mut rotation = [0.0; 4];
    let mut heaviest = None;
    let mut heaviest_weight = -1.0_f32;
    for (weight, pose) in items.iter().filter(|(weight, _)| *weight > 0.0) {
        let normalized = (*weight / max_weight) / total;
        for axis in 0..3 {
            translation[axis] += normalized * pose.translation[axis];
            scale[axis] += normalized * pose.scale[axis];
        }
        let mut q = normalize(pose.rotation)?;
        if dot(q, reference) < 0.0 {
            for component in &mut q {
                *component = -*component;
            }
        }
        for axis in 0..4 {
            rotation[axis] += normalized * q[axis];
        }
        if *weight > heaviest_weight {
            heaviest = Some(pose.rotation);
            heaviest_weight = *weight;
        }
    }
    if rotation.iter().all(|component| *component == 0.0) {
        rotation = normalize(heaviest?)?;
    } else {
        rotation = normalize(rotation)?;
    }
    Some(Trs {
        translation,
        rotation,
        scale,
    })
}

fn multiply(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

fn finite(pose: &Trs) -> bool {
    pose.translation.iter().chain(&pose.rotation).chain(&pose.scale).all(|v| v.is_finite())
}

/// Adds an overlay clip to a base pose: what the clip changed since its `reference` (its first frame) is applied on top of `base`, scaled by
/// `weight` (`0` leaves `base` alone). Translation adds the difference, rotation applies the turn from the reference to `current` (partway
/// by normalised interpolation), scale multiplies by the ratio. Anything not finite, or a weight that is not, gives `base` back.
#[must_use]
pub fn apply_additive(base: &Trs, reference: &Trs, current: &Trs, weight: f32) -> Trs {
    if !weight.is_finite() || weight <= 0.0 || !finite(base) || !finite(reference) || !finite(current) {
        return *base;
    }
    let weight = weight.min(1.0);
    let mut translation = base.translation;
    let mut scale = base.scale;
    for axis in 0..3 {
        translation[axis] += weight * (current.translation[axis] - reference.translation[axis]);
        let ratio = if reference.scale[axis].abs() > 1.0e-6 { current.scale[axis] / reference.scale[axis] } else { 1.0 };
        scale[axis] = base.scale[axis] * (1.0 + weight * (ratio - 1.0));
    }
    let Some(reference_q) = normalize(reference.rotation) else {
        return Trs { translation, scale, ..*base };
    };
    let Some(current_q) = normalize(current.rotation) else {
        return Trs { translation, scale, ..*base };
    };
    let inverse = [-reference_q[0], -reference_q[1], -reference_q[2], reference_q[3]];
    let mut delta = multiply(current_q, inverse);
    if delta[3] < 0.0 {
        delta = delta.map(|c| -c);
    }
    let partway = [delta[0] * weight, delta[1] * weight, delta[2] * weight, (1.0 - weight) + delta[3] * weight];
    let rotation = normalize(multiply(partway, base.rotation)).unwrap_or(base.rotation);
    Trs {
        translation,
        rotation,
        scale,
    }
}

/// Replaces `base` by `clip` in proportion to `weight` (`0` keeps `base`, `1` is `clip`). Not finite gives `base` back.
#[must_use]
pub fn apply_override(base: &Trs, clip: &Trs, weight: f32) -> Trs {
    if !weight.is_finite() || weight <= 0.0 {
        return *base;
    }
    let weight = weight.min(1.0);
    blend_trs(&[(1.0 - weight, *base), (weight, *clip)]).unwrap_or(*base)
}

fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn normalize(q: [f32; 4]) -> Option<[f32; 4]> {
    let max = q.iter().map(|value| value.abs()).fold(0.0_f32, f32::max);
    if max == 0.0 || !max.is_finite() {
        return None;
    }
    let mut scaled = q.map(|value| value / max);
    let length = scaled.iter().map(|value| value * value).sum::<f32>().sqrt();
    for value in &mut scaled {
        *value /= length;
    }
    Some(scaled)
}
