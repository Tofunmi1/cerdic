//! Bounded point lights derived from explicitly emissive output surfaces.
use crate::{Part, SceneLight, SceneLightKind};

const INTENSITY_SCALE: f64 = 20.0;
const MAX_INTENSITY: f64 = 20.0;
const RANGE_SCALE: f64 = 6.0;

pub(crate) fn from_parts(parts: &[Part]) -> Vec<SceneLight> {
    parts.iter().filter_map(emitter).collect()
}

fn emitter(part: &Part) -> Option<SceneLight> {
    let material = part.material.as_ref()?;
    if !material.emits_light {
        return None;
    }
    let peak = material.emissive.iter().copied().fold(0.0_f32, f32::max);
    if peak <= 0.0 {
        return None;
    }
    let (position, diagonal) = bounds(&part.positions)?;
    let area = surface_area(part);
    if !area.is_finite() || area <= 0.0 {
        return None;
    }
    let linear = material.emissive.map(|channel| channel / peak);
    let intensity = (area * f64::from(peak) * INTENSITY_SCALE).min(MAX_INTENSITY);
    #[expect(clippy::cast_possible_truncation, reason = "bounded by MAX_INTENSITY")]
    let intensity = intensity as f32;
    let range = (diagonal * RANGE_SCALE).clamp(1.0, 1_000.0);
    #[expect(clippy::cast_possible_truncation, reason = "bounded to 1,000 metres")]
    let range = range as f32;
    Some(SceneLight {
        node: part.node.clone(),
        name: format!("{} emission", part.name),
        kind: SceneLightKind::Point,
        position,
        direction: [0.0, -1.0, 0.0],
        color: linear.map(linear_to_srgb),
        intensity,
        range,
        inner_angle: 0.0,
        outer_angle: 0.0,
    })
}

fn bounds(positions: &[[f32; 3]]) -> Option<([f32; 3], f64)> {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for position in positions {
        if position.iter().any(|value| !value.is_finite()) {
            return None;
        }
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    let center = std::array::from_fn(|axis| (min[axis] + max[axis]) * 0.5);
    let diagonal = (0..3)
        .map(|axis| f64::from(max[axis] - min[axis]).powi(2))
        .sum::<f64>()
        .sqrt();
    diagonal.is_finite().then_some((center, diagonal))
}

fn surface_area(part: &Part) -> f64 {
    part.indices
        .chunks_exact(3)
        .map(|triangle| {
            let a = part.positions[triangle[0] as usize].map(f64::from);
            let b = part.positions[triangle[1] as usize].map(f64::from);
            let c = part.positions[triangle[2] as usize].map(f64::from);
            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let cross = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            0.5 * cross.iter().map(|axis| axis * axis).sum::<f64>().sqrt()
        })
        .sum()
}

fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Material;

    fn material(emissive: [f32; 3], emits_light: bool) -> Material {
        Material {
            color: [0.8; 3],
            opacity: 1.0,
            metallic: 0.0,
            roughness: 0.5,
            emissive,
            emits_light,
            recipe: crate::MaterialRecipe::None,
            texture_scale: 1.0,
            texture_tile: 0.0,
            texture_size: 0,
            toon: None,
            texture_program: None,
            texture_style: None,
            foliage: None,
        }
    }

    fn box_part() -> Part {
        Part {
            node: ve_graph::NodeId::parse("n_01234567").expect("valid fixture id"),
            name: "lamp".to_owned(),
            positions: vec![
                [-1.0, -0.5, -0.5],
                [1.0, -0.5, -0.5],
                [1.0, 0.5, -0.5],
                [-1.0, 0.5, -0.5],
                [-1.0, -0.5, 0.5],
                [1.0, -0.5, 0.5],
                [1.0, 0.5, 0.5],
                [-1.0, 0.5, 0.5],
            ],
            indices: vec![
                0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 2, 3, 7, 2, 7, 6, 0, 4, 7, 0,
                7, 3, 1, 2, 6, 1, 6, 5,
            ],
            uvs: Vec::new(),
            occlusion: Vec::new(),
            volume: 2.0,
            material: Some(material([0.01, 0.005, 0.0025], true)),
            frames: std::collections::BTreeMap::new(),
            animation: Vec::new(),
            behavior: None,
            parent: None,
            bone: None,
            skin: None,
            ik: None,
            body: None,
            transforms: None,
            instance_indices: Vec::new(),
            region: 0,
            baked: None,
        }
    }

    #[test]
    fn emitting_material_uses_area_bounds_and_normalized_colour() {
        let light = emitter(&box_part()).expect("emitting box creates a light");
        assert!(
            light
                .position
                .iter()
                .all(|value| value.abs() < f32::EPSILON)
        );
        assert!((light.intensity - 2.0).abs() < f32::EPSILON);
        let expected = [1.0, linear_to_srgb(0.5), linear_to_srgb(0.25)];
        assert!(
            light
                .color
                .iter()
                .zip(expected)
                .all(|(actual, expected)| (actual - expected).abs() < f32::EPSILON)
        );
        assert_eq!(light.name, "lamp emission");
    }

    #[test]
    fn unmarked_or_black_materials_do_not_create_lights() {
        let mut part = box_part();
        part.material = Some(material([4.0; 3], false));
        assert!(emitter(&part).is_none());
        part.material = Some(material([0.0; 3], true));
        assert!(emitter(&part).is_none());
    }

    #[test]
    fn emitter_intensity_is_bounded_for_large_or_bright_surfaces() {
        let mut part = box_part();
        part.material = Some(material([100.0, 50.0, 25.0], true));
        let light = emitter(&part).expect("bright material still creates a light");
        assert!((light.intensity - 20.0).abs() < f32::EPSILON);
    }
}
