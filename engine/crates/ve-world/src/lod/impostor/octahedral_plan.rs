//! Plain orthographic bake data; constructing it does not render an atlas.
use super::super::{LodError, PlantMesh, octahedral::frame_direction};

/// One frame's atlas rectangle and orthographic bake camera, in model space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OctahedralImpostorFrame {
    /// Zero-based atlas column.
    pub col: u32,
    /// Zero-based atlas row.
    pub row: u32,
    /// Lower UV corner of the atlas cell; UVs increase with column and row.
    pub uv_min: [f64; 2],
    /// Upper UV corner of the atlas cell (no gutter inset).
    pub uv_max: [f64; 2],
    /// Bake camera position, looking at the plan's bounds centre.
    pub camera_position: [f64; 3],
    /// Unit forward direction from the camera towards the bounds centre.
    pub camera_direction: [f64; 3],
    /// Unit camera up, perpendicular to `camera_direction`.
    pub camera_up: [f64; 3],
    /// Unit camera right: `camera_direction` crossed with `camera_up`.
    pub camera_right: [f64; 3],
}

/// Description of an optional octahedral atlas, independent of renderer resources.
///
/// Every frame uses an orthographic card centred on `bounds_centre`, with the
/// same width and height. The square covers the entire bounding box from any
/// direction, including views between bake frames. Existing billboard specs
/// remain available through [`super::impostor_spec`].
#[derive(Debug, Clone, PartialEq)]
pub struct OctahedralImpostorPlan {
    /// Number of columns and rows, each sampling grid vertices, in 2..=256.
    pub grid: u32,
    /// Whether the directions cover only the upper hemisphere.
    pub hemi: bool,
    /// Frames in row-major atlas order, with exactly `grid * grid` entries.
    pub frames: Vec<OctahedralImpostorFrame>,
    /// Minimum model-space corner of the mesh bounds.
    pub bounds_min: [f64; 3],
    /// Maximum model-space corner of the mesh bounds.
    pub bounds_max: [f64; 3],
    /// Shared target and centre of every card, in model space.
    pub bounds_centre: [f64; 3],
    /// Radius enclosing all corners of the bounding box, in metres.
    pub bounds_radius: f64,
    /// Full orthographic card width and height, in metres (twice the radius).
    pub card_size: [f64; 2],
    /// Distance of each camera from the bounds centre, in metres.
    pub camera_distance: f64,
    /// Orthographic near clip distance from each camera, in metres.
    pub near: f64,
    /// Orthographic far clip distance from each camera, in metres.
    pub far: f64,
}

/// Builds an octahedral bake plan without changing the fixed billboard path.
///
/// The camera offset uses [`frame_direction`]; its forward axis points back at
/// the bounds centre. A box-enclosing sphere supplies a conservative square
/// card for every orientation. Calculations use f64 even for extreme f32 meshes.
/// Empty meshes have zero bounds and zero-sized cards, with valid cameras.
///
/// # Errors
/// Returns [`LodError::BadParameter`] for grids outside 2..=256 (at most 65,536
/// frames) or a mesh containing non-finite positions.
pub fn octahedral_impostor_plan(
    mesh: &PlantMesh,
    grid: u32,
    hemi: bool,
) -> Result<OctahedralImpostorPlan, LodError> {
    if !(2..=256).contains(&grid) {
        return Err(LodError::BadParameter {
            name: "grid",
            why: "use 2..=256 octahedral frames per axis".into(),
        });
    }
    let (minimum, maximum) = bounds(mesh)?;
    let centre = core::array::from_fn(|axis| (minimum[axis] + maximum[axis]) * 0.5);
    let half_extent: [f64; 3] = core::array::from_fn(|axis| (maximum[axis] - minimum[axis]) * 0.5);
    let radius = half_extent[0].hypot(half_extent[1]).hypot(half_extent[2]);
    // A point at a very large coordinate still needs a representable camera
    // offset; adding one metre to such a centre can round back to the centre.
    let minimum_distance = if radius > 0.0 {
        1.0
    } else {
        centre
            .iter()
            .map(|value| value.abs())
            .fold(1.0_f64, f64::max)
    };
    let camera_distance = (radius * 4.0).max(minimum_distance);
    let mut frames = Vec::new();
    for row in 0..grid {
        for col in 0..grid {
            let offset = frame_direction(col, row, grid, hemi);
            let forward = offset.map(|value| -value);
            // A Z reference keeps the vertical pole views well-defined.
            let reference = if offset[1].abs() > 0.999 {
                [0.0, 0.0, -1.0]
            } else {
                [0.0, 1.0, 0.0]
            };
            let right = unit(cross(forward, reference));
            let up = unit(cross(right, forward));
            frames.push(OctahedralImpostorFrame {
                col,
                row,
                uv_min: [
                    f64::from(col) / f64::from(grid),
                    f64::from(row) / f64::from(grid),
                ],
                uv_max: [
                    f64::from(col + 1) / f64::from(grid),
                    f64::from(row + 1) / f64::from(grid),
                ],
                camera_position: core::array::from_fn(|axis| {
                    centre[axis] + offset[axis] * camera_distance
                }),
                camera_direction: forward,
                camera_up: up,
                camera_right: right,
            });
        }
    }
    Ok(OctahedralImpostorPlan {
        grid,
        hemi,
        frames,
        bounds_min: minimum,
        bounds_max: maximum,
        bounds_centre: centre,
        bounds_radius: radius,
        card_size: [2.0 * radius; 2],
        camera_distance,
        near: 0.0,
        far: 2.0 * camera_distance,
    })
}

fn bounds(mesh: &PlantMesh) -> Result<([f64; 3], [f64; 3]), LodError> {
    let mut minimum = [f64::INFINITY; 3];
    let mut maximum = [f64::NEG_INFINITY; 3];
    for vertex in &mesh.vertices {
        for axis in 0..3 {
            let position = f64::from(vertex.position[axis]);
            if !position.is_finite() {
                return Err(LodError::BadParameter {
                    name: "mesh",
                    why: "octahedral bake positions must be finite".into(),
                });
            }
            minimum[axis] = minimum[axis].min(position);
            maximum[axis] = maximum[axis].max(position);
        }
    }
    if mesh.vertices.is_empty() {
        Ok(([0.0; 3], [0.0; 3]))
    } else {
        Ok((minimum, maximum))
    }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(vector: [f64; 3]) -> [f64; 3] {
    let length = vector[0].hypot(vector[1]).hypot(vector[2]);
    vector.map(|value| value / length)
}
