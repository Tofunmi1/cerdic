//! Collision shapes.
use rapier3d::na::{DMatrix, Vector3};
use rapier3d::prelude::{ColliderBuilder, Point};

use crate::{Error, MAX_SHAPE_POINTS};

/// What a body collides as. All shapes are centred on the body's position (a convex shape or a mesh is moved so its
/// bounding-box centre is the origin; the caller carries the offset).
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A box with these half extents along X, Y, Z.
    Box {
        /// Half the size along each axis.
        half: [f32; 3],
    },
    /// A ball.
    Sphere {
        /// Radius in metres.
        radius: f32,
    },
    /// A capsule standing along Y.
    Capsule {
        /// Half the length of the straight part.
        half_height: f32,
        /// Radius of the caps and the sides.
        radius: f32,
    },
    /// A cylinder standing along Y.
    Cylinder {
        /// Half the height.
        half_height: f32,
        /// Radius.
        radius: f32,
    },
    /// The convex hull of some points (any number of moving or static bodies).
    Convex {
        /// Points around the body's centre.
        points: Vec<[f32; 3]>,
    },
    /// An arbitrary triangle surface (static bodies only: terrain, buildings).
    Mesh {
        /// Vertices around the body's centre.
        points: Vec<[f32; 3]>,
        /// Triangles as vertex index triples.
        triangles: Vec<[u32; 3]>,
    },
    /// A sampled terrain surface with rows along Z and columns along X.
    Heightfield {
        /// Number of samples along Z.
        rows: usize,
        /// Number of samples along X.
        cols: usize,
        /// Heights in row-major order.
        heights: Vec<f32>,
        /// Width along X and depth along Z in metres.
        size: [f32; 2],
    },
}

fn finite(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite())
}

fn convex_collider(points: &[[f32; 3]]) -> Result<ColliderBuilder, Error> {
    if points.len() < 4
        || points.len() > MAX_SHAPE_POINTS
        || !points.iter().all(|point| finite(point))
    {
        return Err(Error::BadShape(
            "a convex shape needs 4 to 65536 finite points",
        ));
    }
    let cloud = points
        .iter()
        .map(|point| Point::new(point[0], point[1], point[2]))
        .collect::<Vec<_>>();
    ColliderBuilder::convex_hull(&cloud).ok_or(Error::BadShape("the points have no volume"))
}

fn mesh_collider(points: &[[f32; 3]], triangles: &[[u32; 3]]) -> Result<ColliderBuilder, Error> {
    let valid = points.len() >= 3
        && points.len() <= MAX_SHAPE_POINTS
        && !triangles.is_empty()
        && points.iter().all(|point| finite(point))
        && triangles
            .iter()
            .flatten()
            .all(|index| (*index as usize) < points.len());
    if !valid {
        return Err(Error::BadShape(
            "a mesh needs 3 to 65536 finite points and triangles that name them",
        ));
    }
    let cloud = points
        .iter()
        .map(|point| Point::new(point[0], point[1], point[2]))
        .collect::<Vec<_>>();
    Ok(ColliderBuilder::trimesh(cloud, triangles.to_vec()))
}

fn heightfield_collider(
    rows: usize,
    cols: usize,
    heights: &[f32],
    size: [f32; 2],
) -> Result<ColliderBuilder, Error> {
    let count = rows.checked_mul(cols);
    if rows < 2
        || cols < 2
        || count != Some(heights.len())
        || count.is_none_or(|value| value > MAX_SHAPE_POINTS)
        || !finite(heights)
        || !finite(&size)
        || size.iter().any(|value| *value <= 0.0)
    {
        return Err(Error::BadShape("invalid heightfield dimensions or samples"));
    }
    let matrix = DMatrix::from_row_slice(rows, cols, heights);
    Ok(ColliderBuilder::heightfield(
        matrix,
        Vector3::new(size[0], 1.0, size[1]),
    ))
}

impl Shape {
    /// Whether a moving body may use this shape.
    pub(crate) fn can_move(&self) -> bool {
        !matches!(self, Self::Mesh { .. } | Self::Heightfield { .. })
    }

    pub(crate) fn collider(&self) -> Result<ColliderBuilder, Error> {
        match self {
            Self::Box { half } => {
                if !finite(half) || half.iter().any(|h| *h <= 0.0) {
                    return Err(Error::BadShape("a box needs positive half extents"));
                }
                Ok(ColliderBuilder::cuboid(half[0], half[1], half[2]))
            }
            Self::Sphere { radius } => {
                if !radius.is_finite() || *radius <= 0.0 {
                    return Err(Error::BadShape("a sphere needs a positive radius"));
                }
                Ok(ColliderBuilder::ball(*radius))
            }
            Self::Capsule {
                half_height,
                radius,
            } => {
                if !finite(&[*half_height, *radius]) || *half_height < 0.0 || *radius <= 0.0 {
                    return Err(Error::BadShape(
                        "a capsule needs a positive radius and a length",
                    ));
                }
                Ok(ColliderBuilder::capsule_y(*half_height, *radius))
            }
            Self::Cylinder {
                half_height,
                radius,
            } => {
                if !finite(&[*half_height, *radius]) || *half_height <= 0.0 || *radius <= 0.0 {
                    return Err(Error::BadShape(
                        "a cylinder needs a positive radius and height",
                    ));
                }
                Ok(ColliderBuilder::cylinder(*half_height, *radius))
            }
            Self::Convex { points } => convex_collider(points),
            Self::Mesh { points, triangles } => mesh_collider(points, triangles),
            Self::Heightfield {
                rows,
                cols,
                heights,
                size,
            } => heightfield_collider(*rows, *cols, heights, *size),
        }
    }
}
