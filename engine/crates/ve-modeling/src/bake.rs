//! `bake.maps`: bake a high-poly part's tangent-space normal map and ambient occlusion onto a low-poly part's UVs.
//!
//! The result is the low part itself (same mesh, UVs, material) with the baked images attached, so a bake travels with the
//! part through moves, materials and export. The work is `ve_measure::bake_normals` and `bake_ao`.

use std::rc::Rc;

use ve_graph::{Node, Value as Param};
use ve_measure::{LowMesh, Mesh, bake_ao, bake_normals};

use crate::cache::Value;
use crate::{BakedMaps, Fail, fail_suggest};

/// Most rays a bake may cast, `size * size * rays` for the occlusion map: 512 x 512 at 244 rays, 1024 x 1024 at 61.
const MAX_RAYS: u64 = 64_000_000;

fn number(node: &Node, name: &str) -> f64 {
    node.params.get(name).and_then(Param::as_f64).unwrap_or(0.0)
}

fn whole(node: &Node, name: &str) -> u32 {
    u32::try_from(match node.params.get(name) {
        Some(Param::Int(n)) => *n,
        _ => 0,
    })
    .unwrap_or(0)
}

/// The length of the bounding-box diagonal of a mesh, a scale for the default settings.
fn diagonal(positions: &[[f32; 3]]) -> f32 {
    let (mut low, mut high) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for point in positions {
        for axis in 0..3 {
            low[axis] = low[axis].min(point[axis]);
            high[axis] = high[axis].max(point[axis]);
        }
    }
    (0..3)
        .map(|axis| (high[axis] - low[axis]).powi(2))
        .sum::<f32>()
        .sqrt()
}

fn measured(node: &Node, error: &ve_measure::MeasureError) -> Fail {
    fail_suggest(
        "bake.failed",
        format!("{}: {error}", node.id),
        "check that both parts are closed solids and the low part is unwrapped (poly.unwrap_uv)",
    )
}

/// Runs a `bake.maps` node, or returns `None` for any other operator. `inputs` follow the port names in order, so
/// `high` comes before `low`.
pub(crate) fn run(node: &Node, inputs: &[Rc<Value>]) -> Option<Result<Value, Fail>> {
    (node.op.family().as_str() == "bake.maps").then(|| bake(node, inputs))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "the settings are metres and small counts, well inside f32"
)]
fn bake(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let [high, low] = inputs else {
        return Err(fail_suggest(
            "graph.missing_input",
            format!("{} needs a low and a high part", node.id),
            "link both the low and the high input",
        ));
    };
    if low.mesh.is_empty() || high.mesh.is_empty() {
        return Err(fail_suggest(
            "bake.nothing_to_bake",
            format!("{}: one of the parts has no geometry", node.id),
            "link the nodes that make the low-poly and high-poly parts",
        ));
    }
    if low.uvs.len() != low.mesh.indices.len() {
        return Err(fail_suggest(
            "bake.no_uvs",
            format!("{}: the low part has no UV layout to bake onto", node.id),
            "unwrap the low part first: poly.unwrap_uv, then poly.pack_uv, then poly.to_solid",
        ));
    }
    let (size, rays) = (whole(node, "size"), whole(node, "rays"));
    let maps = match node.params.get("maps") {
        Some(Param::Str(text)) => text.as_str(),
        _ => "both",
    };
    let (want_normal, want_ao) = (maps != "ao", maps != "normal");
    if want_ao && u64::from(size) * u64::from(size) * u64::from(rays) > MAX_RAYS {
        return Err(fail_suggest(
            "budget.bake",
            format!(
                "{}: size {size} with {rays} rays casts more than {MAX_RAYS} rays",
                node.id
            ),
            "lower size or rays (512 with 64 rays is fine), or bake only the normal map with maps=\"normal\"",
        ));
    }
    let scale = diagonal(&low.mesh.positions);
    let setting = |name: &str, fraction: f32| {
        let given = number(node, name) as f32;
        if given > 0.0 { given } else { scale * fraction }
    };
    let (cage, distance, bias) = (
        setting("cage", 0.05),
        setting("max_distance", 0.1),
        setting("bias", 0.001),
    );
    let low_mesh = LowMesh::new(&low.mesh.positions, &low.mesh.indices, &low.uvs)
        .map_err(|error| measured(node, &error))?;
    let high_mesh = Mesh::new(&high.mesh.positions, &high.mesh.indices);
    let normal = if want_normal {
        Some(
            bake_normals(&low_mesh, &high_mesh, size, cage, distance)
                .map_err(|error| measured(node, &error))?
                .rgba,
        )
    } else {
        None
    };
    let ao = if want_ao {
        Some(
            bake_ao(&low_mesh, &high_mesh, size, rays, distance, bias)
                .map_err(|error| measured(node, &error))?
                .rgba,
        )
    } else {
        None
    };
    let mut value = (**low).clone();
    value.baked = Some(std::sync::Arc::new(BakedMaps { size, normal, ao }));
    Ok(value)
}
