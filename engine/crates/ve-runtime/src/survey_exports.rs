//! Camera planning over the committed evaluated geometry.
use serde_json::{Value, json};
use ve_world::survey::{SurveyMode, SurveyOptions, plan_cameras};
use wasm_bindgen::prelude::wasm_bindgen;

/// Plans a survey of all committed drawn parts. Accepts any `SurveyOptions` field
/// as JSON and returns cameras, predicted coverage, geometry counts and elapsed ms.
#[wasm_bindgen]
#[must_use]
pub fn survey_plan(options_json: &str) -> String {
    let began = now();
    let options = match parse_options(options_json) {
        Ok(options) => options,
        Err(error) => return json!({"error": error}).to_string(),
    };
    crate::projection::survey::with_geometry(|triangles, skipped_instances| {
        let plan = plan_cameras(triangles, &options);
        let cameras: Vec<_> = plan
            .cameras
            .iter()
            .map(|camera| {
                json!({
                    "position": camera.position, "target": camera.target, "label": camera.label,
                })
            })
            .collect();
        json!({
            "mode": match plan.mode {
                SurveyMode::Auto => "auto", SurveyMode::Walk => "walk", SurveyMode::Orbit => "orbit",
            },
            "cameras": cameras, "coverage": plan.coverage, "predicted": plan.predicted,
            "standable_points": plan.standable_points, "surface_samples": plan.surface_samples,
            "triangles": triangles.len(), "skipped_instances": skipped_instances, "ms": now() - began,
        }).to_string()
    })
}

fn now() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or(0.0, |clock| clock.now())
}

fn parse_options(text: &str) -> Result<SurveyOptions, String> {
    let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let fields = value
        .as_object()
        .ok_or("survey options must be an object")?;
    let mut o = SurveyOptions::default();
    for (key, value) in fields {
        match key.as_str() {
            "mode" => {
                o.mode = match value.as_str() {
                    Some("auto") => SurveyMode::Auto,
                    Some("walk") => SurveyMode::Walk,
                    Some("orbit") => SurveyMode::Orbit,
                    _ => return Err("mode must be one of auto, walk, orbit".into()),
                }
            }
            "eye_height" => o.eye_height = number(key, value, 0.1, 5.0)?,
            "spacing" => o.spacing = number(key, value, 0.25, 100.0)?,
            "fov_deg" => o.fov_deg = number(key, value, 10.0, 170.0)?,
            "pitch_deg" => o.pitch_deg = number(key, value, -89.0, 89.0)?,
            "target_coverage" => o.target_coverage = number(key, value, 0.0, 1.0)?,
            "max_distance" => o.max_distance = number(key, value, 0.1, 1000.0)?,
            "samples_per_m2" => o.samples_per_m2 = number(key, value, 0.01, 20.0)?,
            "yaws" => o.yaws = integer(key, value, 32)?,
            "max_views" => o.max_views = integer(key, value, 64)?,
            "seed" => {
                o.seed = value
                    .as_u64()
                    .filter(|n| *n <= 9_007_199_254_740_991)
                    .ok_or("seed must be an integer from 0 to 9007199254740991")?
            }
            _ => return Err(format!("unknown survey option {key}")),
        }
    }
    Ok(o)
}

fn number(key: &str, value: &Value, min: f64, max: f64) -> Result<f64, String> {
    value
        .as_f64()
        .filter(|n| n.is_finite() && (min..=max).contains(n))
        .ok_or_else(|| format!("{key} must be a finite number from {min} to {max}"))
}

fn integer(key: &str, value: &Value, max: usize) -> Result<usize, String> {
    value
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| (1..=max).contains(n))
        .ok_or_else(|| format!("{key} must be an integer from 1 to {max}"))
}
