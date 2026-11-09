//! Trunk and limb shape: smooth curved limbs, the lobed root flare and the bark seam.
#![expect(clippy::unwrap_used, clippy::float_cmp, reason = "fixtures unwrap valid data and compare exact seam coordinates")]
use std::collections::BTreeSet;

use ve_world::flora::{Preset, Skeleton, colonize, grow_tree, preset};
use ve_world::lod::{LeafCards, LodError, Meshing, PlantMesh, Surface, mesh_plant};

const NEVER: &dyn Fn() -> bool = &|| false;

fn oak() -> Skeleton {
    match preset("oak", 8.0, 3).unwrap() {
        Preset::Tree(tree) => grow_tree(&tree, NEVER).unwrap(),
        Preset::Colonize(setup) => colonize(&setup, NEVER).unwrap(),
    }
}

fn settings(smooth: u32, flare: f32) -> Meshing {
    Meshing {
        radial_segments: 8,
        max_segment_length: 0.6,
        bark_uv_scale: [0.5, 1.0],
        leaf_cards: LeafCards::Quads { aspect: 0.6, bend_deg: 20.0 },
        fork_blend: 0.5,
        smooth,
        flare,
    }
}

/// The vertices the bark triangles use.
fn bark_vertices(mesh: &PlantMesh) -> BTreeSet<u32> {
    let mut used = BTreeSet::new();
    for range in mesh.material_ranges.iter().filter(|r| r.kind == Surface::Bark) {
        let (first, count) = (range.first_index as usize, range.index_count as usize);
        used.extend(mesh.indices[first..first + count].iter().copied());
    }
    used
}

/// The widest the bark reaches from the trunk axis among vertices within `height` of the soil.
fn root_spread(mesh: &PlantMesh, height: f32) -> f32 {
    bark_vertices(mesh)
        .iter()
        .map(|&i| mesh.vertices[i as usize].position)
        .filter(|p| p[1] < height)
        .map(|p| (p[0] * p[0] + p[2] * p[2]).sqrt())
        .fold(0.0, f32::max)
}

#[test]
fn a_smooth_limb_has_more_rings_but_stays_inside_the_same_crown() {
    let skeleton = oak();
    let plain = mesh_plant(&skeleton, &settings(0, 0.0), NEVER).unwrap();
    let smooth = mesh_plant(&skeleton, &settings(3, 0.0), NEVER).unwrap();
    assert!(bark_vertices(&smooth).len() > bark_vertices(&plain).len() * 2, "rings were added along every limb");
    let extent = |mesh: &PlantMesh| {
        mesh.vertices.iter().fold([f32::MIN; 3], |m, v| [m[0].max(v.position[0]), m[1].max(v.position[1]), m[2].max(v.position[2])])
    };
    let (a, b) = (extent(&plain), extent(&smooth));
    for axis in 0..3 {
        assert!((a[axis] - b[axis]).abs() < 0.6, "axis {axis}: the curve rounds corners, it does not move the crown ({} against {})", a[axis], b[axis]);
    }
}

#[test]
fn the_trunk_swells_at_the_soil_and_not_higher_up() {
    let skeleton = oak();
    let plain = mesh_plant(&skeleton, &settings(0, 0.0), NEVER).unwrap();
    let flared = mesh_plant(&skeleton, &settings(0, 2.0), NEVER).unwrap();
    let (at_soil_plain, at_soil_flared) = (root_spread(&plain, 0.05), root_spread(&flared, 0.05));
    assert!(at_soil_flared > at_soil_plain * 1.8, "the roots spread: {at_soil_flared} against {at_soil_plain}");
    // two metres up the trunk is the same cylinder either way
    let high = |mesh: &PlantMesh| {
        bark_vertices(mesh)
            .iter()
            .map(|&i| mesh.vertices[i as usize].position)
            .filter(|p| p[1] > 2.0 && p[1] < 2.3)
            .map(|p| (p[0] * p[0] + p[2] * p[2]).sqrt())
            .fold(0.0, f32::max)
    };
    assert!((high(&plain) - high(&flared)).abs() < 0.02, "no swell at head height");
}

#[test]
fn every_bark_ring_repeats_its_first_vertex_with_u_one_so_the_texture_does_not_smear() {
    let mesh = mesh_plant(&oak(), &settings(1, 1.0), NEVER).unwrap();
    let used = bark_vertices(&mesh);
    let seam: Vec<[f32; 3]> = used.iter().map(|&i| &mesh.vertices[i as usize]).filter(|v| v.uv[0] == 1.0).map(|v| v.position).collect();
    assert!(!seam.is_empty(), "there are seam vertices");
    for position in seam {
        assert!(
            used.iter().any(|&i| {
                let v = &mesh.vertices[i as usize];
                v.uv[0] == 0.0 && v.position == position
            }),
            "a seam vertex at {position:?} has its twin at u = 0"
        );
    }
}

#[test]
fn settings_out_of_range_are_refused_by_name() {
    let skeleton = oak();
    assert!(matches!(mesh_plant(&skeleton, &settings(7, 0.0), NEVER), Err(LodError::BadParameter { name: "smooth", .. })));
    assert!(matches!(mesh_plant(&skeleton, &settings(0, 3.5), NEVER), Err(LodError::BadParameter { name: "flare", .. })));
    assert!(matches!(mesh_plant(&skeleton, &settings(0, f32::NAN), NEVER), Err(LodError::BadParameter { name: "flare", .. })));
}
