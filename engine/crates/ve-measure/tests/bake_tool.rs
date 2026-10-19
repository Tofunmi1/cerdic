//! Integration tests for the model-facing bake JSON adapter.
use ve_measure::bake_maps_json;

const POS: [f32; 12] = [0., 0., 0., 1., 0., 0., 1., 1., 0., 0., 1., 0.];
const IDX: [u32; 6] = [0, 1, 2, 0, 2, 3];
const UV: [f32; 12] = [0., 0., 1., 0., 1., 1., 0., 0., 1., 1., 0., 1.];
const TILTED: [f32; 12] = [0., 0., 0., 1., 0., 0.5, 1., 1., 0.5, 0., 1., 0.];
fn call(high: &[f32], size: u32, maps: &str, rays: u32) -> String {
    bake_maps_json(
        &POS, &IDX, &UV, high, &IDX, size, maps, 0.6, 1.0, rays, 0.001,
    )
}
fn field<'a>(json: &'a str, key: &str) -> &'a str {
    let marker = format!("\"{key}\":\"");
    let Some(start) = json.find(&marker).map(|index| index + marker.len()) else {
        return "";
    };
    let rest = &json[start..];
    let Some(end) = rest.find('"') else {
        return "";
    };
    &rest[..end]
}
fn decode(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = 0_u32;
    let mut bits = 0;
    for byte in text.bytes().take_while(|b| *b != b'=') {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        buf = (buf << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from(buf >> bits).ok().map_or(0, |value| value));
            buf &= (1_u32 << bits) - 1;
        }
    }
    out
}
#[test]
fn tilted_normal_map_has_expected_centre_and_coverage() {
    let json = call(&TILTED, 32, "normal", 64);
    assert!(json.contains("\"ok\":true"));
    assert!(json.contains("\"ao\":null"));
    let rgba = decode(field(&json, "rgbaBase64"));
    assert_eq!(rgba.len(), 32 * 32 * 4);
    let covered = json
        .split("\"covered\":")
        .nth(1)
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    assert!(covered >= 960, "covered={covered}");
    let centre = &rgba[(16 * 32 + 16) * 4..(16 * 32 + 16) * 4 + 3];
    for (got, want) in centre.iter().zip([70_i16, 128, 242]) {
        assert!((i16::from(*got) - want).abs() <= 2, "centre={centre:?}");
    }
}
#[test]
fn both_maps_are_present_and_flat_surface_is_open() {
    let json = call(&POS, 32, "both", 64);
    assert!(json.contains("\"normal\":{"));
    assert!(json.contains("\"ao\":{"));
    let ao = json.split("\"ao\":{").nth(1).unwrap_or("");
    let pixels = decode(field(ao, "rgbaBase64"));
    assert!((i16::from(pixels[(16 * 32 + 16) * 4]) - 255).abs() <= 3);
}
#[test]
fn missing_uv_has_unwrap_suggestion() {
    let json = bake_maps_json(&POS, &IDX, &[], &POS, &IDX, 32, "normal", 0.6, 1., 64, 0.);
    assert!(json.contains("\"ok\":false"));
    assert!(json.contains("poly.unwrap_uv"));
}
#[test]
fn invalid_size_rays_and_empty_high_refuse_with_suggestions() {
    for json in [
        call(&POS, 4, "normal", 64),
        call(&POS, 32, "ao", 0),
        bake_maps_json(&POS, &IDX, &UV, &[], &[], 32, "normal", 0.6, 1., 64, 0.),
    ] {
        assert!(json.contains("\"ok\":false"));
        assert!(!field(&json, "suggestion").is_empty());
    }
}
#[test]
fn rgba_base64_decodes_to_exact_dimensions() {
    for size in [8, 33, 64] {
        let json = call(&POS, size, "normal", 64);
        assert_eq!(
            decode(field(&json, "rgbaBase64")).len(),
            size as usize * size as usize * 4
        );
    }
}
