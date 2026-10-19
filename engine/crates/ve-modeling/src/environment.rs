//! Evaluation of the authored scene environment (`scene.environment`).
use crate::{Fail, SceneEnvironment, cache::Value, fail, result::SceneSky};
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

/// The light-probe settings a node asks for with `gi` (and `gi_spacing`, `gi_bounces`, `gi_rays`), or `None` when it does not.
pub(crate) fn probes(node: &Node) -> Option<crate::LightProbes> {
    if !matches!(node.params.get("gi"), Some(Param::Bool(true))) {
        return None;
    }
    let whole = |name: &str, default: u32, range: std::ops::RangeInclusive<u32>| {
        let value = match node.params.get(name) {
            Some(Param::Int(n)) => u32::try_from(*n).unwrap_or(default),
            _ => default,
        };
        value.clamp(*range.start(), *range.end())
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the spacing is a few metres"
    )]
    let spacing = scalar(node, "gi_spacing", 0.0).clamp(0.0, 64.0) as f32;
    Some(crate::LightProbes {
        spacing,
        bounces: whole("gi_bounces", 2, 1..=4),
        rays: whole("gi_rays", 128, 32..=512),
    })
}

/// The wind a node asks for with `wind_direction` (degrees) and `wind_strength`; the default breeze for what it leaves out.
pub(crate) fn wind(node: &Node) -> crate::Wind {
    let default = crate::Wind::default();
    #[expect(clippy::cast_possible_truncation, reason = "wind numbers are small")]
    crate::Wind {
        direction: scalar(node, "wind_direction", f64::from(default.direction)).clamp(-360.0, 360.0)
            as f32,
        strength: scalar(node, "wind_strength", f64::from(default.strength)).clamp(0.0, 2.0) as f32,
    }
}

/// Adds a `scene.sky` node's sky to the environment already authored: the environment's ambient stands, the sky is added, and
/// light probes or a wind the environment left unset are taken from the sky node.
pub(crate) fn add_sky(existing: &mut SceneEnvironment, scene: &SceneEnvironment, sky: &SceneSky) {
    existing.sky = Some(sky.clone());
    existing.gi = existing.gi.or(scene.gi);
    if existing.wind == crate::Wind::default() {
        existing.wind = scene.wind;
    }
}

/// The environment `authored` by a `scene.environment` node over what a `scene.sky` node already gave (`current`): the node's
/// own ambient replaces the sky's, and the sky background, light probes and wind it left unset are kept.
pub(crate) fn over_sky(
    authored: &SceneEnvironment,
    current: Option<&SceneEnvironment>,
) -> SceneEnvironment {
    let mut merged = authored.clone();
    if let Some(current) = current {
        merged.sky.clone_from(&current.sky);
        merged.gi = merged.gi.or(current.gi);
        if merged.wind == crate::Wind::default() {
            merged.wind = current.wind;
        }
    }
    merged
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
        gi: probes(node),
        wind: wind(node),
    }))
}
