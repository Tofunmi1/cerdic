use super::{cross, dot, frames, normalize, reflect, sub};

#[test]
fn g5_04_double_reflection_frames_stay_orthonormal_through_an_inflection() {
    let points = [
        [0.0, 0.0, 0.0],
        [0.6, 0.8, 0.1],
        [1.2, 1.0, 0.4],
        [1.8, 0.6, 0.8],
        [2.4, 0.0, 0.4],
        [3.0, -0.2, 0.0],
    ];
    let frames = frames(&points).unwrap_or_default();
    assert_eq!(frames.len(), points.len());
    for index in 0..frames.len() {
        let current_tangent = tangent(&points, index);
        let (normal, binormal) = frames[index];
        assert!((dot(normal, normal) - 1.0).abs() < 1.0e-12);
        assert!((dot(binormal, binormal) - 1.0).abs() < 1.0e-12);
        assert!(dot(normal, current_tangent).abs() < 1.0e-12);
        assert!(dot(binormal, current_tangent).abs() < 1.0e-12);
        assert!(dot(cross(current_tangent, normal), binormal) > 1.0 - 1.0e-12);
        if index > 0 {
            assert!(dot(frames[index - 1].0, normal) > 0.0);
            let chord = sub(points[index], points[index - 1]);
            let previous = tangent(&points, index - 1);
            let reflected = reflect(previous, chord).unwrap_or(previous);
            let normal = reflect(frames[index - 1].0, chord).unwrap_or(frames[index - 1].0);
            let correction = sub(current_tangent, reflected);
            let expected = if dot(correction, correction) > 1.0e-24 {
                reflect(normal, correction).unwrap_or(normal)
            } else {
                normal
            };
            assert!(dot(expected, frames[index].0) > 1.0 - 1.0e-12);
        }
    }
}

#[test]
fn g5_04_double_reflection_keeps_a_straight_path_frame_fixed() {
    let points = [
        [0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 2.0, 0.0],
        [0.0, 3.0, 0.0],
    ];
    let frames = frames(&points).unwrap_or_default();
    assert_eq!(frames.len(), points.len());
    assert!(
        frames
            .windows(2)
            .all(|pair| dot(pair[0].0, pair[1].0) > 1.0 - 1.0e-12)
    );
}

fn tangent(points: &[[f64; 3]], index: usize) -> [f64; 3] {
    let direction = if index == 0 {
        sub(points[1], points[0])
    } else if index + 1 == points.len() {
        sub(points[index], points[index - 1])
    } else {
        sub(points[index + 1], points[index - 1])
    };
    normalize(direction).unwrap_or([0.0; 3])
}
