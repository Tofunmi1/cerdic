//! Native-testable collection and measurement of evaluated output meshes.
use std::collections::BTreeMap;
use ve_measure::{MAX_ATTACHMENT_TRIANGLES, MAX_PARTS, Mesh, NamedMesh, attachments_json};

/// Borrowed mesh data from one evaluated material region.
pub(crate) struct Region<'a> {
    pub name: &'a str,
    pub positions: &'a [[f32; 3]],
    pub indices: &'a [u32],
    pub transforms: Option<&'a [[f32; 16]]>,
}

/// Measures region meshes, merging identical output names into a single output mesh.
pub(crate) fn report(regions: &[Region<'_>], tolerance: f32) -> String {
    let mut merged: BTreeMap<String, (Vec<[f32; 3]>, Vec<u32>)> = BTreeMap::new();
    for region in regions {
        if region.transforms.is_some_and(|matrices| matrices.len() > 1) {
            continue;
        }
        let mesh = merged.entry(region.name.to_owned()).or_default();
        let offset = mesh.0.len() as u32;
        if let Some(matrix) = region.transforms.and_then(|matrices| matrices.first()) {
            mesh.0.extend(region.positions.iter().map(|point| {
                [
                    matrix[0] * point[0] + matrix[4] * point[1] + matrix[8] * point[2] + matrix[12],
                    matrix[1] * point[0] + matrix[5] * point[1] + matrix[9] * point[2] + matrix[13],
                    matrix[2] * point[0]
                        + matrix[6] * point[1]
                        + matrix[10] * point[2]
                        + matrix[14],
                ]
            }));
        } else {
            mesh.0.extend_from_slice(region.positions);
        }
        mesh.1
            .extend(region.indices.iter().map(|index| index + offset));
    }
    merged.retain(|_, (_, indices)| indices.len() >= 12);
    let parts = merged.len();
    let triangles: usize = merged.values().map(|(_, indices)| indices.len() / 3).sum();
    if parts > MAX_PARTS {
        return serde_json::json!({"error":format!("attachment part budget exceeded: {parts} > {MAX_PARTS}")}).to_string();
    }
    if triangles > MAX_ATTACHMENT_TRIANGLES {
        return serde_json::json!({"error":format!("attachment triangle budget exceeded: {triangles} > {MAX_ATTACHMENT_TRIANGLES}")}).to_string();
    }
    if parts < 2 {
        return serde_json::json!({"floating":[],"parts":parts}).to_string();
    }
    let names: Vec<_> = merged.keys().map(String::as_str).collect();
    let meshes: Vec<_> = merged
        .values()
        .map(|(positions, indices)| Mesh::new(positions, indices))
        .collect();
    let named: Vec<_> = names
        .iter()
        .zip(&meshes)
        .map(|(name, mesh)| NamedMesh { name, mesh: *mesh })
        .collect();
    let measured: serde_json::Value =
        serde_json::from_str(&attachments_json(&named, tolerance)).unwrap_or_default();
    if let Some(error) = measured.get("error") {
        return serde_json::json!({"error":error}).to_string();
    }
    serde_json::json!({"floating":measured["floating"],"parts":parts}).to_string()
}

#[cfg(test)]
mod tests {
    use super::{Region, report};
    use serde_json::Value;

    const POSITIONS: [[f32; 3]; 8] = [
        [0., 0., 0.],
        [1., 0., 0.],
        [1., 1., 0.],
        [0., 1., 0.],
        [0., 0., 1.],
        [1., 0., 1.],
        [1., 1., 1.],
        [0., 1., 1.],
    ];
    const INDICES: [u32; 36] = [
        0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6,
        3, 0, 4, 3, 4, 7,
    ];
    const fn moved(x: f32) -> [f32; 16] {
        let mut matrix = [0.; 16];
        matrix[0] = 1.;
        matrix[5] = 1.;
        matrix[10] = 1.;
        matrix[15] = 1.;
        matrix[12] = x;
        matrix
    }
    const LEFT: [[f32; 16]; 1] = [moved(0.)];
    const TOUCHING: [[f32; 16]; 1] = [moved(1.)];
    const SEPARATED: [[f32; 16]; 1] = [moved(3.5)];
    fn region<'a>(name: &'a str, transforms: &'static [[f32; 16]]) -> Region<'a> {
        Region {
            name,
            positions: &POSITIONS,
            indices: &INDICES,
            transforms: Some(transforms),
        }
    }
    #[test]
    fn touching_boxes_are_attached_and_separated_boxes_report_the_gap() {
        let touching = [region("left", &LEFT), region("right", &TOUCHING)];
        let value: Value = serde_json::from_str(&report(&touching, 0.02)).expect("report JSON");
        assert_eq!(value["floating"], serde_json::json!([]));
        let separated = [
            region("left", &LEFT),
            region("right", &TOUCHING),
            region("floating", &SEPARATED),
        ];
        let value: Value = serde_json::from_str(&report(&separated, 0.02)).expect("report JSON");
        assert_eq!(value["floating"].as_array().map(Vec::len), Some(1));
        assert_eq!(value["floating"][0]["name"], "floating");
        assert!((value["floating"][0]["gap"].as_f64().unwrap_or_default() - 1.5).abs() < 0.01);
    }
    #[test]
    fn material_regions_with_one_name_count_as_one_part() {
        let regions = [region("body", &LEFT), region("body", &LEFT)];
        let value: Value = serde_json::from_str(&report(&regions, 0.02)).expect("report JSON");
        assert_eq!(value["parts"], 1);
        assert_eq!(value["floating"], serde_json::json!([]));
    }
}
