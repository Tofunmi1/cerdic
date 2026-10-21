use super::hat_weights;

#[test]
fn hat_weights_interpolate_adjacent_anchors() {
    let anchors = [0.0, 0.5, 1.0];
    let low = hat_weights(0.0, &anchors);
    assert!(low[0] > 1.0 - f32::EPSILON && low[1] < f32::EPSILON);
    let exact = hat_weights(0.5, &anchors);
    assert!(exact[1] > 1.0 - f32::EPSILON);
    let between = hat_weights(0.25, &anchors);
    assert!((between[0] - 0.5).abs() < f32::EPSILON);
    assert!((between[1] - 0.5).abs() < f32::EPSILON);
    assert!(between[2].abs() < f32::EPSILON);
    assert!((between.iter().sum::<f32>() - 1.0).abs() < f32::EPSILON);
}
