//! Attribute transfer for operators that rebuild a mesh from provenance lists.

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{AttrData, Attribute, Domain, GeometryError, PolyMesh};

/// For each element of the candidate, the index of the source element its attribute value comes from.
pub(crate) struct Sources {
    pub(crate) points: Vec<usize>,
    pub(crate) faces: Vec<usize>,
    pub(crate) corners: Vec<usize>,
}

/// Copies every attribute of `source` onto `target` following `sources`. Edge attributes have no
/// unambiguous source for new edges and refuse with `poly.attribute_conflict`.
pub(crate) fn transfer(
    source: &PolyMesh,
    target: &mut PolyMesh,
    sources: &Sources,
    context: &ExecutionContext,
) -> Result<(), GeometryError> {
    for (name, attribute) in source.attributes() {
        check_cancelled(context)?;
        let data = match attribute.domain {
            Domain::Point => gather(&attribute.data, &sources.points)?,
            Domain::Face => gather(&attribute.data, &sources.faces)?,
            Domain::Corner => gather(&attribute.data, &sources.corners)?,
            Domain::Edge => {
                return Err(GeometryError::PolyRefusal {
                    code: "poly.attribute_conflict",
                    element: None,
                });
            }
            Domain::CurvePoint | Domain::Instance => {
                return Err(GeometryError::DomainNotOnMesh(attribute.domain));
            }
        };
        target.set_attribute(
            name,
            Attribute {
                domain: attribute.domain,
                data,
            },
        )?;
    }
    Ok(())
}

pub(crate) fn gather(data: &AttrData, indices: &[usize]) -> Result<AttrData, GeometryError> {
    macro_rules! select {
        ($variant:ident, $values:expr) => {{
            let values = indices
                .iter()
                .map(|&i| {
                    $values
                        .get(i)
                        .copied()
                        .ok_or(GeometryError::InvalidMesh("attribute source missing"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(AttrData::$variant(values))
        }};
    }
    match data {
        AttrData::F32(v) => select!(F32, v),
        AttrData::F64(v) => select!(F64, v),
        AttrData::I32(v) => select!(I32, v),
        AttrData::U32(v) => select!(U32, v),
    }
}
