//! UV unwrapping with Blender's parametrizer and island packer, stored as the corner attributes `u`
//! and `v`.

use ve_kernel::{ExecutionContext, PolygonMesh, UnwrapOptions};

use crate::topology::check_cancelled;
use crate::{AttrData, Attribute, Domain, Element, GeometryError, PolyMesh, Topology};

pub use ve_kernel::UvMethod;

/// Parameters of [`unwrap_uv`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvUnwrap {
    /// How islands are flattened.
    pub method: UvMethod,
    /// Pack the islands into the unit square.
    pub pack: bool,
    /// Let packing rotate islands.
    pub rotate: bool,
    /// Space between packed islands as a fraction of the unit square (0 to 0.5).
    pub margin: f64,
}

fn refusal(code: &'static str, element: Option<Element>) -> GeometryError {
    GeometryError::PolyRefusal { code, element }
}

/// Cuts the mesh along `seams` (edges of the mesh) and flattens every island, writing the result to the
/// corner attributes `u` and `v` (replacing any that exist; other attributes are kept). A mesh that is a
/// closed surface needs seams to open it; a mesh that is already open can be unwrapped with none.
///
/// # Errors
/// `poly.selection_invalid` for a seam that is not an edge of the mesh, `poly.uv_unwrap_failed` when an
/// island could not be flattened (usually a closed surface without seams), a kernel error for a bad
/// option or a mesh over 1,000,000 faces, or cancellation. The input is never changed.
pub fn unwrap_uv(
    mesh: &PolyMesh,
    seams: &[(u32, u32)],
    options: &UvUnwrap,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    if !seams.is_empty() {
        let topology = Topology::build(mesh, context)?;
        for &(a, b) in seams {
            let key = (a.min(b), a.max(b));
            if !topology.edges().contains_key(&key) {
                return Err(refusal(
                    "poly.selection_invalid",
                    Some(Element::Edge(key.0, key.1)),
                ));
            }
        }
    }
    check_cancelled(context)?;
    let pairs: Vec<[u32; 2]> = seams.iter().map(|&(a, b)| [a, b]).collect();
    let input = PolygonMesh {
        positions: mesh.positions(),
        face_sizes: mesh.face_sizes(),
        corners: mesh.corners(),
    };
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the margin is a small fraction the kernel validates; it works in f32"
    )]
    let kernel_options = UnwrapOptions {
        method: options.method,
        pack: options.pack,
        rotate: options.rotate,
        margin: options.margin as f32,
    };
    let layout = ve_kernel::unwrap_uvs(&input, &pairs, &kernel_options)?;
    check_cancelled(context)?;
    if layout.failed_islands > 0 {
        return Err(refusal("poly.uv_unwrap_failed", None));
    }
    let mut result = mesh.clone();
    for (name, axis) in [("u", 0), ("v", 1)] {
        result.set_attribute(
            name,
            Attribute {
                domain: Domain::Corner,
                data: AttrData::F32(layout.uvs.iter().map(|uv| uv[axis]).collect()),
            },
        )?;
    }
    Ok(result)
}
