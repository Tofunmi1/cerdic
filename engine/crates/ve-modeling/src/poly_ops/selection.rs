//! What a `poly.select` node chose, and the identity of the mesh it chose on.

use ve_geometry::PolyMesh;

/// The chosen elements, in ascending order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Elements {
    Faces(Vec<usize>),
    Edges(Vec<(u32, u32)>),
    Vertices(Vec<u32>),
    /// Ordinals into the mesh's boundary loops, each rotated to start at its smallest vertex.
    Loops(Vec<usize>),
}

impl Elements {
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::Faces(items) | Self::Loops(items) => items.len(),
            Self::Edges(items) => items.len(),
            Self::Vertices(items) => items.len(),
        }
    }

    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Faces(_) => "face",
            Self::Edges(_) => "edge",
            Self::Vertices(_) => "vertex",
            Self::Loops(_) => "loop",
        }
    }
}

/// A rule's result on one mesh.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Selection {
    pub(crate) elements: Elements,
    /// Identity of the mesh the elements index into.
    pub(crate) fingerprint: u64,
}

impl Selection {
    pub(crate) fn bytes(&self) -> usize {
        self.elements.len() * 8 + size_of::<Self>()
    }
}

/// A deterministic hash of a mesh's topology and positions (FNV-1a).
pub(crate) fn fingerprint(mesh: &PolyMesh) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |value: u32| {
        for byte in value.to_le_bytes() {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    };
    mix(u32::try_from(mesh.face_count()).unwrap_or(u32::MAX));
    for &size in mesh.face_sizes() {
        mix(size);
    }
    for &corner in mesh.corners() {
        mix(corner);
    }
    for position in mesh.positions() {
        for coordinate in position {
            mix(coordinate.to_bits());
        }
    }
    hash
}
