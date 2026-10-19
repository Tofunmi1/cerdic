//! Rotation-minimizing frames with distributed holonomy correction for closed paths.
use super::vec3::{cross, dot, normalize, sub};

pub(super) fn frames(points: &[[f64; 3]]) -> Option<Vec<([f64; 3], [f64; 3])>> {
    if points.len() < 3 {
        return None;
    }
    let tangents = (0..points.len())
        .map(|index| {
            let previous = points[(index + points.len() - 1) % points.len()];
            let next = points[(index + 1) % points.len()];
            normalize(sub(next, previous))
        })
        .collect::<Option<Vec<_>>>()?;
    let reference = if tangents[0][1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let mut normal = normalize(cross(tangents[0], reference))?;
    let mut normals = vec![normal];
    for index in 1..points.len() {
        normal = transport(
            normal,
            tangents[index - 1],
            tangents[index],
            sub(points[index], points[index - 1]),
        )?;
        normals.push(normal);
    }
    let seam = transport(
        normal,
        tangents[points.len() - 1],
        tangents[0],
        sub(points[0], points[points.len() - 1]),
    )?;
    let residual = dot(tangents[0], cross(seam, normals[0])).atan2(dot(seam, normals[0]));
    let count = f64::from(u32::try_from(points.len()).ok()?);
    normals
        .into_iter()
        .zip(tangents)
        .enumerate()
        .map(|(index, (normal, tangent))| {
            let index = u32::try_from(index).ok()?;
            let denominator = count - 1.0;
            let angle = residual * f64::from(index) / denominator;
            let normal = rotate(normal, tangent, angle);
            let binormal = normalize(cross(tangent, normal))?;
            let normal = normalize(cross(binormal, tangent))?;
            Some((normal, binormal))
        })
        .collect::<Option<Vec<_>>>()
}

fn transport(
    normal: [f64; 3],
    previous: [f64; 3],
    current: [f64; 3],
    chord: [f64; 3],
) -> Option<[f64; 3]> {
    let tangent_reflection = reflect(previous, chord)?;
    let normal_reflection = reflect(normal, chord)?;
    let correction = sub(current, tangent_reflection);
    if dot(correction, correction) > 1.0e-24 {
        reflect(normal_reflection, correction)
    } else {
        Some(normal_reflection)
    }
}

fn reflect(vector: [f64; 3], plane_normal: [f64; 3]) -> Option<[f64; 3]> {
    let denominator = dot(plane_normal, plane_normal);
    if denominator <= 1.0e-24 {
        return None;
    }
    let factor = 2.0 * dot(vector, plane_normal) / denominator;
    normalize(sub(
        vector,
        plane_normal.map(|component| component * factor),
    ))
}

fn rotate(vector: [f64; 3], axis: [f64; 3], angle: f64) -> [f64; 3] {
    let (sine, cosine) = angle.sin_cos();
    let perpendicular = cross(axis, vector);
    let projection = dot(axis, vector) * (1.0 - cosine);
    [0, 1, 2].map(|index| {
        vector[index] * cosine + perpendicular[index] * sine + axis[index] * projection
    })
}

#[cfg(test)]
mod tests {
    use super::{frames, transport};
    use crate::curves::vec3::{dot, normalize, sub};

    #[test]
    fn g5_07_closed_frames_match_across_a_nonplanar_seam() {
        let points = [
            [1.0, 0.0, 0.0],
            [0.3, 0.8, 0.5],
            [-0.8, 0.4, -0.3],
            [-0.7, -0.6, 0.4],
            [0.5, -0.9, -0.2],
        ];
        let frames = frames(&points).unwrap_or_default();
        assert_eq!(frames.len(), points.len());
        let tangent_at = |index: usize| {
            normalize(sub(
                points[(index + 1) % points.len()],
                points[(index + points.len() - 1) % points.len()],
            ))
            .unwrap_or([0.0; 3])
        };
        let last = points.len() - 1;
        let seam = transport(
            frames[last].0,
            tangent_at(last),
            tangent_at(0),
            sub(points[0], points[last]),
        )
        .unwrap_or(frames[0].0);
        let seam_angle = dot(seam, frames[0].0).clamp(-1.0, 1.0).acos();
        assert!(seam_angle < 1.0e-6, "seam angle is {seam_angle} radians");
    }
}
