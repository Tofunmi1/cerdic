//! Origin runs: which input each triangle of a boolean result came from.
#![expect(
    clippy::unwrap_used,
    reason = "the helpers unwrap fixtures that are known to be valid"
)]
use ve_kernel::Solid;

/// A cube of the given size centred on `at`, labelled with a fresh origin id.
fn labelled(size: f64, at: [f64; 3]) -> (Solid, u32) {
    let id = Solid::reserve_ids(1);
    let cube = Solid::cube([size; 3], true).unwrap().translate(at).unwrap();
    let mesh = cube.to_mesh().unwrap();
    (Solid::from_mesh_runs(&mesh, &[0], &[id]).unwrap(), id)
}

/// Mean x of the triangles in each run, by id.
fn centroids(solid: &Solid) -> Vec<(u32, f32, usize)> {
    let mesh = solid.to_mesh().unwrap();
    let runs = solid.runs().unwrap();
    let triangles = mesh.indices.len() / 3;
    runs.iter()
        .enumerate()
        .map(|(index, &(first, id))| {
            let end = runs.get(index + 1).map_or(triangles, |next| next.0);
            let mut sum = 0.0;
            for triangle in first..end {
                for corner in 0..3 {
                    sum += mesh.positions[mesh.indices[triangle * 3 + corner] as usize][0];
                }
            }
            let corners = f32::from(u16::try_from((end - first) * 3).unwrap());
            (id, sum / corners, end - first)
        })
        .collect()
}

#[test]
fn a_union_keeps_each_operands_label_on_its_own_faces() {
    let (left, left_id) = labelled(1.0, [-0.6, 0.0, 0.0]);
    let (right, right_id) = labelled(1.0, [0.6, 0.0, 0.0]);
    let joined = left.union(&right).unwrap();
    let runs = centroids(&joined);
    assert_eq!(runs.len(), 2, "two labels survive: {runs:?}");
    let by_id = |id| runs.iter().find(|run| run.0 == id).unwrap();
    assert!(
        by_id(left_id).1 < -0.1,
        "left faces are on the left: {runs:?}"
    );
    assert!(
        by_id(right_id).1 > 0.1,
        "right faces are on the right: {runs:?}"
    );
}

#[test]
fn cut_faces_take_the_cutters_label_and_transforms_keep_labels() {
    let (body, body_id) = labelled(2.0, [0.0, 0.0, 0.0]);
    let (cutter, cutter_id) = labelled(1.0, [0.0, 0.0, 0.0]);
    let hollow = body.difference(&cutter).unwrap();
    let ids = hollow
        .runs()
        .unwrap()
        .iter()
        .map(|run| run.1)
        .collect::<Vec<_>>();
    assert!(
        ids.contains(&body_id) && ids.contains(&cutter_id),
        "{ids:?}"
    );
    let moved = hollow.translate([5.0, 0.0, 0.0]).unwrap();
    assert_eq!(
        moved
            .runs()
            .unwrap()
            .iter()
            .map(|run| run.1)
            .collect::<Vec<_>>(),
        ids,
        "a move keeps the labels"
    );
}

#[test]
fn malformed_runs_are_refused() {
    let mesh = Solid::cube([1.0; 3], true).unwrap().to_mesh().unwrap();
    let id = Solid::reserve_ids(2);
    assert!(
        Solid::from_mesh_runs(&mesh, &[1], &[id]).is_err(),
        "must start at 0"
    );
    assert!(
        Solid::from_mesh_runs(&mesh, &[0, 0], &[id, id + 1]).is_err(),
        "must ascend"
    );
    assert!(
        Solid::from_mesh_runs(&mesh, &[0, 1], &[id + 1, id]).is_err(),
        "ids must ascend"
    );
    assert!(
        Solid::from_mesh_runs(&mesh, &[0, 99], &[id, id + 1]).is_err(),
        "inside the mesh"
    );
}
