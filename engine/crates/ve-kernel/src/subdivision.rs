//! Uniform Catmull-Clark subdivision of indexed polygon cages.
use crate::{ExecutionContext, KernelError, ffi};

/// Optional continuous channels refined with the cage. Point values are interleaved per vertex;
/// face-varying UV values are indexed separately for every cage corner.
#[derive(Clone, Copy, Debug, Default)]
pub struct SubdivisionChannels<'a> {
    /// Interleaved point scalar channels, in vertex-major order; at most eight channels.
    pub point_channels: usize,
    /// Point data, with `positions.len() * point_channels` values.
    pub point_values: &'a [f32],
    /// Unique `(u, v)` values used by the face-varying channel.
    pub fvar_values: &'a [[f32; 2]],
    /// Unique value index for each cage corner.
    pub fvar_corner_values: &'a [u32],
}

/// Sharpness applied to an undirected cage edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crease {
    /// Edge endpoints; order is ignored.
    pub vertices: [u32; 2],
    /// Sharpness in `OpenSubdiv`'s units. Zero is smooth; values of ten are effectively sharp.
    pub sharpness: f32,
}

/// Sharpness applied to one cage vertex.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerSharpness {
    /// Cage vertex ordinal.
    pub vertex: u32,
    /// Sharpness in `OpenSubdiv`'s units.
    pub sharpness: f32,
}

/// Refined polygon mesh and element provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct SubdivisionMesh {
    /// Refined vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Number of corners in each output face.
    pub face_sizes: Vec<u32>,
    /// Refined vertex index for each output corner.
    pub corners: Vec<u32>,
    /// Immediate parent element kind per output vertex: 0 vertex, 1 edge, 2 face.
    pub vertex_origin_kind: Vec<u32>,
    /// Immediate parent element index per output vertex. Edge indices use sorted undirected endpoints.
    pub vertex_origin_index: Vec<u32>,
    /// Level-zero cage face for each output face.
    pub face_parent: Vec<u32>,
    /// Interleaved point channel values on refined vertices.
    pub point_values: Vec<f32>,
    /// Number of interleaved point channels.
    pub point_channels: usize,
    /// Unique face-varying UV values on the refined mesh.
    pub fvar_values: Vec<[f32; 2]>,
    /// Unique UV value index for each refined corner.
    pub fvar_corner_values: Vec<u32>,
}

/// Refines a polygon cage using uniform Catmull-Clark subdivision.
///
/// Boundary rule: 0 none, 1 edge-only, or 2 edge-and-corner. Crease method: 0 uniform or
/// 1 Chaikin. Use [`subdivide_catmark_with_features`] to set crease and corner sharpness.
///
/// # Errors
/// Returns [`KernelError::InvalidArgument`] for malformed topology or options,
/// [`KernelError::NotManifold`] for non-manifold/inconsistently wound cages,
/// [`KernelError::TooLarge`] when the checked refinement budget is exceeded, or
/// [`KernelError::Cancelled`] when the context is cancelled.
pub fn subdivide_catmark(
    positions: &[[f32; 3]],
    face_sizes: &[u32],
    corners: &[u32],
    levels: u32,
    boundary_rule: i32,
    crease_method: i32,
    context: &ExecutionContext,
) -> Result<SubdivisionMesh, KernelError> {
    subdivide_catmark_with_features(
        positions,
        face_sizes,
        corners,
        &[],
        &[],
        levels,
        boundary_rule,
        crease_method,
        context,
    )
}

/// Catmull-Clark with edge creases and corner sharpness.
///
/// # Errors
/// Returns the same errors as [`subdivide_catmark`], plus `InvalidArgument` for a sharpness list
/// whose values are non-finite, negative, or refer to an invalid element.
pub fn subdivide_catmark_with_features(
    positions: &[[f32; 3]],
    face_sizes: &[u32],
    corners: &[u32],
    creases: &[Crease],
    sharp_corners: &[CornerSharpness],
    levels: u32,
    boundary_rule: i32,
    crease_method: i32,
    context: &ExecutionContext,
) -> Result<SubdivisionMesh, KernelError> {
    subdivide_catmark_with_channels(
        positions,
        face_sizes,
        corners,
        creases,
        sharp_corners,
        SubdivisionChannels::default(),
        levels,
        boundary_rule,
        crease_method,
        context,
    )
}

/// Catmull-Clark with continuous point channels and an optional face-varying UV channel.
///
/// # Errors
/// Returns the same errors as [`subdivide_catmark_with_features`], plus `AttributeLimit` when
/// channel count or refined storage exceeds its fixed bound.
pub fn subdivide_catmark_with_channels(
    positions: &[[f32; 3]],
    face_sizes: &[u32],
    corners: &[u32],
    creases: &[Crease],
    sharp_corners: &[CornerSharpness],
    channels: SubdivisionChannels<'_>,
    levels: u32,
    boundary_rule: i32,
    crease_method: i32,
    context: &ExecutionContext,
) -> Result<SubdivisionMesh, KernelError> {
    validate_cage(positions, face_sizes, corners, levels)?;
    validate_channels(positions.len(), corners.len(), channels)?;
    validate_sharpness(positions.len(), creases, sharp_corners)?;
    let crease_edges: Vec<_> = creases.iter().map(|crease| crease.vertices).collect();
    let crease_weights: Vec<_> = creases.iter().map(|crease| crease.sharpness).collect();
    let corner_vertices: Vec<_> = sharp_corners.iter().map(|corner| corner.vertex).collect();
    let corner_weights: Vec<_> = sharp_corners
        .iter()
        .map(|corner| corner.sharpness)
        .collect();
    let raw = ffi::subdivision::catmark(
        positions,
        face_sizes,
        corners,
        &crease_edges,
        &crease_weights,
        &corner_vertices,
        &corner_weights,
        channels,
        levels,
        boundary_rule,
        crease_method,
        context,
    )?;
    Ok(SubdivisionMesh {
        positions: raw.positions,
        face_sizes: raw.face_sizes,
        corners: raw.corners,
        vertex_origin_kind: raw.vertex_origin_kind,
        vertex_origin_index: raw.vertex_origin_index,
        face_parent: raw.face_parent,
        point_values: raw.point_values,
        point_channels: raw.point_channels,
        fvar_values: raw.fvar_values,
        fvar_corner_values: raw.fvar_corner_values,
    })
}

fn validate_cage(
    positions: &[[f32; 3]],
    face_sizes: &[u32],
    corners: &[u32],
    levels: u32,
) -> Result<(), KernelError> {
    if positions.is_empty()
        || face_sizes.is_empty()
        || corners.is_empty()
        || levels == 0
        || levels > 4
    {
        return Err(KernelError::InvalidArgument(
            "subdivision cage or level count",
        ));
    }
    if positions.iter().flatten().any(|value| !value.is_finite()) {
        return Err(KernelError::NonFinite);
    }
    let corner_total = face_sizes
        .iter()
        .try_fold(0_usize, |sum, size| {
            if *size < 3 {
                None
            } else {
                sum.checked_add(*size as usize)
            }
        })
        .ok_or(KernelError::InvalidArgument("subdivision face sizes"))?;
    if corner_total != corners.len() {
        return Err(KernelError::InvalidArgument("subdivision corner count"));
    }
    if corners
        .iter()
        .any(|&index| index as usize >= positions.len())
    {
        return Err(KernelError::OutOfBounds);
    }
    Ok(())
}

fn validate_channels(
    vertex_count: usize,
    corner_count: usize,
    channels: SubdivisionChannels<'_>,
) -> Result<(), KernelError> {
    if channels.point_channels > 8 {
        return Err(KernelError::AttributeLimit);
    }
    let expected_point_values = vertex_count
        .checked_mul(channels.point_channels)
        .ok_or(KernelError::AttributeLimit)?;
    let valid_fvar = (channels.fvar_values.is_empty() && channels.fvar_corner_values.is_empty())
        || channels.fvar_corner_values.len() == corner_count;
    if channels.point_values.len() != expected_point_values
        || channels.point_values.iter().any(|value| !value.is_finite())
        || !valid_fvar
        || channels
            .fvar_values
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        || channels
            .fvar_corner_values
            .iter()
            .any(|index| *index as usize >= channels.fvar_values.len())
    {
        return Err(KernelError::InvalidArgument("subdivision channel layout"));
    }
    if !channels.fvar_corner_values.is_empty() && channels.fvar_corner_values.len() != corner_count
    {
        return Err(KernelError::InvalidArgument(
            "subdivision face-varying corner count",
        ));
    }
    Ok(())
}

fn validate_sharpness(
    vertex_count: usize,
    creases: &[Crease],
    sharp_corners: &[CornerSharpness],
) -> Result<(), KernelError> {
    if creases.iter().any(|crease| {
        !crease.sharpness.is_finite()
            || crease.sharpness < 0.0
            || crease.vertices[0] as usize >= vertex_count
            || crease.vertices[1] as usize >= vertex_count
            || crease.vertices[0] == crease.vertices[1]
    }) || sharp_corners.iter().any(|corner| {
        !corner.sharpness.is_finite()
            || corner.sharpness < 0.0
            || corner.vertex as usize >= vertex_count
    }) {
        return Err(KernelError::InvalidArgument("subdivision sharpness"));
    }
    Ok(())
}
