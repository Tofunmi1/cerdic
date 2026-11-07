//! `poly.import`: the user's own mesh file, as text, becomes an editable polygon mesh (G5).
//!
//! Nothing is fetched and nothing is bundled: the file's text is a parameter of the node, so the document carries it and the result is
//! deterministic. Three plain-text formats are read: Wavefront OBJ (positions, texture coordinates, polygons of any size, negative indices),
//! ASCII PLY (vertex x y z, face lists) and ASCII STL (triangle soup, welded by exact position). Binary files are refused with the way out.

use ve_geometry::{AttrData, Attribute, Domain, PolyMesh};
use ve_graph::Node;
use ve_kernel::ExecutionContext;

use super::convert::value_from_poly;
use super::errors::poly_fail;
use super::params::{number, text};
use crate::{Fail, cache::Value, fail};

/// Most vertices and faces an import may hold (the evaluator's own caps apply after this).
const MAX_VERTICES: usize = 200_000;
const MAX_FACES: usize = 200_000;

/// A parsed file before it becomes a `PolyMesh`.
#[derive(Debug, Default)]
pub(super) struct Raw {
    pub positions: Vec<[f32; 3]>,
    pub sizes: Vec<u32>,
    pub corners: Vec<u32>,
    /// One (u, v) per corner when every corner of the file had a texture coordinate.
    pub uvs: Option<Vec<[f32; 2]>>,
}

fn bad(line: usize, why: impl std::fmt::Display) -> String {
    format!("line {line}: {why}")
}

fn float(token: &str, line: usize) -> Result<f32, String> {
    let value: f32 = token
        .parse()
        .map_err(|_| bad(line, format!("`{token}` is not a number")))?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(bad(line, "a coordinate is not finite"))
    }
}

/// Wavefront OBJ. Groups, materials, normals, smoothing and everything else are ignored; only the shape and UVs are read.
pub(super) fn parse_obj(source: &str) -> Result<Raw, String> {
    let mut raw = Raw::default();
    let mut texcoords: Vec<[f32; 2]> = Vec::new();
    let mut corner_uvs: Vec<[f32; 2]> = Vec::new();
    let mut every_corner_has_uv = true;
    for (index, line) in source.lines().enumerate() {
        let number = index + 1;
        let mut tokens = line.split_whitespace();
        match tokens.next() {
            Some("v") => {
                let xyz: Vec<&str> = tokens.take(3).collect();
                if xyz.len() < 3 {
                    return Err(bad(number, "a vertex `v` needs x y z"));
                }
                if raw.positions.len() >= MAX_VERTICES {
                    return Err(format!("more than {MAX_VERTICES} vertices"));
                }
                raw.positions.push([
                    float(xyz[0], number)?,
                    float(xyz[1], number)?,
                    float(xyz[2], number)?,
                ]);
            }
            Some("vt") => {
                let uv: Vec<&str> = tokens.take(2).collect();
                if uv.len() < 2 {
                    return Err(bad(number, "a texture coordinate `vt` needs u v"));
                }
                texcoords.push([float(uv[0], number)?, float(uv[1], number)?]);
            }
            Some("f") => {
                let before = raw.corners.len();
                for token in tokens {
                    let mut parts = token.split('/');
                    let vertex = resolve(parts.next().unwrap_or(""), raw.positions.len(), number)?;
                    raw.corners.push(vertex);
                    match parts.next().filter(|part| !part.is_empty()) {
                        Some(part) => corner_uvs
                            .push(texcoords[resolve(part, texcoords.len(), number)? as usize]),
                        None => {
                            every_corner_has_uv = false;
                            corner_uvs.push([0.0, 0.0]);
                        }
                    }
                }
                let size = raw.corners.len() - before;
                if size < 3 {
                    return Err(bad(number, "a face needs at least three vertices"));
                }
                if raw.sizes.len() >= MAX_FACES {
                    return Err(format!("more than {MAX_FACES} faces"));
                }
                raw.sizes
                    .push(u32::try_from(size).map_err(|_| bad(number, "a face is too large"))?);
            }
            _ => {}
        }
    }
    if raw.sizes.is_empty() {
        return Err("the file has no faces (`f` lines); a points-only or line-only OBJ cannot be imported as a surface".into());
    }
    if every_corner_has_uv && !texcoords.is_empty() {
        raw.uvs = Some(corner_uvs);
    }
    Ok(raw)
}

/// An OBJ index: 1-based, or negative counting back from the end.
fn resolve(token: &str, count: usize, line: usize) -> Result<u32, String> {
    let value: i64 = token
        .parse()
        .map_err(|_| bad(line, format!("`{token}` is not a vertex index")))?;
    let count_i = i64::try_from(count).unwrap_or(i64::MAX);
    let zero_based = if value > 0 {
        value - 1
    } else if value < 0 {
        count_i + value
    } else {
        -1
    };
    if zero_based < 0 || zero_based >= count_i {
        return Err(bad(
            line,
            format!(
                "index {value} points at a vertex that does not exist (there are {count} so far)"
            ),
        ));
    }
    u32::try_from(zero_based).map_err(|_| bad(line, "index too large"))
}

/// ASCII PLY: `vertex` elements with x y z (other properties are skipped) and `face` elements with a vertex index list.
pub(super) fn parse_ply(source: &str) -> Result<Raw, String> {
    let mut lines = source.lines().enumerate();
    if lines.next().map(|(_, l)| l.trim()) != Some("ply") {
        return Err("a PLY file starts with the line `ply`".into());
    }
    // Header: element counts and the property layout of each.
    struct Element {
        name: String,
        count: usize,
        properties: Vec<(String, bool)>, // (name, is a list)
    }
    let mut elements: Vec<Element> = Vec::new();
    for (index, line) in lines.by_ref() {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["format", "ascii", _] => {}
            ["format", other, _] => {
                return Err(format!(
                    "this PLY is `{other}`; only ascii PLY can be imported (re-save it as ASCII)"
                ));
            }
            ["element", name, count] => elements.push(Element {
                name: (*name).to_owned(),
                count: count
                    .parse()
                    .map_err(|_| bad(index + 1, "an element count is not a number"))?,
                properties: Vec::new(),
            }),
            ["property", "list", ..] => {
                if let Some(last) = elements.last_mut() {
                    last.properties
                        .push((words.last().copied().unwrap_or("").to_owned(), true));
                }
            }
            ["property", _, name] => {
                if let Some(last) = elements.last_mut() {
                    last.properties.push(((*name).to_owned(), false));
                }
            }
            ["end_header"] => break,
            _ => {}
        }
    }
    let mut raw = Raw::default();
    let mut body = lines
        .map(|(i, l)| (i + 1, l))
        .filter(|(_, l)| !l.trim().is_empty());
    for element in &elements {
        for _ in 0..element.count {
            let (number, line) = body
                .next()
                .ok_or("the file ends before all its elements are listed")?;
            let words: Vec<&str> = line.split_whitespace().collect();
            match element.name.as_str() {
                "vertex" => {
                    let at = |name: &str| element.properties.iter().position(|(p, _)| p == name);
                    let (Some(x), Some(y), Some(z)) = (at("x"), at("y"), at("z")) else {
                        return Err("the vertex element needs x, y and z properties".into());
                    };
                    let get = |slot: usize| {
                        words
                            .get(slot)
                            .ok_or_else(|| bad(number, "a vertex line has too few values"))
                            .and_then(|w| float(w, number))
                    };
                    if raw.positions.len() >= MAX_VERTICES {
                        return Err(format!("more than {MAX_VERTICES} vertices"));
                    }
                    raw.positions.push([get(x)?, get(y)?, get(z)?]);
                }
                "face" => {
                    let size: usize = words
                        .first()
                        .and_then(|w| w.parse().ok())
                        .ok_or_else(|| bad(number, "a face line starts with its vertex count"))?;
                    if size < 3 || words.len() < size + 1 {
                        return Err(bad(
                            number,
                            "a face needs at least three vertices and all its indices",
                        ));
                    }
                    for word in &words[1..=size] {
                        let index: usize = word
                            .parse()
                            .map_err(|_| bad(number, format!("`{word}` is not a vertex index")))?;
                        if index >= raw.positions.len() {
                            return Err(bad(
                                number,
                                format!("index {index} points at a vertex that does not exist"),
                            ));
                        }
                        raw.corners.push(
                            u32::try_from(index).map_err(|_| bad(number, "index too large"))?,
                        );
                    }
                    if raw.sizes.len() >= MAX_FACES {
                        return Err(format!("more than {MAX_FACES} faces"));
                    }
                    raw.sizes
                        .push(u32::try_from(size).map_err(|_| bad(number, "a face is too large"))?);
                }
                _ => {}
            }
        }
    }
    if raw.sizes.is_empty() {
        return Err(
            "the PLY has no `face` element; a point cloud cannot be imported as a surface".into(),
        );
    }
    Ok(raw)
}

/// ASCII STL: triangles listed one by one; vertices with identical coordinates are welded so the mesh is connected.
pub(super) fn parse_stl(source: &str) -> Result<Raw, String> {
    let trimmed = source.trim_start();
    if !trimmed.starts_with("solid") {
        return Err("an ASCII STL starts with `solid`; a binary STL cannot be imported (re-save it as ASCII)".into());
    }
    let mut raw = Raw::default();
    let mut seen: std::collections::HashMap<[u32; 3], u32> = std::collections::HashMap::new();
    let mut in_triangle = 0;
    for (index, line) in source.lines().enumerate() {
        let mut tokens = line.split_whitespace();
        if tokens.next() != Some("vertex") {
            continue;
        }
        let xyz: Vec<&str> = tokens.take(3).collect();
        if xyz.len() < 3 {
            return Err(bad(index + 1, "a `vertex` needs x y z"));
        }
        let p = [
            float(xyz[0], index + 1)?,
            float(xyz[1], index + 1)?,
            float(xyz[2], index + 1)?,
        ];
        // -0.0 and 0.0 are the same place.
        let key = [
            (p[0] + 0.0).to_bits(),
            (p[1] + 0.0).to_bits(),
            (p[2] + 0.0).to_bits(),
        ];
        let id = if let Some(&id) = seen.get(&key) {
            id
        } else {
            if raw.positions.len() >= MAX_VERTICES {
                return Err(format!("more than {MAX_VERTICES} vertices"));
            }
            let id =
                u32::try_from(raw.positions.len()).map_err(|_| "too many vertices".to_string())?;
            raw.positions.push(p);
            seen.insert(key, id);
            id
        };
        raw.corners.push(id);
        in_triangle += 1;
        if in_triangle == 3 {
            in_triangle = 0;
            if raw.sizes.len() >= MAX_FACES {
                return Err(format!("more than {MAX_FACES} faces"));
            }
            raw.sizes.push(3);
        }
    }
    if in_triangle != 0 || raw.sizes.is_empty() {
        return Err(
            "the STL has no complete triangles (each `facet` needs three `vertex` lines)".into(),
        );
    }
    Ok(raw)
}

/// `poly.import`.
pub(super) fn run(node: &Node, context: &ExecutionContext) -> Result<Value, Fail> {
    let format = text(node, "format")?;
    let data = text(node, "data")?;
    let scale = number(node, "scale")?;
    let up = text(node, "up")?;
    if data.trim().is_empty() {
        return Err(Box::new(
            ve_graph::Diagnostic::error("import.empty", "poly.import has no data: put the file's text in `data`")
                .at_node(&node.id)
                .suggest("read your own .obj, .ply (ascii) or .stl (ascii) file as text and pass it as `data`"),
        ));
    }
    let parsed = match format {
        "obj" => parse_obj(data),
        "ply" => parse_ply(data),
        "stl" => parse_stl(data),
        other => Err(format!("unknown format {other}")),
    };
    let raw = parsed.map_err(|why| {
        Box::new(
            ve_graph::Diagnostic::error("import.invalid", format!("poly.import: {why}"))
                .at_node(&node.id)
                .suggest("check the file is a text .obj, ascii .ply or ascii .stl, and that `format` matches"),
        )
    })?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "coordinates are stored as f32"
    )]
    let scale = scale as f32;
    let positions: Vec<[f32; 3]> = raw
        .positions
        .iter()
        .map(|&[x, y, z]| {
            // Files authored Z up are turned to this engine's Y up: (x, y, z) -> (x, z, -y).
            let [x, y, z] = if up == "z" { [x, z, -y] } else { [x, y, z] };
            [x * scale, y * scale, z * scale]
        })
        .collect();
    // A Z-up file turned to Y-up keeps its handedness, so the winding stays outward.
    let mut mesh = PolyMesh::new(positions, raw.sizes, raw.corners).map_err(|error| {
        if matches!(error, ve_geometry::GeometryError::PolyRefusal { .. }) {
            poly_fail(node, &error, None)
        } else {
            fail("import.invalid", format!("poly.import: {error}"))
        }
    })?;
    if let Some(uvs) = raw.uvs {
        let (u, v): (Vec<f32>, Vec<f32>) = uvs.into_iter().map(|[u, v]| (u, v)).unzip();
        for (name, data) in [("u", u), ("v", v)] {
            mesh.set_attribute(
                name,
                Attribute {
                    domain: Domain::Corner,
                    data: AttrData::F32(data),
                },
            )
            .map_err(|e| poly_fail(node, &e, None))?;
        }
    }
    value_from_poly(node, mesh, None, context)
}
