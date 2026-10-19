//! JSON adapter for baking maps from the flat arrays held by the editor.
use crate::{LowMesh, MeasureError, Mesh, bake_ao, bake_normals};

/// Bakes normal and/or ambient-occlusion maps and returns their RGBA bytes as base64 JSON.
#[must_use]
pub fn bake_maps_json(
    low_positions: &[f32],
    low_indices: &[u32],
    low_corner_uvs: &[f32],
    high_positions: &[f32],
    high_indices: &[u32],
    size: u32,
    maps: &str,
    cage: f32,
    max_distance: f32,
    rays: u32,
    bias: f32,
) -> String {
    match bake(
        low_positions,
        low_indices,
        low_corner_uvs,
        high_positions,
        high_indices,
        size,
        maps,
        cage,
        max_distance,
        rays,
        bias,
    ) {
        Ok((normal, ao)) => format!(
            "{{\"ok\":true,\"size\":{size},\"normal\":{},\"ao\":{}}}",
            bake_json(normal.as_ref()),
            bake_json(ao.as_ref())
        ),
        Err(error) => format!(
            "{{\"ok\":false,\"error\":\"{}\",\"suggestion\":\"{}\"}}",
            escape(&error.to_string()),
            suggestion(&error)
        ),
    }
}

fn bake(
    lp: &[f32],
    li: &[u32],
    uv: &[f32],
    hp: &[f32],
    hi: &[u32],
    size: u32,
    maps: &str,
    cage: f32,
    distance: f32,
    rays: u32,
    bias: f32,
) -> Result<(Option<crate::Bake>, Option<crate::Bake>), MeasureError> {
    if !matches!(maps, "normal" | "ao" | "both") {
        return Err(MeasureError::Bake("maps must be normal, ao or both"));
    }
    let lp = triples(lp, "low positions must be xyz triples")?;
    let hp = triples(hp, "high positions must be xyz triples")?;
    if !uv.len().is_multiple_of(2) {
        return Err(MeasureError::Bake("corner uvs must be xy pairs"));
    }
    if uv.is_empty() {
        return Err(MeasureError::MissingUv);
    }
    let uv: Vec<[f32; 2]> = uv.chunks_exact(2).map(|v| [v[0], v[1]]).collect();
    let low = LowMesh::new(&lp, li, &uv)?;
    let high = Mesh::new(&hp, hi);
    let normal = if maps == "ao" {
        None
    } else {
        Some(bake_normals(&low, &high, size, cage, distance)?)
    };
    let ao = if maps == "normal" {
        None
    } else {
        Some(bake_ao(&low, &high, size, rays, distance, bias)?)
    };
    Ok((normal, ao))
}

fn triples(values: &[f32], message: &'static str) -> Result<Vec<[f32; 3]>, MeasureError> {
    if !values.len().is_multiple_of(3) {
        return Err(MeasureError::Bake(message));
    }
    Ok(values.chunks_exact(3).map(|v| [v[0], v[1], v[2]]).collect())
}

fn bake_json(bake: Option<&crate::Bake>) -> String {
    bake.map_or_else(
        || "null".to_string(),
        |b| {
            format!(
                "{{\"covered\":{},\"rgbaBase64\":\"{}\"}}",
                b.covered,
                base64(&b.rgba)
            )
        },
    )
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for shift in [18, 12, 6, 0] {
            out.push(char::from(TABLE[((n >> shift) & 63) as usize]));
        }
        if chunk.len() < 3 {
            out.truncate(out.len() - (3 - chunk.len()));
            out.push_str(&"=".repeat(3 - chunk.len()));
        }
    }
    out
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
fn suggestion(error: &MeasureError) -> &'static str {
    match error {
        MeasureError::MissingUv => {
            "the low mesh has no corner uvs: unwrap it with poly.unwrap_uv first"
        }
        MeasureError::Empty => "the mesh has no triangles: build geometry first",
        MeasureError::NotTriangles(_) => {
            "indices must come in whole triangles (multiples of three)"
        }
        MeasureError::IndexOutOfRange { .. } => {
            "an index pointed past the vertex list: check the part's indices"
        }
        MeasureError::NonFinite(_) => "a vertex coordinate was not finite: check the geometry",
        MeasureError::Bake(_) => {
            "check the bake settings (image side 8 to 2048, rays 1 to 256) and unwrap the low mesh with poly.unwrap_uv"
        }
        _ => "check the mesh and bake settings, then try again",
    }
}
