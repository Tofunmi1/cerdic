//! Bevel any set of manifold edges, connected networks and vertex fans included, with Blender's bevel.

use ve_kernel::{BevelOffset, BevelOptions, ExecutionContext, PolygonMesh};

use crate::topology::check_cancelled;
use crate::{Element, GeometryError, PolyMesh, Topology};

/// The most segments one network bevel may use.
pub const MAX_NETWORK_BEVEL_SEGMENTS: usize = 16;

/// How the width of a [`bevel_network`] is measured.
pub type NetworkBevelOffset = BevelOffset;

/// Parameters of [`bevel_network`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NetworkBevel {
    /// Bevel size in the unit chosen by `offset`; positive and finite.
    pub width: f64,
    /// Segments across the bevel, 1 to [`MAX_NETWORK_BEVEL_SEGMENTS`].
    pub segments: usize,
    /// Profile shape, 0.1 to 1; 0.5 is a circular arc.
    pub profile: f64,
    /// How `width` is measured.
    pub offset: NetworkBevelOffset,
}

fn refusal(code: &'static str, element: Option<Element>) -> GeometryError {
    GeometryError::PolyRefusal { code, element }
}

/// Bevels the selected edges, whatever the valence of their vertices: a cube corner, a vertex where
/// several beveled edges meet, a closed loop around a box, a fan of edges. Faces keep their attributes
/// (a new bevel face takes its neighbour's); vertex and corner attributes follow the input vertex a
/// vertex descends from. Edge attributes have no source on the new edges and refuse, as in the other
/// polygon edits.
///
/// Every selected edge must be a manifold interior edge (exactly two faces). The width is clamped so
/// the bevel never overlaps itself, so a very large width gives the largest bevel that fits.
///
/// # Errors
/// A typed refusal naming the edge (`poly.selection_empty`, `poly.selection_invalid`,
/// `poly.boundary_edge`, `poly.nonmanifold`, `poly.bevel_width_invalid`, `poly.bevel_segments_invalid`,
/// `poly.bevel_profile_invalid`), a kernel error, or cancellation. The input is never changed.
pub fn bevel_network(
    mesh: &PolyMesh,
    edges: &[(u32, u32)],
    options: &NetworkBevel,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    check_options(edges, options)?;
    let topology = Topology::build(mesh, context)?;
    check_edges(&topology, edges)?;
    check_cancelled(context)?;
    let pairs: Vec<[u32; 2]> = edges.iter().map(|&(a, b)| [a, b]).collect();
    let input = PolygonMesh {
        positions: mesh.positions(),
        face_sizes: mesh.face_sizes(),
        corners: mesh.corners(),
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "width and profile are finite and in range; the kernel works in f32"
    )]
    let kernel_options = BevelOptions {
        width: options.width as f32,
        segments: u32::try_from(options.segments).unwrap_or(u32::MAX),
        offset_type: options.offset,
        profile: options.profile as f32,
    };
    let beveled = ve_kernel::bevel_edges(&input, &pairs, &kernel_options)?;
    super::blender_result::into_poly(mesh, &beveled, context)
}

fn check_options(edges: &[(u32, u32)], options: &NetworkBevel) -> Result<(), GeometryError> {
    if edges.is_empty() {
        return Err(refusal("poly.selection_empty", None));
    }
    if !options.width.is_finite() || options.width <= 0.0 {
        return Err(refusal("poly.bevel_width_invalid", None));
    }
    if options.segments == 0 || options.segments > MAX_NETWORK_BEVEL_SEGMENTS {
        return Err(refusal("poly.bevel_segments_invalid", None));
    }
    if !options.profile.is_finite() || !(0.1..=1.0).contains(&options.profile) {
        return Err(refusal("poly.bevel_profile_invalid", None));
    }
    Ok(())
}

/// Every edge exists and joins exactly two faces.
fn check_edges(topology: &Topology, edges: &[(u32, u32)]) -> Result<(), GeometryError> {
    for &(a, b) in edges {
        let key = (a.min(b), a.max(b));
        let element = Some(Element::Edge(key.0, key.1));
        let Some(info) = topology.edges().get(&key) else {
            return Err(refusal("poly.selection_invalid", element));
        };
        match info.face_count() {
            2 => {}
            1 => return Err(refusal("poly.boundary_edge", element)),
            _ => return Err(refusal("poly.nonmanifold", element)),
        }
    }
    Ok(())
}
