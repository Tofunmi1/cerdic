//! Evaluation of authored scene-light values.
use crate::{Fail, SceneLight, SceneLightKind, cache::Value, fail};
use ve_graph::{Node, Value as Param};

fn vector(node: &Node, name: &str) -> Option<[f64; 3]> {
    let Some(Param::List(items)) = node.params.get(name) else {
        return None;
    };
    let [x, y, z] = items.as_slice() else {
        return None;
    };
    Some([x.as_f64()?, y.as_f64()?, z.as_f64()?])
}

fn scalar(node: &Node, name: &str) -> Option<f64> {
    node.params.get(name)?.as_f64()
}

fn cast(vector: [f64; 3]) -> [f32; 3] {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "light parameters are bounded well within f32"
    )]
    {
        vector.map(|value| value as f32)
    }
}

pub(crate) fn execute(node: &Node) -> Result<Value, Fail> {
    let family = node.op.family().as_str();
    let color =
        vector(node, "color").ok_or_else(|| fail("param.type", "light colour must be a vec3"))?;
    let intensity = scalar(node, "intensity")
        .ok_or_else(|| fail("param.type", "light intensity is required"))?;
    let position = vector(node, "position").unwrap_or([0.0; 3]);
    let mut direction = vector(node, "direction").unwrap_or([0.0, -1.0, 0.0]);
    let direction_length = direction.iter().map(|axis| axis * axis).sum::<f64>().sqrt();
    if family != "light.point" && direction_length <= f64::EPSILON {
        return Err(fail(
            "light.zero_direction",
            "light direction must not be zero",
        ));
    }
    if direction_length > 0.0 {
        for axis in &mut direction {
            *axis /= direction_length;
        }
    }
    let inner = scalar(node, "inner_angle").unwrap_or(0.0);
    let outer = scalar(node, "outer_angle").unwrap_or(0.0);
    if family == "light.spot" && inner >= outer {
        return Err(fail(
            "light.invalid_cone",
            "inner_angle must be smaller than outer_angle",
        ));
    }
    let kind = match family {
        "light.point" => SceneLightKind::Point,
        "light.spot" => SceneLightKind::Spot,
        "light.sun" => SceneLightKind::Sun,
        _ => return Err(fail("parse.unknown_op", family)),
    };
    let name = node.label.clone().unwrap_or_else(|| node.id.to_string());
    Ok(Value::light(SceneLight {
        node: node.id.clone(),
        name,
        kind,
        position: cast(position),
        direction: cast(direction),
        color: cast(color),
        #[expect(clippy::cast_possible_truncation, reason = "intensity is bounded")]
        intensity: intensity as f32,
        #[expect(clippy::cast_possible_truncation, reason = "range is bounded")]
        range: scalar(node, "range").unwrap_or(0.0) as f32,
        #[expect(clippy::cast_possible_truncation, reason = "angles are bounded")]
        inner_angle: inner as f32,
        #[expect(clippy::cast_possible_truncation, reason = "angles are bounded")]
        outer_angle: outer as f32,
    }))
}
