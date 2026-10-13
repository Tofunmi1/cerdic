//! Material parsing at the projection boundary.
use super::{Color, VeMaterial};
pub(super) fn parse(text: &str) -> Option<VeMaterial> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let triple = |name: &str, max: f64| -> Option<[f32; 3]> {
        let items = value.get(name)?.as_array()?;
        if items.len() != 3 {
            return None;
        }
        let mut out = [0.0; 3];
        for (slot, item) in out.iter_mut().zip(items) {
            let channel = item.as_f64().filter(|v| (0.0..=max).contains(v))?;
            #[expect(clippy::cast_possible_truncation, reason = "bounded material channels")]
            {
                *slot = channel as f32;
            }
        }
        Some(out)
    };
    let factor = |name: &str| -> Option<f32> {
        let number = value
            .get(name)?
            .as_f64()
            .filter(|v| (0.0..=1.0).contains(v))?;
        #[expect(clippy::cast_possible_truncation, reason = "bounded material factors")]
        Some(number as f32)
    };
    let color = triple("color", 1.0)?;
    let emissive = triple("emissive", 100.0)?;
    Some(VeMaterial {
        base_color: Color::srgb(color[0], color[1], color[2]),
        metallic: factor("metallic")?,
        roughness: factor("roughness")?,
        emissive: Color::linear_rgb(emissive[0], emissive[1], emissive[2]),
    })
}
