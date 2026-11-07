//! Value inspector and trace: what each node of a program produced and why. `Evaluator::trace` runs a document and, for every node in the order it ran, records the operator and
//! its parameters, which nodes fed it, what kind of value came out (a solid, a polygon mesh, a number, a selection, a texture-less field, a volume, a terrain tile ...), how big it
//! is, and what the node changed from its main input (the box grew from 1 to 3 m wide, the triangle count went from 12 to 96, the selection holds 13 vertices). A model that
//! asks "why is this part 4 m tall" or "why is this selection empty" reads the chain instead of guessing.

use serde::Serialize;
use serde_json::{Map, Value as Json, json};

use crate::cache::Value;
use crate::poly_ops::Elements;

/// One node's record.
#[derive(Clone, Debug, Serialize)]
pub struct NodeTrace {
    /// The node's id.
    pub id: String,
    /// The name the program gave it, if any.
    pub label: Option<String>,
    /// The operator family and version, such as `poly.grid@1`.
    pub operator: String,
    /// The parameters it ran with.
    pub params: Json,
    /// The nodes that fed it, by input port.
    pub inputs: Vec<InputTrace>,
    /// What came out: solid, poly, number, selection, field, pattern, volume, terrain, curve, light, environment, game, profile or nothing.
    pub kind: String,
    /// One line saying what the value is.
    pub summary: String,
    /// The numbers behind the summary (triangles, bounds, size, volume, faces, tracks, layers, ...).
    pub facts: Json,
    /// What the node changed from its first geometry input (empty for a node with none).
    pub effect: Vec<String>,
    /// Warnings this node raised.
    pub warnings: Vec<String>,
}

/// A link into a node.
#[derive(Clone, Debug, Serialize)]
pub struct InputTrace {
    /// The input port's name.
    pub port: String,
    /// The label (or id) of the node that fed it.
    pub from: String,
}

fn bounds(value: &Value) -> Option<([f32; 3], [f32; 3])> {
    let mut points = value.mesh.positions.iter();
    let first = points.next()?;
    let (mut lo, mut hi) = (*first, *first);
    for p in points {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    Some((lo, hi))
}

fn round(v: f32) -> f64 {
    (f64::from(v) * 1000.0).round() / 1000.0
}

fn triple(v: [f32; 3]) -> Json {
    json!([round(v[0]), round(v[1]), round(v[2])])
}

/// What kind of value this is, a line about it and the numbers.
pub(crate) fn describe(value: &Value) -> (&'static str, String, Json) {
    let mut facts = Map::new();
    let kind;
    let mut summary;
    if let Some(n) = value.number {
        kind = "number";
        summary = format!("the number {}", (n * 1.0e6).round() / 1.0e6);
        facts.insert("number".into(), json!(n));
    } else if let Some(selection) = &value.selection {
        kind = "selection";
        let what = match &selection.elements {
            Elements::Faces(_) => "faces",
            Elements::Edges(_) => "edges",
            Elements::Vertices(_) => "vertices",
            Elements::Loops(_) => "boundary loops",
        };
        summary = format!("a selection of {} {what}", selection.elements.len());
        facts.insert("elements".into(), json!(what));
        facts.insert("count".into(), json!(selection.elements.len()));
    } else if value.pattern.is_some() {
        kind = "pattern";
        summary = "a pattern: a function of position that scatter and masks sample".to_owned();
    } else if value.field.is_some() && value.mesh.positions.is_empty() {
        kind = "field";
        summary = "a spatial field (not meshed yet)".to_owned();
    } else if let Some(volume) = &value.sdf {
        kind = "volume";
        summary = format!(
            "a volume of {} x {} x {} voxels of {} m",
            volume.dims[0],
            volume.dims[1],
            volume.dims[2],
            (volume.cell * 1.0e4).round() / 1.0e4
        );
        facts.insert("dims".into(), json!(volume.dims));
        facts.insert("voxel".into(), json!(volume.cell));
    } else if let Some(tile) = &value.grid {
        kind = "terrain";
        let layers = tile.grid.layer_names();
        summary = format!(
            "a terrain tile of {} x {} cells with layers [{}]",
            tile.grid.width(),
            tile.grid.depth(),
            layers.join(", ")
        );
        facts.insert("layers".into(), json!(layers));
    } else if value.light.is_some() {
        kind = "light";
        summary = "a light".to_owned();
    } else if value.environment.is_some() {
        kind = "environment";
        summary = "a scene environment".to_owned();
    } else if value.game.is_some() {
        kind = "game";
        summary = "a fragment of the game definition".to_owned();
    } else if value.mesh.triangle_count() > 0 {
        let poly = value.poly.is_some();
        kind = if poly { "poly" } else { "solid" };
        let triangles = value.mesh.triangle_count();
        summary = format!(
            "a {} of {triangles} triangles",
            if poly { "polygon mesh" } else { "solid" }
        );
        facts.insert("triangles".into(), json!(triangles));
        facts.insert("vertices".into(), json!(value.mesh.positions.len()));
        if let Some(poly) = &value.poly {
            facts.insert("faces".into(), json!(poly.face_sizes().len()));
            facts.insert("points".into(), json!(poly.positions().len()));
        }
        if let Some((lo, hi)) = bounds(value) {
            let size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
            summary.push_str(&format!(
                ", {} x {} x {} m",
                round(size[0]),
                round(size[1]),
                round(size[2])
            ));
            facts.insert("min".into(), triple(lo));
            facts.insert("max".into(), triple(hi));
            facts.insert("size".into(), triple(size));
        }
        if value.volume > 0.0 {
            facts.insert(
                "volume".into(),
                json!((value.volume * 1.0e6).round() / 1.0e6),
            );
        }
    } else if value.curve.is_some() {
        kind = "curve";
        summary = format!(
            "a curve of {} points",
            value.curve.as_ref().map_or(0, |c| c.points.len())
        );
    } else if !value.loops.is_empty() {
        kind = "profile";
        summary = format!("a profile of {} loops", value.loops.len());
    } else {
        kind = "nothing";
        summary = "nothing (no geometry)".to_owned();
    }
    if let Some(instances) = &value.instances {
        summary.push_str(&format!("; {} placed copies", instances.transforms.len()));
        facts.insert("copies".into(), json!(instances.transforms.len()));
    }
    let mut notes: Vec<String> = Vec::new();
    if !value.animation.is_empty() {
        notes.push(format!("{} animation tracks", value.animation.len()));
        facts.insert("tracks".into(), json!(value.animation.len()));
    }
    if let Some(bone) = value.bone {
        notes.push("a bone".to_owned());
        facts.insert(
            "bone".into(),
            json!({"head": triple(bone.head), "tail": triple(bone.tail)}),
        );
    }
    if value.skin.is_some() {
        notes.push("skinned".to_owned());
    }
    if value.ik.is_some() {
        notes.push("an IK target".to_owned());
    }
    if value.cloth.is_some() {
        notes.push("live cloth".to_owned());
    }
    if value.fluid.is_some() {
        notes.push("a live liquid".to_owned());
    }
    if value.sim.is_some() {
        notes.push("a simulation program".to_owned());
    }
    if value.body.is_some() {
        notes.push("a physics body".to_owned());
    }
    if value.parent.is_some() {
        notes.push(format!(
            "parented to {}",
            value.parent.as_deref().unwrap_or("")
        ));
    }
    if let Some(material) = &value.material {
        facts.insert(
            "material".into(),
            json!({"color": material.color, "roughness": material.roughness}),
        );
        notes.push("has a material".to_owned());
    }
    if value.baked.is_some() {
        notes.push("baked maps".to_owned());
    }
    if !notes.is_empty() {
        summary.push_str(&format!("; {}", notes.join(", ")));
    }
    (kind, summary, Json::Object(facts))
}

/// What `value` changed from its main input `before`, as short sentences a person can check.
pub(crate) fn effect(before: Option<&Value>, value: &Value) -> Vec<String> {
    let Some(before) = before else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let (a, b) = (before.mesh.triangle_count(), value.mesh.triangle_count());
    if a != b {
        out.push(format!(
            "triangles {a} to {b} ({}{})",
            if b >= a { "+" } else { "-" },
            a.abs_diff(b)
        ));
    }
    if let (Some((alo, ahi)), Some((blo, bhi))) = (bounds(before), bounds(value)) {
        for (axis, name) in ["x", "y", "z"].iter().enumerate() {
            let (sa, sb) = (ahi[axis] - alo[axis], bhi[axis] - blo[axis]);
            if (sa - sb).abs() > 1.0e-4 {
                out.push(format!("size {name} {} to {} m", round(sa), round(sb)));
            }
            let (ca, cb) = ((ahi[axis] + alo[axis]) * 0.5, (bhi[axis] + blo[axis]) * 0.5);
            if (ca - cb).abs() > 1.0e-4 {
                out.push(format!("centre {name} {} to {} m", round(ca), round(cb)));
            }
        }
    } else if bounds(before).is_none() && bounds(value).is_some() {
        out.push("made geometry from none".to_owned());
    } else if bounds(before).is_some() && bounds(value).is_none() {
        out.push("removed all geometry".to_owned());
    }
    if before.animation.len() != value.animation.len() {
        out.push(format!(
            "animation tracks {} to {}",
            before.animation.len(),
            value.animation.len()
        ));
    }
    if before.material.is_none() && value.material.is_some() {
        out.push("gave it a material".to_owned());
    }
    if before.parent != value.parent && value.parent.is_some() {
        out.push(format!(
            "parented it to {}",
            value.parent.as_deref().unwrap_or("")
        ));
    }
    if out.is_empty() {
        out.push("left the geometry as it was".to_owned());
    }
    out
}
