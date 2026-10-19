//! `scene.sky`: bounded analytic atmosphere parameters, with no sampled assets.
use crate::{Fail, SceneEnvironment, cache::Value, fail, result::SceneSky};
use ve_graph::{Node, Value as Param};

fn scalar(node: &Node, key: &str, default: f64) -> f64 {
    node.params
        .get(key)
        .and_then(Param::as_f64)
        .unwrap_or(default)
}

fn colour(node: &Node, key: &str, default: [f64; 3]) -> Result<[f32; 3], Fail> {
    let Some(Param::List(values)) = node.params.get(key) else {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "fallback sRGB channels are bounded"
        )]
        return Ok(default.map(|v| v as f32));
    };
    let [r, g, b] = values.as_slice() else {
        return Err(fail("param.type", format!("{key} must be an RGB triple")));
    };
    let values = [r.as_f64(), g.as_f64(), b.as_f64()];
    if values
        .iter()
        .any(|v| !v.is_some_and(|n| n.is_finite() && (0.0..=1.0).contains(&n)))
    {
        return Err(fail(
            "param.range",
            format!("{key} channels must be between zero and one"),
        ));
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "sRGB channels are bounded to 0..=1"
    )]
    Ok(values.map(|v| v.unwrap_or_default() as f32))
}

/// The colour of the sunlight itself for a sun shining along `sun_direction` (the way the light travels): warm white high
/// in the sky, deepening to orange as it nears the horizon. It is not the sky's horizon colour, which is blue by day.
pub(crate) fn sun_colour(sun_direction: [f32; 3]) -> [f32; 3] {
    let elevation = (-sun_direction[1]).clamp(-1.0, 1.0).asin().to_degrees();
    let height = (elevation / 40.0).clamp(0.0, 1.0);
    let t = height * height * (3.0 - 2.0 * height);
    let (low, high) = ([1.0_f32, 0.55, 0.28], [1.0_f32, 0.95, 0.86]);
    [0, 1, 2].map(|i| low[i] + (high[i] - low[i]) * t)
}

pub(crate) fn execute(node: &Node) -> Result<Value, Fail> {
    let elevation = scalar(node, "sun_elevation", 35.0).to_radians();
    let azimuth = scalar(node, "sun_azimuth", 0.0).to_radians();
    let direction = [
        elevation.cos() * azimuth.sin(),
        -elevation.sin(),
        -elevation.cos() * azimuth.cos(),
    ];
    let turbidity = scalar(node, "turbidity", 3.0);
    let sun_intensity = scalar(node, "sun_intensity", 3.0);
    let ground = colour(node, "ground_color", [0.37, 0.29, 0.20])?;
    let sunset = ((20.0 - scalar(node, "sun_elevation", 35.0)).max(0.0) / 20.0).clamp(0.0, 1.0)
        * (turbidity / 10.0);
    let horizon = [
        0.47 + 0.35 * sunset,
        0.66 - 0.31 * sunset,
        0.89 - 0.48 * sunset,
    ];
    let zenith = [
        0.15 + 0.16 * turbidity / 10.0,
        0.36 - 0.08 * turbidity / 10.0,
        0.72 - 0.14 * turbidity / 10.0,
    ];
    let clouds = matches!(node.params.get("clouds"), Some(Param::Bool(true)));
    let cloud_cover = scalar(node, "cloud_cover", 0.0);
    let seed_value = node
        .params
        .get("seed")
        .and_then(Param::as_f64)
        .unwrap_or(0.0)
        .clamp(0.0, 1_000_000.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "seed is clamped to the non-negative range 0..=1,000,000"
    )]
    let seed = seed_value as u32;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "sun, turbidity and colour parameters are bounded"
    )]
    let (sun_direction, sun_intensity, turbidity, horizon_color, zenith_color, cloud_cover) = (
        direction.map(|v| v as f32),
        sun_intensity as f32,
        turbidity as f32,
        horizon.map(|v| v as f32),
        zenith.map(|v| v as f32),
        cloud_cover as f32,
    );
    let sky = SceneSky {
        node: node.id.clone(),
        sun_direction,
        sun_intensity,
        turbidity,
        horizon_color,
        zenith_color,
        ground_color: ground,
        clouds,
        cloud_cover,
        seed,
    };
    Ok(Value::environment(SceneEnvironment {
        node: node.id.clone(),
        sky_color: sky.zenith_color,
        ground_color: ground,
        intensity: 0.22,
        // Surfaces reflect the authored sky itself (see `sky_environment` in the forward shader), at full strength.
        reflection: 1.0,
        bounce_color: ground,
        bounce: 0.2,
        sky: Some(sky),
    }))
}
