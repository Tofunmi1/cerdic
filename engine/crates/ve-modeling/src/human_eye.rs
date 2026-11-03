//! `body.eye`: the two eyes of a `body.human`, layered like a real eye (sclera, iris, pupil, glossy cornea) and sized to the body's own eyeballs.
use std::rc::Rc;

use ve_graph::{Node, Value as Param};
use ve_kernel::Mesh;

use crate::cache::Value;
use crate::regions::Region;
use crate::result::Finish;
use crate::{Fail, Material, MaterialRecipe, fail, fail_suggest};

const LATITUDES: usize = 20;
const LONGITUDES: usize = 32;

/// Most triangles `body.eye` makes (four layers on two eyes, the sclera a whole sphere), for the cost estimate.
pub(crate) const TRIANGLES: usize = 2 * 4 * 2 * LATITUDES * LONGITUDES;

fn number(node: &Node, name: &str) -> Result<f32, Fail> {
    #[expect(clippy::cast_possible_truncation, reason = "ratios are 0 to 1")]
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .map(|v| v as f32)
        .ok_or_else(|| fail("param.type", format!("body.eye {name} must be a number")))
}

fn triple(node: &Node, name: &str) -> Result<[f32; 3], Fail> {
    let Some(Param::List(items)) = node.params.get(name) else {
        return Err(fail(
            "param.type",
            format!("body.eye {name} must be three numbers"),
        ));
    };
    let values: Vec<f64> = items.iter().filter_map(Param::as_f64).collect();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "colours and directions are small"
    )]
    match values.as_slice() {
        [a, b, c] => Ok([*a as f32, *b as f32, *c as f32]),
        _ => Err(fail(
            "param.type",
            format!("body.eye {name} must be three numbers"),
        )),
    }
}

fn surface(color: [f32; 3], roughness: f32, opacity: f32, finish: Option<Finish>) -> Material {
    Material {
        color,
        opacity,
        metallic: 0.0,
        roughness,
        emissive: [0.0; 3],
        emits_light: false,
        recipe: MaterialRecipe::None,
        texture_scale: 1.0,
        texture_tile: 0.0,
        no_repeat: false,
        no_repeat_scale: 1.0,
        texture_size: 0,
        toon: None,
        texture_program: None,
        texture_style: None,
        foliage: None,
        finish,
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: [f32; 3]) -> [f32; 3] {
    let length = dot(a, a).sqrt();
    if length < 1e-6 {
        [0.0, 0.0, 1.0]
    } else {
        [a[0] / length, a[1] / length, a[2] / length]
    }
}

/// A spherical cap around `forward` of angular radius `reach` (pi is the whole sphere), outward-facing, with UVs laid out around the gaze axis.
fn cap(
    centre: [f32; 3],
    radius: f32,
    forward: [f32; 3],
    reach: f32,
) -> (Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>) {
    let up_hint = if forward[1].abs() > 0.95 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let u = unit(cross(up_hint, forward));
    let v = cross(forward, u);
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    for i in 0..=LATITUDES {
        #[expect(clippy::cast_precision_loss, reason = "tiny counts")]
        let theta = reach * i as f32 / LATITUDES as f32;
        for j in 0..=LONGITUDES {
            #[expect(clippy::cast_precision_loss, reason = "tiny counts")]
            let phi = std::f32::consts::TAU * j as f32 / LONGITUDES as f32;
            let (s, c) = (theta.sin(), theta.cos());
            let direction =
                [0, 1, 2].map(|a| s * phi.cos() * u[a] + s * phi.sin() * v[a] + c * forward[a]);
            positions.push([0, 1, 2].map(|a| centre[a] + radius * direction[a]));
            uvs.push([0.5 + 0.5 * s * phi.cos(), 0.5 + 0.5 * s * phi.sin()]);
        }
    }
    let ring = u32::try_from(LONGITUDES + 1).unwrap_or(33);
    let mut indices = Vec::new();
    for i in 0..LATITUDES {
        for j in 0..LONGITUDES {
            let a = u32::try_from(i * (LONGITUDES + 1) + j).unwrap_or(0);
            for tri in [[a, a + ring, a + 1], [a + 1, a + ring, a + ring + 1]] {
                let (p0, p1, p2) = (
                    positions[tri[0] as usize],
                    positions[tri[1] as usize],
                    positions[tri[2] as usize],
                );
                let normal = cross(sub(p1, p0), sub(p2, p0));
                let outward = sub(p0, centre);
                // Skip the degenerate triangles at the pole, and wind the rest outward.
                if dot(normal, normal) < 1e-18 {
                    continue;
                }
                if dot(normal, outward) >= 0.0 {
                    indices.extend_from_slice(&tri);
                } else {
                    indices.extend_from_slice(&[tri[0], tri[2], tri[1]]);
                }
            }
        }
    }
    (positions, uvs, indices)
}

pub(crate) fn build(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "body.eye needs a body.human"))?;
    let Some(rig) = input.human.as_ref() else {
        return Err(fail_suggest(
            "human.not_a_body",
            "body.eye needs the body.human node itself as its input",
            "connect the body.human node directly, before solid.material and transforms, and give body.eye its own output",
        ));
    };
    // The gaze is in the body's space, so it turns with a turned person.
    let local = triple(node, "gaze")?;
    let (sin, cos) = rig.turn.sin_cos();
    let gaze = unit([
        local[0] * cos + local[2] * sin,
        local[1],
        -local[0] * sin + local[2] * cos,
    ]);
    let iris = number(node, "iris_size")?.clamp(0.1, 0.9);
    let pupil = number(node, "pupil_size")?.clamp(0.05, 0.95);
    let (iris_color, sclera_color) = (triple(node, "iris_color")?, triple(node, "sclera_color")?);
    let cornea = Finish {
        clearcoat: 1.0,
        clearcoat_roughness: 0.03,
        anisotropy: 0.0,
        translucency: 0.0,
        relief: 0.0,
        // Unused with no relief, but the renderer only accepts a scale from 0.1 to 500.
        relief_scale: 10.0,
        relief_style: 0.0,
        wetness: 0.0,
        iridescence: 0.0,
        sheen: 0.0,
        subsurface: 0.0,
        thickness: 0.0,
        terrain: None,
        water: None,
    };
    let layers = [
        // Radius multiplier, angular reach, material. The caps sit just proud of each other so nothing z-fights.
        (
            1.012,
            std::f32::consts::PI,
            surface(sclera_color, 0.32, 1.0, None),
        ),
        (1.016, iris.asin(), surface(iris_color, 0.3, 1.0, None)),
        (
            1.019,
            (iris * pupil).asin(),
            surface([0.01, 0.01, 0.012], 0.2, 1.0, None),
        ),
        (
            1.07,
            (iris * 1.35).min(0.99).asin(),
            surface([1.0; 3], 0.02, 0.1, Some(cornea)),
        ),
    ];
    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut regions = Vec::new();
    for (scale, reach, material) in layers {
        regions.push(Region {
            first_triangle: indices.len() / 3,
            material: Some(material),
        });
        for (centre, radius) in rig.eyes {
            if radius <= 0.0 {
                continue;
            }
            let base = u32::try_from(positions.len()).unwrap_or(0);
            let (p, uv, i) = cap(centre, radius * scale, gaze, reach);
            positions.extend(p);
            indices.extend(i.into_iter().map(|index| index + base));
            uvs.extend(uv);
        }
    }
    if positions.is_empty() {
        return Err(fail("human.no_eyes", "this body has no eyes to build"));
    }
    let corner_uvs: Vec<[f32; 2]> = indices.iter().map(|&i| uvs[i as usize]).collect();
    let mut value = Value::solid(Mesh { positions, indices }, 0.0);
    value.uvs = corner_uvs;
    value.material = regions.first().and_then(|r| r.material.clone());
    value.regions = regions;
    Ok(value)
}
