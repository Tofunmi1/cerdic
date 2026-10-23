//! Validated access to the compact binary body pack.
use std::io::Read;

use serde::Deserialize;

use crate::Error;

#[derive(Deserialize)]
pub(crate) struct Header {
    pub vertex_count: usize,
    pub render_vertex_count: usize,
    pub rank: usize,
    pub phenotype_labels: Vec<String>,
    pub phenotype_variations: serde_json::Map<String, serde_json::Value>,
    pub anchors: serde_json::Map<String, serde_json::Value>,
    pub local_change_labels: Vec<String>,
    pub facial_action_labels: Vec<String>,
    pub bone_labels: Vec<String>,
    pub sections: std::collections::BTreeMap<String, Section>,
    /// Imported clothes and hair (name to its description); empty in a pack without them.
    #[serde(default)]
    pub outfits: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
pub(crate) struct Section {
    offset: usize,
    dtype: String,
    shape: Vec<usize>,
}

/// A decompressed, validated pack.
pub struct Pack {
    pub(crate) header: Header,
    pub(crate) payload: Vec<u8>,
}

impl Pack {
    /// Decompress and validate a version 1 pack supplied as bytes.
    ///
    /// # Errors
    /// Returns an error for malformed compression, headers, sections, or indices.
    pub fn from_zstd(bytes: &[u8]) -> Result<Self, Error> {
        let mut input = bytes;
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(&mut input)
            .map_err(|e| Error::Compression(e.to_string()))?;
        let mut raw = Vec::new();
        decoder
            .read_to_end(&mut raw)
            .map_err(|e| Error::Compression(e.to_string()))?;
        if raw.len() < 12 || &raw[..8] != b"VEHUMAN\x01" {
            return Err(Error::InvalidPack("bad magic or truncated header".into()));
        }
        let header_len = u32::from_le_bytes(
            raw[8..12]
                .try_into()
                .map_err(|_| Error::InvalidPack("truncated header length".into()))?,
        ) as usize;
        let end = 12usize
            .checked_add(header_len)
            .ok_or_else(|| Error::InvalidPack("header length overflow".into()))?;
        let header: Header = serde_json::from_slice(
            raw.get(12..end)
                .ok_or_else(|| Error::InvalidPack("truncated JSON header".into()))?,
        )
        .map_err(|e| Error::InvalidPack(e.to_string()))?;
        let payload = raw
            .get(end..)
            .ok_or_else(|| Error::InvalidPack("missing payload".into()))?
            .to_vec();
        validate(&header, &payload)?;
        Ok(Self { header, payload })
    }
}

fn validate(h: &Header, p: &[u8]) -> Result<(), Error> {
    validate_section_table(h, p)?;
    validate_phenotype_metadata(h)?;
    let required: &[(&str, &str, &[usize])] = &[
        ("template", "float32", &[h.vertex_count, 3]),
        ("phenotype_basis", "int16", &[h.rank, h.vertex_count, 3]),
        ("phenotype_basis_scale", "float32", &[h.rank]),
        ("phenotype_coefficients", "float32", &[624, h.rank]),
        ("phenotype_mask", "uint8", &[624, 26]),
        ("face_offsets", "uint32", &[53]),
        ("face_scales", "float32", &[52]),
        ("local_offsets", "uint32", &[513]),
        ("local_scales", "float32", &[512]),
        ("render_source", "uint32", &[h.render_vertex_count]),
        ("render_uv", "float32", &[h.render_vertex_count, 2]),
        ("bone_heads", "float32", &[104, 3]),
        ("bone_head_shapes", "float32", &[1188, 104, 3]),
        ("bone_parents", "int32", &[104]),
        ("skin_bones", "uint8", &[h.vertex_count, 9]),
        ("skin_weights", "uint16", &[h.vertex_count, 9]),
    ];
    for (name, dtype, shape) in required {
        section(h, p, name, dtype, shape)?;
    }
    validate_sparse(h, p, "face", 52)?;
    validate_sparse(h, p, "local", 512)?;
    validate_topology(h, p)?;
    validate_indices(h, p)?;
    if h.phenotype_labels.len() != 11
        || h.local_change_labels.len() != 256
        || h.facial_action_labels.len() != 52
        || h.bone_labels.len() != 104
    {
        return Err(Error::InvalidPack("unexpected label counts".into()));
    }
    Ok(())
}

fn validate_phenotype_metadata(h: &Header) -> Result<(), Error> {
    let mut columns = 0usize;
    for (feature, names) in &h.phenotype_variations {
        let labels = names.as_array().ok_or_else(|| {
            Error::InvalidPack(format!("invalid phenotype variations for {feature}"))
        })?;
        columns += labels.len();
        if feature != "race" {
            let values = h
                .anchors
                .get(feature)
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| {
                    Error::InvalidPack(format!("missing phenotype anchors for {feature}"))
                })?;
            if values.len() != labels.len() || values.len() < 2 {
                return Err(Error::InvalidPack(format!(
                    "invalid phenotype anchors for {feature}"
                )));
            }
            let anchors: Vec<f64> = values
                .iter()
                .map(|value| {
                    value.as_f64().ok_or_else(|| {
                        Error::InvalidPack(format!("non-numeric phenotype anchor for {feature}"))
                    })
                })
                .collect::<Result<_, _>>()?;
            if anchors.iter().any(|value| !value.is_finite())
                || anchors.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(Error::InvalidPack(format!(
                    "unordered phenotype anchors for {feature}"
                )));
            }
        }
    }
    if columns != 26
        || h.phenotype_variations
            .get("race")
            .and_then(serde_json::Value::as_array)
            .is_none_or(|labels| labels.len() != 3)
        || h.anchors.len() + 1 != h.phenotype_variations.len()
    {
        return Err(Error::InvalidPack("unexpected phenotype metadata".into()));
    }
    Ok(())
}

fn validate_sparse(h: &Header, p: &[u8], prefix: &str, count: usize) -> Result<(), Error> {
    let offsets = section(h, p, &format!("{prefix}_offsets"), "uint32", &[count + 1])?;
    let scales = section(h, p, &format!("{prefix}_scales"), "float32", &[count])?;
    let index_section = h
        .sections
        .get(&format!("{prefix}_indices"))
        .ok_or_else(|| Error::InvalidPack(format!("missing section {prefix}_indices")))?;
    if index_section.dtype != "uint32" || index_section.shape.len() != 1 {
        return Err(Error::InvalidPack(format!(
            "bad shape or type for {prefix}_indices"
        )));
    }
    let nnz = index_section.shape[0];
    let indices = section(h, p, &format!("{prefix}_indices"), "uint32", &[nnz])?;
    let deltas = section(h, p, &format!("{prefix}_deltas"), "int16", &[nnz, 3])?;
    let _ = (scales, deltas);
    let offsets: Vec<u32> = offsets
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();
    if offsets.first() != Some(&0)
        || offsets.last().copied() != u32::try_from(nnz).ok()
        || offsets.windows(2).any(|pair| pair[0] > pair[1])
    {
        return Err(Error::InvalidPack(format!("invalid {prefix} offsets")));
    }
    validate_index_values(indices, h.vertex_count, &format!("{prefix}_indices"))
}

fn validate_topology(h: &Header, p: &[u8]) -> Result<(), Error> {
    for (name, width, limit) in [
        ("triangles", 3, h.render_vertex_count),
        ("quads", 4, h.vertex_count),
    ] {
        let s = h
            .sections
            .get(name)
            .ok_or_else(|| Error::InvalidPack(format!("missing section {name}")))?;
        if s.dtype != "uint32" || s.shape.len() != 2 || s.shape[1] != width {
            return Err(Error::InvalidPack(format!("bad shape or type for {name}")));
        }
        let data = section(h, p, name, "uint32", &s.shape)?;
        validate_index_values(data, limit, name)?;
    }
    Ok(())
}

fn validate_section_table(h: &Header, p: &[u8]) -> Result<(), Error> {
    let mut ranges = Vec::with_capacity(h.sections.len());
    for (name, s) in &h.sections {
        let width = match s.dtype.as_str() {
            "float32" | "uint32" | "int32" => 4,
            "int16" | "uint16" => 2,
            "uint8" => 1,
            _ => return Err(Error::InvalidPack(format!("unknown dtype {}", s.dtype))),
        };
        if s.offset % 4 != 0 {
            return Err(Error::InvalidPack(format!("unaligned section {name}")));
        }
        let count = s
            .shape
            .iter()
            .try_fold(1usize, |n, &dim| n.checked_mul(dim))
            .ok_or_else(|| Error::InvalidPack(format!("section size overflow for {name}")))?;
        let len = count
            .checked_mul(width)
            .ok_or_else(|| Error::InvalidPack(format!("section size overflow for {name}")))?;
        let end = s
            .offset
            .checked_add(len)
            .ok_or_else(|| Error::InvalidPack(format!("section offset overflow for {name}")))?;
        if end > p.len() {
            return Err(Error::InvalidPack(format!(
                "section {name} outside payload"
            )));
        }
        ranges.push((s.offset, end, name));
    }
    ranges.sort_unstable_by_key(|range| range.0);
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(Error::InvalidPack("overlapping sections".into()));
    }
    Ok(())
}

fn validate_indices(h: &Header, p: &[u8]) -> Result<(), Error> {
    validate_index_values(
        section(h, p, "render_source", "uint32", &[h.render_vertex_count])?,
        h.vertex_count,
        "render_source",
    )?;
    let bones = section(h, p, "skin_bones", "uint8", &[h.vertex_count, 9])?;
    if bones.iter().any(|&b| b >= 104) {
        return Err(Error::InvalidPack(
            "skin bone index outside bone table".into(),
        ));
    }
    let parents = section(h, p, "bone_parents", "int32", &[104])?;
    if parents.chunks_exact(4).any(|b| {
        let i = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        !(-1..104).contains(&i)
    }) {
        return Err(Error::InvalidPack(
            "bone parent index outside bone table".into(),
        ));
    }
    Ok(())
}

fn validate_index_values(data: &[u8], limit: usize, name: &str) -> Result<(), Error> {
    if data
        .chunks_exact(4)
        .any(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize >= limit)
    {
        return Err(Error::InvalidPack(format!(
            "index outside valid range in {name}"
        )));
    }
    Ok(())
}

fn section<'a>(
    h: &Header,
    p: &'a [u8],
    n: &str,
    dtype: &str,
    shape: &[usize],
) -> Result<&'a [u8], Error> {
    let s = h
        .sections
        .get(n)
        .ok_or_else(|| Error::InvalidPack(format!("missing section {n}")))?;
    if s.dtype != dtype || s.shape != shape {
        return Err(Error::InvalidPack(format!("bad shape or type for {n}")));
    }
    let width = match dtype {
        "float32" | "uint32" | "int32" => 4,
        "int16" | "uint16" => 2,
        "uint8" => 1,
        _ => return Err(Error::InvalidPack(format!("unknown dtype {dtype}"))),
    };
    let count = shape
        .iter()
        .try_fold(1usize, |a, &b| a.checked_mul(b))
        .ok_or_else(|| Error::InvalidPack("section size overflow".into()))?;
    let len = count
        .checked_mul(width)
        .ok_or_else(|| Error::InvalidPack("section size overflow".into()))?;
    p.get(
        s.offset
            ..s.offset
                .checked_add(len)
                .ok_or_else(|| Error::InvalidPack("section offset overflow".into()))?,
    )
    .ok_or_else(|| Error::InvalidPack(format!("section {n} outside payload")))
}

pub(crate) fn bytes<'a>(pack: &'a Pack, n: &str) -> &'a [u8] {
    let s = &pack.header.sections[n];
    let len = s.shape.iter().product::<usize>()
        * match s.dtype.as_str() {
            "float32" | "uint32" | "int32" => 4,
            "int16" | "uint16" => 2,
            _ => 1,
        };
    &pack.payload[s.offset..s.offset + len]
}
pub(crate) fn f32s(p: &Pack, n: &str) -> Vec<f32> {
    bytes(p, n)
        .chunks_exact(4)
        .map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::Pack;

    #[test]
    fn byte_corruption_never_panics() {
        let original = include_bytes!("../../../assets/human/human.pack");
        let stride = (original.len() / 17).max(1);
        for index in (0..original.len()).step_by(stride) {
            let mut corrupted = original.to_vec();
            corrupted[index] ^= 0x5a;
            let outcome = std::panic::catch_unwind(|| Pack::from_zstd(&corrupted));
            assert!(
                outcome.is_ok(),
                "loader panicked after changing byte {index}"
            );
        }
    }
}
