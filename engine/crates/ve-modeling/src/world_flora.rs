//! `flora.tree`: a real tree from a named species (ADR 0056; growth by `ve_world::flora`, meshing and levels of detail by `ve_world::lod`).
//!
//! The result is one solid with two material regions, so it plants into `scatter.place` like any shape: bark (a tube mesh with
//! pixel-shader relief) and leaves (cards cut out by a procedural leaf shape that is drawn once and shared by every copy).
use ve_graph::{Node, Value as Param};
use ve_kernel::{ExecutionContext, Mesh};
use ve_world::flora::{Preset, colonize, grow_tree, preset, simplify};
use ve_world::lod::{LeafCards, LodError, Meshing, PlantMesh, Surface, mesh_plant};

use crate::appearance::textured_surface_default;
use crate::finish::Finish;
use crate::regions::{Region, apply};
use crate::result::{Foliage, Material};
use crate::world_ops::text;
use crate::world_terrain::param;
use crate::{Fail, cache::Value, fail, fail_suggest};

/// Triangles the node may make at level 0, for the cost estimate: a full 8 m oak is about 2,600, so this leaves room for taller trees
/// and the leafier species without refusing a forest of five kinds planted on four tiles (each placement node counts its shape once).
const FULL_TRIANGLES: usize = 12_000;

/// How many leaves of the species' own size one drawn leaf card stands for (a card is a small cluster of leaves).
const CLUSTER: f32 = 12.0;

pub(crate) fn run(node: &Node, context: &ExecutionContext) -> Option<Result<Value, Fail>> {
    (node.op.family().as_str() == "flora.tree").then(|| build(node, context))
}

/// The triangles the node will make: the full tree's estimate, halved per level of detail.
pub(crate) fn estimate(node: &Node) -> usize {
    let level = param(node, "lod").map_or(0, |v| v as u32);
    FULL_TRIANGLES >> level.min(8)
}

fn colour(node: &Node, name: &str) -> Result<[f32; 3], Fail> {
    let Some(Param::List(items)) = node.params.get(name) else {
        return Err(fail(
            "param.type",
            format!("flora.tree {name} must be three numbers"),
        ));
    };
    let values: Vec<f64> = items.iter().filter_map(Param::as_f64).collect();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "colour channels are 0 to 1"
    )]
    match values.as_slice() {
        [r, g, b] => Ok([*r as f32, *g as f32, *b as f32]),
        _ => Err(fail(
            "param.type",
            format!("flora.tree {name} must be three numbers"),
        )),
    }
}

fn lod_error(error: LodError) -> Fail {
    match error {
        LodError::Cancelled => fail("eval.cancelled", "tree meshing cancelled"),
        LodError::TooLarge { .. } => fail_suggest(
            "budget.triangles",
            error.to_string(),
            "lower `sides`, use a smaller tree or a higher `lod`",
        ),
        other => fail("flora.failed", other.to_string()),
    }
}

fn flora_error(error: ve_world::flora::FloraError) -> Fail {
    use ve_world::flora::FloraError;
    match error {
        FloraError::Cancelled => fail("eval.cancelled", "tree growth cancelled"),
        FloraError::BadParameter { name, why } => fail_suggest(
            "param.invalid",
            format!("flora.tree {name}: {why}"),
            "use one of oak, pine, birch, willow, shrub, palm",
        ),
        other => fail("flora.failed", other.to_string()),
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "parameters are small bounded numbers"
)]
fn build(node: &Node, context: &ExecutionContext) -> Result<Value, Fail> {
    let cancel = || context.is_cancelled();
    let (height, seed) = (param(node, "height")? as f32, param(node, "seed")? as u32);
    let grown = match preset(text(node, "kind"), height, seed).map_err(flora_error)? {
        Preset::Tree(tree) => grow_tree(&tree, &cancel).map_err(flora_error)?,
        Preset::Colonize(setup) => colonize(&setup, &cancel).map_err(flora_error)?,
    };
    // a level of detail is the same tree grown once and pruned: thin twigs cut, rings thinned, fewer but larger leaf cards
    let level = param(node, "lod")? as u32;
    let mut skeleton = simplify(&grown, level);
    if text(node, "season") == "winter" {
        skeleton.leaves.clear();
    }
    // A leaf point stands for a cluster of leaves, drawn as a few cards: the species size is one leaf, so it is scaled up to a cluster
    // (`leaf_size` 1 is a natural full crown).
    let cluster = param(node, "leaf_size")? as f32 * CLUSTER;
    for leaf in &mut skeleton.leaves {
        leaf.size *= cluster;
    }
    let aspect = 0.6;
    let leaf_cards = match text(node, "leaf_style") {
        "crossed" => LeafCards::Crossed { aspect },
        "quads" => LeafCards::Quads {
            aspect,
            bend_deg: 18.0,
        },
        _ => LeafCards::Clumps {
            leaves: 3,
            spread: 38.0,
            aspect,
        },
    };
    let meshing = Meshing {
        // each level halves the sides of the trunk and branches as well as the leaves and rings, so a level is about half the weight of the one before
        radial_segments: ((param(node, "sides")? as u32) >> level.min(5)).max(3),
        max_segment_length: (height / 12.0).max(0.2),
        bark_uv_scale: [0.5, 1.0],
        leaf_cards,
        fork_blend: 0.5,
        // smoothing and the root flare cost triangles and are invisible far away: they thin out with the level of detail
        smooth: (param(node, "smooth")? as u32) >> level.min(3),
        flare: if level < 2 { param(node, "flare")? as f32 } else { 0.0 },
    };
    let plant = mesh_plant(&skeleton, &meshing, &cancel).map_err(lod_error)?;
    tree_value(node, &plant)
}

/// The leaf shape and colour, as a procedural texture program (drawn once, shared by every copy of the tree).
/// The leaf of a species as numbers: half length and half width on a card, how deeply its edge is lobed and in how many lobes, how
/// much it narrows toward the tip, and how many fall on one card.
struct LeafShape {
    length: f32,
    width: f32,
    lobe: f32,
    lobes: f32,
    taper: f32,
    count: u32,
}

fn leaf_shape(kind: &str) -> LeafShape {
    match kind {
        "oak" => LeafShape { length: 0.17, width: 0.1, lobe: 0.24, lobes: 5.0, taper: 0.1, count: 44 },
        "birch" => LeafShape { length: 0.15, width: 0.095, lobe: 0.05, lobes: 11.0, taper: 0.55, count: 52 },
        "willow" => LeafShape { length: 0.3, width: 0.04, lobe: 0.0, lobes: 1.0, taper: 0.35, count: 56 },
        "pine" => LeafShape { length: 0.27, width: 0.04, lobe: 0.0, lobes: 1.0, taper: 0.5, count: 64 },
        "palm" => LeafShape { length: 0.46, width: 0.05, lobe: 0.0, lobes: 1.0, taper: 0.25, count: 18 },
        _ => LeafShape { length: 0.15, width: 0.09, lobe: 0.1, lobes: 3.0, taper: 0.3, count: 44 },
    }
}

/// The leaf shape and colour of a species, as a procedural texture program (drawn once, shared by every copy of the tree).
fn leaf_program(kind: &str, colour: [f32; 3]) -> String {
    let [r, g, b] = colour;
    let shape = leaf_shape(kind);
    // A card is a spray of leaves, each at its own spot, angle, size and shade; the leaves painted later cover the earlier ones. Each leaf is
    // darker at its edge, and on broadleaf species a few lean yellow, so a crown is not one green. The card's alpha is where any leaf is.
    // The texture compiler allows 160 syntax nodes, 64 loop turns and spells the two-argument arctangent `atan`, so this is as much as one
    // expression holds: the lobe ripple (a sine of the position across the leaf) and the yellow shift are left out where a species has none.
    let wave = if shape.lobe > 0.0 {
        format!(" * (1.0 + {} * sin(d.y * {} + hash(vec2(i, 6.1)) * 6.283))", shape.lobe, shape.lobes * 12.0)
    } else {
        String::new()
    };
    let (shift, red, green) = if shape.count <= 56 {
        ("ylw = step(0.82, hash(vec2(i, 4.4))) * 0.22; ", " + ylw", " + ylw * 0.7")
    } else {
        ("", "", "")
    };
    format!(
        "n = expr(code=\"col = vec3(0.0); a = 0.0; for i in 0..{count} {{ c = vec2(hash(vec2(i, 1.3)), hash(vec2(i, 7.1))) * 1.7 - 0.85; d = rot(q - c, hash(vec2(i, 3.7)) * 6.283) / (0.8 + 0.4 * hash(vec2(i, 2.2))); nar = 1.0 - {taper} * clamp(d.x / {length}, 0.0, 1.0); w = length(vec2(d.x / {length}, d.y / ({width} * nar))){wave}; inside = 1.0 - step(1.0, w); kk = (0.8 + 0.9 * hash(vec2(i, 9.9))) * (1.0 - 0.3 * smoothstep(0.55, 1.0, w)); {shift}col = mix(col, vec3({r:.3} * kk{red}, {g:.3} * kk{green}, {b:.3} * kk), inside); a = max(a, inside) }}; out = vec4(col, a)\"); albedo = n",
        count = shape.count.min(64),
        length = shape.length,
        width = shape.width,
        taper = shape.taper,
    )
}

/// The leaf colour for a time of year: the default green becomes the season's colour, any colour chosen by hand stays.
fn seasonal(colour: [f32; 3], season: &str) -> [f32; 3] {
    let is_default = (colour[0] - 0.2).abs() < 1.0e-3 && (colour[1] - 0.42).abs() < 1.0e-3 && (colour[2] - 0.12).abs() < 1.0e-3;
    if !is_default {
        return colour;
    }
    match season {
        "spring" => [0.42, 0.62, 0.2],
        "autumn" => [0.74, 0.34, 0.07],
        _ => colour,
    }
}

/// The three tones of a bark pattern, darkest first, as hex colours.
fn bark_palette(kind: &str, custom: Option<[f32; 3]>) -> [String; 3] {
    if let Some([r, g, b]) = custom {
        let tone = |scale: f32| {
            let c = |v: f32| (255.0 * (v * scale).clamp(0.0, 1.0)).round() as u8;
            format!("#{:02x}{:02x}{:02x}", c(r), c(g), c(b))
        };
        return [tone(0.4), tone(0.85), tone(1.4)];
    }
    let [d, m, l] = match kind {
        "birch" => ["#26221d", "#d6d0c1", "#f6f2e8"],
        "pine" => ["#2a1a10", "#573824", "#8a5c3a"],
        "willow" => ["#3a342a", "#6b5f4c", "#a39580"],
        "shrub" => ["#4a3722", "#755a3c", "#a2825c"],
        "palm" => ["#5a4a30", "#8f7850", "#c8ae7e"],
        _ => ["#2c1d10", "#5e4529", "#947552"],
    };
    [d.to_owned(), m.to_owned(), l.to_owned()]
}

/// The bark of a species as a procedural texture program: `u` runs round the trunk and `v` along it, so a pattern stretched along `v` is
/// ridges and one stretched round `u` is rings and marks.
fn bark_program(kind: &str, style: &str, custom: Option<[f32; 3]>) -> String {
    let [d, m, l] = bark_palette(kind, custom);
    let style = if style == "auto" {
        match kind {
            "birch" => "white",
            "pine" => "plated",
            "palm" => "rings",
            "shrub" => "smooth",
            _ => "furrowed",
        }
    } else {
        style
    };
    match style {
        "white" => format!(
            "n = fbm(scale_x=2.5, scale_y=26, iterations=4); f = fbm(scale_x=14, scale_y=6, iterations=3); c = colorize(n, gradient=[0:{l}, 0.45:{m}, 0.6:{d}, 0.68:{d}, 0.74:{m}, 1:{l}]); albedo = c; roughness = colorize(f, gradient=[0:#707070, 1:#b8b8b8]); height = n"
        ),
        "plated" => format!(
            "v = voronoi(scale_x=5, scale_y=12, randomness=1.0); f = fbm(scale_x=18, scale_y=18, iterations=3); c = colorize(v, gradient=[0:{d}, 0.22:{m}, 1:{l}]); albedo = c; roughness = colorize(f, gradient=[0:#808080, 1:#c0c0c0]); height = v"
        ),
        "rings" => format!(
            "n = fbm(scale_x=1.5, scale_y=22, iterations=3); f = fbm(scale_x=20, scale_y=20, iterations=3); c = colorize(n, gradient=[0:{d}, 0.5:{m}, 1:{l}]); albedo = c; roughness = colorize(f, gradient=[0:#808080, 1:#c0c0c0]); height = n"
        ),
        "smooth" => format!(
            "n = fbm(scale_x=4, scale_y=8, iterations=3); c = colorize(n, gradient=[0:{d}, 0.6:{m}, 1:{l}]); albedo = c; roughness = colorize(n, gradient=[0:#909090, 1:#c0c0c0]); height = n"
        ),
        _ => format!(
            "n = fbm(scale_x=7, scale_y=2.5, iterations=5); f = fbm(scale_x=30, scale_y=9, iterations=3); c = colorize(n, gradient=[0:{d}, 0.5:{m}, 1:{l}]); albedo = c; roughness = colorize(f, gradient=[0:#808080, 1:#c0c0c0]); height = n"
        ),
    }
}

fn tree_value(node: &Node, plant: &PlantMesh) -> Result<Value, Fail> {
    let positions: Vec<[f32; 3]> = plant.vertices.iter().map(|v| v.position).collect();
    let uvs: Vec<[f32; 2]> = plant
        .indices
        .iter()
        .map(|i| plant.vertices[*i as usize].uv)
        .collect();
    let mut value = Value::solid(
        Mesh {
            positions,
            indices: plant.indices.clone(),
        },
        0.0,
    );
    value.uvs = uvs;
    let sway = param(node, "sway")? as f32;
    let (bark_colour, leaf_colour) = (colour(node, "bark")?, seasonal(colour(node, "leaves")?, text(node, "season")));
    let kind = text(node, "kind");
    // the default bark colour means "the species' own"; any other colour recolours the pattern
    let custom = ((bark_colour[0] - 0.3).abs() > 1.0e-3 || (bark_colour[1] - 0.2).abs() > 1.0e-3 || (bark_colour[2] - 0.13).abs() > 1.0e-3).then_some(bark_colour);
    let bark = Material {
        color: [1.0; 3],
        roughness: 0.95,
        texture_program: Some(bark_program(kind, text(node, "bark_style"), custom)),
        texture_size: 512,
        // pixel-shader relief needs the tangents of the full mesh; the simplified levels have none that hold, and at that distance it is invisible
        finish: (param(node, "lod")? < 0.5).then(|| Finish {
            relief: 0.6,
            relief_scale: 5.0,
            relief_style: 1.0,
            ..Finish::plain()
        }),
        ..textured_surface_default(None)
    };
    let leaf = Material {
        color: [1.0; 3],
        roughness: 0.75,
        texture_program: Some(leaf_program(kind, leaf_colour)),
        texture_size: 256,
        foliage: Some(Foliage {
            alpha_cutoff: 0.5,
            sway,
            flutter: sway * 0.6,
            // sunlight glowing through the leaves, and a lift so a crown in shade is not black
            translucency: 0.4,
            exposure_boost: 1.3,
            ..Foliage::default()
        }),
        ..textured_surface_default(None)
    };
    let regions: Vec<Region> = plant
        .material_ranges
        .iter()
        .map(|range| Region {
            first_triangle: range.first_index as usize / 3,
            material: Some(match range.kind {
                Surface::Bark => bark.clone(),
                Surface::Leaf => leaf.clone(),
            }),
        })
        .collect();
    apply(&mut value, regions);
    Ok(value)
}
