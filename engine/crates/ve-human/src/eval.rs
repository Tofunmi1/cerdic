//! Named parameter validation and deterministic mesh evaluation.
use crate::{
    Error, Pack,
    pack::{bytes, f32s},
};
use std::collections::BTreeMap;

/// Named body controls. Missing values use the reference model defaults.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Params {
    /// Phenotype controls.
    pub phenotype: BTreeMap<String, f32>,
    /// Signed local shape controls.
    pub local: BTreeMap<String, f32>,
    /// Facial action controls.
    pub face: BTreeMap<String, f32>,
}
/// Evaluated body geometry and skeleton heads.
#[derive(Clone, Debug)]
pub struct Body {
    /// Body vertex positions in metres.
    pub positions: Vec<[f32; 3]>,
    /// UV seam split positions.
    pub render_positions: Vec<[f32; 3]>,
    /// Deformed bone head positions.
    pub bone_heads: Vec<[f32; 3]>,
}
/// Render topology and UVs.
pub struct RenderMesh {
    /// Body vertex for each render vertex.
    pub source: Vec<u32>,
    /// UV coordinate for each render vertex.
    pub uv: Vec<[f32; 2]>,
    /// Triangle indices into render vertices.
    pub triangles: Vec<[u32; 3]>,
}
/// Loaded human model.
pub struct Human {
    pack: Pack,
    mesh: RenderMesh,
    pub(crate) quads: Vec<[u32; 4]>,
    skin: Vec<[(u8, f32); 9]>,
}

impl Human {
    fn load_quads(pack: &Pack) -> Vec<[u32; 4]> {
        u32s(pack, "quads")
            .chunks_exact(4)
            .map(|x| [x[0], x[1], x[2], x[3]])
            .collect()
    }
    /// Construct an evaluator from a validated pack.
    #[must_use]
    pub fn new(pack: Pack) -> Self {
        let source = u32s(&pack, "render_source");
        let uvv = f32s(&pack, "render_uv");
        let tri = u32s(&pack, "triangles");
        let mesh = RenderMesh {
            source,
            uv: uvv.chunks_exact(2).map(|x| [x[0], x[1]]).collect(),
            triangles: tri.chunks_exact(3).map(|x| [x[0], x[1], x[2]]).collect(),
        };
        let bones = bytes(&pack, "skin_bones");
        let weights = bytes(&pack, "skin_weights");
        let skin = (0..pack.header.vertex_count)
            .map(|i| {
                std::array::from_fn(|j| {
                    let k = i * 9 + j;
                    (
                        bones[k],
                        f32::from(u16::from_le_bytes([weights[k * 2], weights[k * 2 + 1]]))
                            / 65535.0,
                    )
                })
            })
            .collect();
        let quads = Self::load_quads(&pack);
        Self {
            pack,
            mesh,
            quads,
            skin,
        }
    }
    /// Number of source body vertices.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.pack.header.vertex_count
    }
    /// Bone names in pack order.
    #[must_use]
    pub fn bone_names(&self) -> &[String] {
        &self.pack.header.bone_labels
    }
    /// Parent bone indices; root is -1.
    #[must_use]
    pub fn bone_parents(&self) -> Vec<i32> {
        i32s(&self.pack, "bone_parents")
    }
    /// Valid phenotype control names.
    #[must_use]
    pub fn phenotype_labels(&self) -> &[String] {
        &self.pack.header.phenotype_labels
    }
    /// Valid signed local shape names.
    #[must_use]
    pub fn local_change_labels(&self) -> &[String] {
        &self.pack.header.local_change_labels
    }
    /// Valid facial action names.
    #[must_use]
    pub fn face_action_labels(&self) -> &[String] {
        &self.pack.header.facial_action_labels
    }
    /// The names of the imported clothes and hair the pack carries.
    #[must_use]
    pub fn outfit_names(&self) -> Vec<String> {
        self.pack.outfit_names()
    }
    /// One imported garment or hair piece, bound to the body (see [`Outfit`](crate::Outfit)).
    ///
    /// # Errors
    /// The asset is damaged.
    pub fn outfit(&self, name: &str) -> Result<Option<crate::Outfit>, Error> {
        self.pack.outfit(name)
    }
    /// Per-vertex nine bone influences.
    #[must_use]
    pub fn skin(&self) -> &[[(u8, f32); 9]] {
        &self.skin
    }
    /// The body's quad faces, as indices into the body vertices (not the UV-split render vertices).
    #[must_use]
    pub fn quads(&self) -> &[[u32; 4]] {
        &self.quads
    }
    /// UV-split render topology.
    #[must_use]
    pub fn render_mesh(&self) -> &RenderMesh {
        &self.mesh
    }
    /// Evaluate named controls; omitted controls use defaults.
    ///
    /// # Errors
    /// Returns an error for unknown controls or values outside their supported range.
    pub fn evaluate(&self, p: &Params) -> Result<Body, Error> {
        validate_map(
            &p.phenotype,
            self.phenotype_labels(),
            "phenotype",
            0.0,
            1.0,
            "0..=1",
        )?;
        validate_map(
            &p.face,
            self.face_action_labels(),
            "face",
            0.0,
            1.0,
            "0..=1",
        )?;
        validate_map(
            &p.local,
            self.local_change_labels(),
            "local",
            -1.0,
            1.0,
            "-1..=1",
        )?;
        let (mut positions, phenotype_weights) = evaluate_phenotype(&self.pack, &p.phenotype);
        add_sparse(
            &self.pack,
            "face",
            &p.face,
            self.face_action_labels(),
            &mut positions,
            false,
        );
        add_sparse(
            &self.pack,
            "local",
            &p.local,
            self.local_change_labels(),
            &mut positions,
            true,
        );
        let bone_heads = evaluate_bones(&self.pack, &p.face, &p.local, &phenotype_weights);
        let render_positions = self
            .mesh
            .source
            .iter()
            .map(|&i| positions[i as usize])
            .collect();
        Ok(Body {
            positions,
            render_positions,
            bone_heads,
        })
    }
}

fn evaluate_phenotype(
    pack: &Pack,
    parameters: &BTreeMap<String, f32>,
) -> (Vec<[f32; 3]>, Vec<f32>) {
    let mut positions = triples(pack, "template");
    let mut coeff = vec![0.0; pack.header.rank];
    let columns: Vec<f32> = pack
        .header
        .phenotype_variations
        .iter()
        .flat_map(|(feature, names)| {
            let names = names.as_array().map_or(&[][..], Vec::as_slice);
            if feature == "race" {
                let vals = ["african", "asian", "caucasian"]
                    .map(|n| parameters.get(n).copied().unwrap_or(0.5));
                let sum = vals.iter().sum::<f32>();
                vals.into_iter()
                    .map(|v| if sum == 0.0 { 1.0 / 3.0 } else { v / sum })
                    .collect::<Vec<_>>()
            } else {
                let x = parameters.get(feature).copied().unwrap_or(0.5);
                let a: Vec<f32> = pack.header.anchors[feature]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(serde_json::Value::as_f64)
                    .map(f64_to_f32)
                    .collect();
                let w = hat_weights(x, &a);
                names
                    .iter()
                    .enumerate()
                    .map(|(i, _)| w.get(i).copied().unwrap_or(0.0))
                    .collect()
            }
        })
        .collect();
    let mask = bytes(pack, "phenotype_mask");
    let c = f32s(pack, "phenotype_coefficients");
    let mut phenotype_weights = vec![0.0; 624];
    for k in 0..624 {
        let w = (0..26)
            .map(|j| {
                columns.get(j).copied().unwrap_or(0.0) * f32::from(mask[k * 26 + j])
                    + (1.0 - f32::from(mask[k * 26 + j]))
            })
            .product::<f32>();
        phenotype_weights[k] = w;
        if w != 0.0 {
            for r in 0..coeff.len() {
                coeff[r] += w * c[k * coeff.len() + r];
            }
        }
    }
    let basis = bytes(pack, "phenotype_basis");
    let scales = f32s(pack, "phenotype_basis_scale");
    for r in 0..coeff.len() {
        if coeff[r] != 0.0 {
            for v in 0..positions.len() {
                for a in 0..3 {
                    let i = (r * positions.len() + v) * 3 + a;
                    let q = i * 2;
                    let val = i16::from_le_bytes([basis[q], basis[q + 1]]);
                    positions[v][a] += coeff[r] * f32::from(val) * scales[r];
                }
            }
        }
    }
    (positions, phenotype_weights)
}

fn evaluate_bones(
    pack: &Pack,
    face: &BTreeMap<String, f32>,
    local: &BTreeMap<String, f32>,
    phenotype_weights: &[f32],
) -> Vec<[f32; 3]> {
    let mut bone_heads = triples(pack, "bone_heads");
    let bone_count = bone_heads.len();
    let shape_values = f32s(pack, "bone_head_shapes");
    for (b, head) in bone_heads.iter_mut().enumerate() {
        for (a, coordinate) in head.iter_mut().enumerate() {
            for k in 0..624 {
                *coordinate += shape_values[(k * bone_count + b) * 3 + a] * phenotype_weights[k];
            }
            for k in 624..1188 {
                let weight = if k < 676 {
                    face.get(&pack.header.facial_action_labels[k - 624])
                        .copied()
                        .unwrap_or(0.0)
                } else {
                    let name = &pack.header.local_change_labels[(k - 676) / 2];
                    let v = local.get(name).copied().unwrap_or(0.0);
                    if (k - 676) % 2 == 0 {
                        v.max(0.0)
                    } else {
                        (-v).max(0.0)
                    }
                };
                *coordinate += shape_values[(k * bone_count + b) * 3 + a] * weight;
            }
        }
    }
    bone_heads
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "anchor coordinates are bounded model controls stored as JSON f64"
)]
fn f64_to_f32(value: f64) -> f32 {
    value as f32
}
fn validate_map(
    m: &BTreeMap<String, f32>,
    labels: &[String],
    kind: &'static str,
    lo: f32,
    hi: f32,
    range: &'static str,
) -> Result<(), Error> {
    for (n, &v) in m {
        if !labels.contains(n) {
            return Err(Error::UnknownParameter {
                kind,
                name: n.clone(),
                valid: labels.join(", "),
            });
        }
        if !v.is_finite() || v < lo || v > hi {
            return Err(Error::OutOfRange {
                kind,
                name: n.clone(),
                value: v,
                range,
            });
        }
    }
    Ok(())
}
fn hat_weights(x: f32, a: &[f32]) -> Vec<f32> {
    let mut w = vec![0.0; a.len()];
    if a.len() < 2 {
        return w;
    }
    let i = a.partition_point(|v| *v < x).clamp(1, a.len() - 1);
    let alpha = (x - a[i - 1]) / (a[i] - a[i - 1]);
    w[i - 1] = 1.0 - alpha;
    w[i] = alpha;
    w
}

fn add_sparse(
    pack: &Pack,
    prefix: &str,
    m: &BTreeMap<String, f32>,
    labels: &[String],
    out: &mut [[f32; 3]],
    signed: bool,
) {
    let offsets = u32s(pack, &format!("{prefix}_offsets"));
    let indices = u32s(pack, &format!("{prefix}_indices"));
    let deltas = bytes(pack, &format!("{prefix}_deltas"));
    let scales = f32s(pack, &format!("{prefix}_scales"));
    for (name, &v) in m {
        if v == 0.0 {
            continue;
        }
        let i = labels.iter().position(|x| x == name).unwrap_or(0);
        let shape = if signed && v < 0.0 {
            2 * i + 1
        } else if signed {
            2 * i
        } else {
            i
        };
        let w = if signed { v.abs() } else { v };
        for (k, &vertex_index) in indices
            .iter()
            .enumerate()
            .take(offsets[shape + 1] as usize)
            .skip(offsets[shape] as usize)
        {
            let vertex = vertex_index as usize;
            for (a, coordinate) in out[vertex].iter_mut().enumerate() {
                let q = (k * 3 + a) * 2;
                *coordinate +=
                    w * f32::from(i16::from_le_bytes([deltas[q], deltas[q + 1]])) * scales[shape];
            }
        }
    }
}
mod read;
use read::{i32s, triples, u32s};

#[cfg(test)]
mod tests;
