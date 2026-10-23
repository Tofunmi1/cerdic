//! Pattern driven changes to each placement in an instance set.
use glam::DMat4;
use ve_graph::Node;
use ve_world::expr::Type;

use super::InstanceSet;
use crate::{Fail, fail_suggest};

/// Samples linked patterns at each placement origin, then changes its basis or height in place.
pub(super) fn transforms(
    node: &Node,
    base: &InstanceSet,
    inputs: &[std::rc::Rc<crate::cache::Value>],
) -> Result<Vec<DMat4>, Fail> {
    let linked: std::collections::BTreeMap<_, _> = node
        .inputs
        .keys()
        .zip(inputs)
        .map(|(name, value)| (name.as_str(), value))
        .collect();
    let patterns = ["scale", "yaw", "lift"]
        .map(|name| linked.get(name).and_then(|value| value.pattern.as_deref()));
    for (name, pattern) in ["scale", "yaw", "lift"].into_iter().zip(patterns) {
        if let Some(pattern) = pattern {
            if pattern.output_type() != Type::Float {
                return Err(fail_suggest(
                    "pattern.not_float",
                    format!(
                        "the `{name}` pattern of {} gives {:?}, but variation needs one number",
                        node.id,
                        pattern.output_type()
                    ),
                    "end the expression with a single number, for example `out = 1.0`, or pick one component such as `.x`",
                ));
            }
        }
    }
    let mut result = Vec::with_capacity(base.transforms.len());
    for original in &base.transforms {
        let translation = original.w_axis.truncate();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "placement positions are bounded by the kernel extent"
        )]
        let position = translation.to_array().map(|component| component as f32);
        let mut matrix = *original;
        if let Some(pattern) = patterns[0] {
            let scale = sample(node, "scale", pattern, position)?;
            if !(0.01..=100.0).contains(&scale) {
                return Err(fail_suggest(
                    "param.range",
                    format!(
                        "{} scale pattern returned {scale}, outside 0.01..=100",
                        node.id
                    ),
                    "change the scale pattern so every placement returns a value from 0.01 to 100",
                ));
            }
            matrix.x_axis *= f64::from(scale);
            matrix.y_axis *= f64::from(scale);
            matrix.z_axis *= f64::from(scale);
        }
        if let Some(pattern) = patterns[1] {
            let yaw = sample(node, "yaw", pattern, position)?;
            let origin = matrix.w_axis;
            matrix = DMat4::from_rotation_y(f64::from(yaw)) * matrix;
            matrix.w_axis = origin;
        }
        if let Some(pattern) = patterns[2] {
            let lift = sample(node, "lift", pattern, position)?;
            matrix.w_axis.y += f64::from(lift);
        }
        result.push(matrix);
    }
    Ok(result)
}

fn sample(
    node: &Node,
    name: &str,
    pattern: &crate::world_field::Pattern,
    position: [f32; 3],
) -> Result<f32, Fail> {
    match pattern.sample(position, [0.0, 1.0, 0.0]) {
        ve_world::expr::Value::Float(value) if value.is_finite() => Ok(value),
        _ => Err(fail_suggest(
            "pattern.not_float",
            format!(
                "the `{name}` pattern of {} did not give a finite number",
                node.id
            ),
            "change the pattern so it gives one finite number at every placement",
        )),
    }
}
