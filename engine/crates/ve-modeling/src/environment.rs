//! Evaluation of the authored scene environment (`scene.environment`).
use crate::{Fail, SceneEnvironment, cache::Value, fail};
use ve_graph::{Node, Value as Param};

#[path = "sky.rs"]
pub(crate) mod sky;

fn vector(node: &Node, name: &str) -> Option<[f64; 3]> {
    let Some(Param::List(items)) = node.params.get(name) else {
        return None;
    };
    let [x, y, z] = items.as_slice() else {
        return None;
    };
    Some([x.as_f64()?, y.as_f64()?, z.as_f64()?])
}

fn scalar(node: &Node, name: &str, default: f64) -> f64 {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .unwrap_or(default)
}

fn cast(vector: [f64; 3]) -> [f32; 3] {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "environment colours are bounded to 0..1"
    )]
    {
        vector.map(|value| value as f32)
    }
}

pub(crate) fn execute(node: &Node) -> Result<Value, Fail> {
    let sky =
        vector(node, "sky_color").ok_or_else(|| fail("param.type", "sky_color must be a vec3"))?;
    let ground = vector(node, "ground_color")
        .ok_or_else(|| fail("param.type", "ground_color must be a vec3"))?;
    let bounce_color = vector(node, "bounce_color").unwrap_or([0.6, 0.44, 0.28]);
    Ok(Value::environment(SceneEnvironment {
        node: node.id.clone(),
        sky_color: cast(sky),
        ground_color: cast(ground),
        #[expect(
            clippy::cast_possible_truncation,
            reason = "intensity is bounded to 0..8"
        )]
        intensity: scalar(node, "intensity", 0.15) as f32,
        #[expect(
            clippy::cast_possible_truncation,
            reason = "reflection is bounded to 0..1"
        )]
        reflection: scalar(node, "reflection", 0.0) as f32,
        bounce_color: cast(bounce_color),
        #[expect(clippy::cast_possible_truncation, reason = "bounce is bounded to 0..1")]
        bounce: scalar(node, "bounce", 0.0) as f32,
        sky: None,
    }))
}
