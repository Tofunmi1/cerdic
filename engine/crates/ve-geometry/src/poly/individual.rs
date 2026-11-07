//! Quad-grid source mesh and "individual faces" modes of inset and extrude.
//!
//! A facade is a grid of cells that are each inset or extruded on their own. `grid` makes the cells;
//! `inset_each` and `extrude_each` apply the single-face contract to every selected face in turn.
//! If any face violates its contract the whole command refuses, naming that face, and the input is
//! untouched.

use std::collections::BTreeSet;

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{
    AttrData, Attribute, Domain, Element, GeometryError, PolyMesh, extrude_faces, inset_face,
};

/// The most faces `grid` will make and the most faces an individual-mode command will edit.
pub const MAX_GRID_FACES: usize = 262_144;
/// The most faces one individual-mode command edits (each edit re-validates the mesh).
pub const MAX_INDIVIDUAL_FACES: usize = 4096;

fn refusal(code: &'static str, face: Option<usize>) -> GeometryError {
    GeometryError::PolyRefusal {
        code,
        element: face.map(Element::Face),
    }
}

/// A flat grid of `columns` x `rows` quads in the z = 0 plane, `width` wide centred on x = 0 and
/// `height` tall from y = 0, facing +z. Faces are numbered row by row from the bottom left. Each
/// corner also has normalized `u`/`v` coordinates for texturing and face-varying subdivision.
///
/// # Errors
/// `poly.grid_invalid` for a size or count that is not positive and finite or exceeds
/// [`MAX_GRID_FACES`].
pub fn grid(width: f32, height: f32, columns: u32, rows: u32) -> Result<PolyMesh, GeometryError> {
    let faces = (columns as usize).checked_mul(rows as usize);
    if !width.is_finite()
        || !height.is_finite()
        || width <= 0.0
        || height <= 0.0
        || columns == 0
        || rows == 0
        || faces.is_none_or(|count| count > MAX_GRID_FACES)
    {
        return Err(refusal("poly.grid_invalid", None));
    }
    let across = columns + 1;
    let mut positions = Vec::with_capacity((across * (rows + 1)) as usize);
    for row in 0..=rows {
        for column in 0..=columns {
            #[expect(
                clippy::cast_precision_loss,
                reason = "grid counts are bounded far below 2^24"
            )]
            positions.push([
                width * (column as f32 / columns as f32 - 0.5),
                height * (row as f32 / rows as f32 - 0.5),
                0.0,
            ]);
        }
    }
    let mut corners = Vec::with_capacity(faces.unwrap_or(0) * 4);
    for row in 0..rows {
        for column in 0..columns {
            let a = row * across + column;
            corners.extend([a, a + 1, a + across + 1, a + across]);
        }
    }
    let (u, v) = grid_uvs(columns, rows, faces.unwrap_or(0));
    let mut mesh = PolyMesh::new(positions, vec![4; faces.unwrap_or(0)], corners)?;
    mesh.set_attribute(
        "u",
        Attribute {
            domain: Domain::Corner,
            data: AttrData::F32(u),
        },
    )?;
    mesh.set_attribute(
        "v",
        Attribute {
            domain: Domain::Corner,
            data: AttrData::F32(v),
        },
    )?;
    Ok(mesh)
}

fn grid_uvs(columns: u32, rows: u32, faces: usize) -> (Vec<f32>, Vec<f32>) {
    let mut u = Vec::with_capacity(faces * 4);
    let mut v = Vec::with_capacity(faces * 4);
    for row in 0..rows {
        for column in 0..columns {
            #[expect(
                clippy::cast_precision_loss,
                reason = "grid counts stay below f32 exact integer range"
            )]
            let (left, right, bottom, top) = (
                column as f32 / columns as f32,
                (column + 1) as f32 / columns as f32,
                row as f32 / rows as f32,
                (row + 1) as f32 / rows as f32,
            );
            u.extend([left, right, right, left]);
            v.extend([bottom, bottom, top, top]);
        }
    }
    (u, v)
}

fn checked_selection(mesh: &PolyMesh, selection: &[usize]) -> Result<(), GeometryError> {
    let unique: BTreeSet<usize> = selection.iter().copied().collect();
    if unique.is_empty() {
        return Err(refusal("poly.selection_empty", None));
    }
    if unique.len() != selection.len() || unique.iter().any(|&f| f >= mesh.face_count()) {
        return Err(refusal("poly.selection_invalid", unique.first().copied()));
    }
    if selection.len() > MAX_INDIVIDUAL_FACES {
        return Err(refusal("poly.budget_exceeded", selection.first().copied()));
    }
    Ok(())
}

/// The result of insetting several faces: the mesh, and where each inner cap ended up.
#[derive(Clone, Debug, PartialEq)]
pub struct InsetEach {
    /// The inset mesh.
    pub mesh: PolyMesh,
    /// The face ordinal of each selected face's inner cap in `mesh`, in ascending order of the
    /// original face ordinals.
    pub caps: Vec<usize>,
    /// The ordinals of the ring quads around the caps, ascending.
    pub rings: Vec<usize>,
}

/// Insets every selected face by `distance`, each on its own.
///
/// An inset puts the ring and the inner cap in the place of the face, so later faces move up; faces
/// are inset from the highest ordinal down so the given ordinals stay valid, and the final ordinal of
/// each cap is returned.
///
/// # Errors
/// The refusal of the first face that breaks the [`inset_face`] contract, a selection error, or
/// cancellation.
pub fn inset_each(
    mesh: &PolyMesh,
    selection: &[usize],
    distance: f32,
    context: &ExecutionContext,
) -> Result<InsetEach, GeometryError> {
    checked_selection(mesh, selection)?;
    let mut ascending = selection.to_vec();
    ascending.sort_unstable();
    let mut current = mesh.clone();
    for &face in ascending.iter().rev() {
        check_cancelled(context)?;
        current = inset_face(&current, face, distance, context)?;
    }
    let mut shift = 0_usize;
    let mut caps = Vec::with_capacity(ascending.len());
    let mut rings = Vec::new();
    for &face in &ascending {
        let corners = mesh.face_sizes()[face] as usize;
        rings.extend(face + shift..face + shift + corners);
        caps.push(face + shift + corners);
        shift += corners;
    }
    Ok(InsetEach {
        mesh: current,
        caps,
        rings,
    })
}

/// Extrudes every selected face by `offset`, each on its own.
///
/// # Errors
/// The refusal of the first face that breaks the [`extrude_faces`] contract, a selection error, or
/// cancellation.
pub fn extrude_each(
    mesh: &PolyMesh,
    selection: &[usize],
    offset: [f32; 3],
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    checked_selection(mesh, selection)?;
    let mut current = mesh.clone();
    for &face in selection {
        check_cancelled(context)?;
        current = extrude_faces(&current, &[face], offset, context)?;
    }
    Ok(current)
}
