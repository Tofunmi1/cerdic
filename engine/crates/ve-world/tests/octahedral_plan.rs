//! Atlas tiling, bake camera orientation, and projected bounds coverage.
use ve_world::lod::{
    LodError, PlantMesh, Vertex, WindData, octahedral::frame_direction, octahedral_impostor_plan,
};

fn box_mesh(minimum: [f32; 3], maximum: [f32; 3]) -> PlantMesh {
    let vertices = (0..8)
        .map(|corner| Vertex {
            position: core::array::from_fn(|axis| {
                if corner & (1 << axis) == 0 {
                    minimum[axis]
                } else {
                    maximum[axis]
                }
            }),
            normal: [0.0, 1.0, 0.0],
            uv: [0.0; 2],
            tangent: [1.0, 0.0, 0.0, 1.0],
            wind: WindData {
                pivot: [0.0; 3],
                phase: 0.0,
                trunk: 0.0,
                branch: 0.0,
                leaf: 0.0,
                edge: 0.0,
            },
        })
        .collect();
    PlantMesh {
        vertices,
        ..PlantMesh::default()
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn close(a: [f64; 3], b: [f64; 3]) {
    for axis in 0..3 {
        assert!((a[axis] - b[axis]).abs() < 1e-12, "{a:?} != {b:?}");
    }
}

#[test]
fn atlas_cells_tile_the_square_exactly() -> Result<(), Box<dyn std::error::Error>> {
    let mesh = box_mesh([2.0, -4.0, 10.0], [6.0, 14.0, 11.0]);
    for grid in [2, 3, 8, 9, 12] {
        for hemi in [false, true] {
            let plan = octahedral_impostor_plan(&mesh, grid, hemi)?;
            assert_eq!(plan.grid, grid);
            assert_eq!(plan.hemi, hemi);
            assert_eq!(u32::try_from(plan.frames.len())?, grid * grid);
            let mut area = 0.0;
            for (index, frame) in plan.frames.iter().enumerate() {
                let index = u32::try_from(index)?;
                assert_eq!((frame.col, frame.row), (index % grid, index / grid));
                assert!(
                    frame
                        .uv_min
                        .iter()
                        .chain(&frame.uv_max)
                        .all(|v| (0.0..=1.0).contains(v))
                );
                for (axis, cell) in [frame.col, frame.row].into_iter().enumerate() {
                    assert_eq!(
                        frame.uv_min[axis].to_bits(),
                        (f64::from(cell) / f64::from(grid)).to_bits()
                    );
                    assert_eq!(
                        frame.uv_max[axis].to_bits(),
                        (f64::from(cell + 1) / f64::from(grid)).to_bits()
                    );
                }
                if frame.col > 0 {
                    let previous = &plan.frames[usize::try_from(index - 1)?];
                    assert_eq!(previous.uv_max[0].to_bits(), frame.uv_min[0].to_bits());
                }
                if frame.row > 0 {
                    let above = &plan.frames[usize::try_from(index - grid)?];
                    assert_eq!(above.uv_max[1].to_bits(), frame.uv_min[1].to_bits());
                }
                area += (frame.uv_max[0] - frame.uv_min[0]) * (frame.uv_max[1] - frame.uv_min[1]);
            }
            assert!((area - 1.0).abs() < 1e-12);
        }
    }
    Ok(())
}

#[test]
fn bake_cameras_look_at_the_centre_from_the_grid_directions() -> Result<(), LodError> {
    let mesh = box_mesh([2.0, -4.0, 10.0], [6.0, 14.0, 11.0]);
    for grid in [2, 3, 8, 9, 12] {
        for hemi in [false, true] {
            let plan = octahedral_impostor_plan(&mesh, grid, hemi)?;
            close(plan.bounds_centre, [4.0, 5.0, 10.5]);
            for frame in &plan.frames {
                let direction = frame_direction(frame.col, frame.row, grid, hemi);
                close(frame.camera_direction, direction.map(|value| -value));
                let relative = core::array::from_fn(|axis| {
                    frame.camera_position[axis] - plan.bounds_centre[axis]
                });
                close(
                    relative.map(|value| value / plan.camera_distance),
                    direction,
                );
                for vector in [frame.camera_direction, frame.camera_up, frame.camera_right] {
                    assert!((dot(vector, vector) - 1.0).abs() < 1e-12);
                }
                assert!(dot(frame.camera_direction, frame.camera_up).abs() < 1e-12);
                assert!(dot(frame.camera_direction, frame.camera_right).abs() < 1e-12);
                assert!(dot(frame.camera_up, frame.camera_right).abs() < 1e-12);
                let [fx, fy, fz] = frame.camera_direction;
                let [ux, uy, uz] = frame.camera_up;
                close(
                    frame.camera_right,
                    [fy * uz - fz * uy, fz * ux - fx * uz, fx * uy - fy * ux],
                );
                assert!(dot(relative, frame.camera_direction) < 0.0);
                if hemi {
                    assert!(relative[1] >= 0.0);
                }
            }
        }
    }
    Ok(())
}

#[test]
fn every_projected_bounds_corner_fits_the_card_and_clip_range() -> Result<(), LodError> {
    for mesh in [
        box_mesh([2.0, -4.0, 10.0], [6.0, 14.0, 11.0]),
        box_mesh([-20.0, -0.1, -0.2], [20.0, 0.1, 0.2]),
    ] {
        for grid in [2, 3, 8, 9, 12] {
            for hemi in [false, true] {
                let plan = octahedral_impostor_plan(&mesh, grid, hemi)?;
                for frame in &plan.frames {
                    for vertex in &mesh.vertices {
                        let centred = core::array::from_fn(|axis| {
                            f64::from(vertex.position[axis]) - plan.bounds_centre[axis]
                        });
                        let from_camera = core::array::from_fn(|axis| {
                            f64::from(vertex.position[axis]) - frame.camera_position[axis]
                        });
                        let horizontal = dot(centred, frame.camera_right);
                        let vertical = dot(centred, frame.camera_up);
                        assert!(horizontal.abs() <= plan.card_size[0] * 0.5 + 1e-12);
                        assert!(vertical.abs() <= plan.card_size[1] * 0.5 + 1e-12);
                        let depth = dot(from_camera, frame.camera_direction);
                        assert!(depth >= plan.near && depth <= plan.far);
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn plans_are_deterministic_and_handle_degenerate_and_extreme_bounds() -> Result<(), LodError> {
    for mesh in [
        PlantMesh::default(),
        box_mesh([3.0, 2.0, 1.0], [3.0, 2.0, 1.0]),
        box_mesh([f32::MAX; 3], [f32::MAX; 3]),
        box_mesh([-f32::MAX; 3], [f32::MAX; 3]),
    ] {
        for hemi in [false, true] {
            let plan = octahedral_impostor_plan(&mesh, 9, hemi)?;
            assert_eq!(plan, octahedral_impostor_plan(&mesh, 9, hemi)?);
            assert!(plan.card_size.iter().all(|v| v.is_finite() && *v >= 0.0));
            assert!(plan.camera_distance.is_finite() && plan.far.is_finite());
            for frame in &plan.frames {
                assert!(frame.camera_position.iter().all(|v| v.is_finite()));
                let relative = core::array::from_fn(|axis| {
                    (frame.camera_position[axis] - plan.bounds_centre[axis]) / plan.camera_distance
                });
                close(relative, frame.camera_direction.map(|value| -value));
            }
        }
    }
    Ok(())
}

#[test]
fn invalid_grids_and_nonfinite_mesh_positions_are_rejected() {
    for grid in [0, 1, 257, u32::MAX] {
        assert!(matches!(
            octahedral_impostor_plan(&PlantMesh::default(), grid, true),
            Err(LodError::BadParameter { name: "grid", .. })
        ));
    }
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mesh = box_mesh([bad, 0.0, 0.0], [1.0; 3]);
        assert!(matches!(
            octahedral_impostor_plan(&mesh, 8, false),
            Err(LodError::BadParameter { name: "mesh", .. })
        ));
    }
}
