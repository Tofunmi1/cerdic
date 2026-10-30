//! Clothes and hair imported from the CC0 MakeHuman library (ADR 0055, amended), stored in the pack as bindings to the body.
//!
//! Each asset is a triangle mesh with per-corner UVs and, for every vertex, the body triangle nearest to it when the asset was made (three template vertex ids and
//! barycentric weights) and the vertex's offset from that surface point in the triangle's own frame (first edge, edge-normal, normal). Evaluating the asset on a
//! body is `sum(weights * body vertices) + offset in the frame of the current body triangle`: the asset follows every phenotype slider.
use crate::Error;
use crate::pack::{Pack, bytes, f32s};

/// One imported garment or hair piece, ready to be fitted to a body.
#[derive(Clone, Debug)]
pub struct Outfit {
    /// `clothes` or `hair`.
    pub kind: String,
    /// Per vertex: the three template (body) vertices of the triangle the vertex is bound to.
    pub ids: Vec<[u32; 3]>,
    /// Per vertex: the barycentric weights on those three body vertices.
    pub bary: Vec<[f32; 3]>,
    /// Per vertex: the offset from the bound surface point along the triangle's first edge, its second axis and its normal, in metres.
    pub local: Vec<[f32; 3]>,
    /// Triangle corners as vertex numbers, three per triangle.
    pub indices: Vec<u32>,
    /// One UV per triangle corner.
    pub uvs: Vec<[f32; 2]>,
    /// Per vertex: the nine bone influences (pack bone number, weight), blended from the body vertices that face the way the vertex does.
    pub skin: Vec<[(u8, f32); 9]>,
}

fn u32s(pack: &Pack, name: &str) -> Vec<u32> {
    bytes(pack, name)
        .chunks_exact(4)
        .map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
        .collect()
}

fn triples<T: Copy>(flat: &[T]) -> Vec<[T; 3]> {
    flat.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
}

impl Pack {
    /// The names of the imported clothes and hair.
    #[must_use]
    pub fn outfit_names(&self) -> Vec<String> {
        self.header.outfits.keys().cloned().collect()
    }

    /// One imported asset, validated against the body it binds to.
    ///
    /// # Errors
    /// The asset's sections are missing, mis-sized, or point outside the body.
    pub fn outfit(&self, name: &str) -> Result<Option<Outfit>, Error> {
        let Some(meta) = self.header.outfits.get(name) else {
            return Ok(None);
        };
        let key = format!("outfit_{name}");
        let section = |suffix: &str| format!("{key}_{suffix}");
        for suffix in [
            "ids",
            "bary",
            "local",
            "indices",
            "uv",
            "skin_bones",
            "skin_weights",
        ] {
            if !self.header.sections.contains_key(&section(suffix)) {
                return Err(Error::InvalidPack(format!("outfit {name} lacks {suffix}")));
            }
        }
        let ids = triples(&u32s(self, &section("ids")));
        let bary = triples(&f32s(self, &section("bary")));
        let local = triples(&f32s(self, &section("local")));
        let indices = u32s(self, &section("indices"));
        let flat_uv = f32s(self, &section("uv"));
        let uvs: Vec<[f32; 2]> = flat_uv.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
        let skin_bone_bytes = bytes(self, &section("skin_bones"));
        let skin_weight_bytes = bytes(self, &section("skin_weights"));
        let skin: Vec<[(u8, f32); 9]> = skin_bone_bytes
            .chunks_exact(9)
            .zip(skin_weight_bytes.chunks_exact(18))
            .map(|(b, w)| {
                std::array::from_fn(|k| {
                    (
                        b[k],
                        f32::from(u16::from_le_bytes([w[2 * k], w[2 * k + 1]])) / 65535.0,
                    )
                })
            })
            .collect();
        let vertices = ids.len();
        if bary.len() != vertices
            || local.len() != vertices
            || skin.len() != vertices
            || indices.len() % 3 != 0
            || uvs.len() != indices.len()
            || ids
                .iter()
                .flatten()
                .any(|&v| v as usize >= self.header.vertex_count)
            || indices.iter().any(|&i| i as usize >= vertices)
        {
            return Err(Error::InvalidPack(format!("outfit {name} is inconsistent")));
        }
        let kind = meta
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("clothes")
            .to_owned();
        Ok(Some(Outfit {
            kind,
            ids,
            bary,
            local,
            indices,
            uvs,
            skin,
        }))
    }
}
