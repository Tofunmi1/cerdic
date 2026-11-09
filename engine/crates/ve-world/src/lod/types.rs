//! Plain data types of plant meshing.

/// Which surface a run of triangles belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// Trunk and branches.
    Bark,
    /// Leaf cards.
    Leaf,
}

/// Baked per-vertex wind data for the three-scale vertex function. Weights are 0..=1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindData {
    /// The world-space point the branch bends around.
    pub pivot: [f32; 3],
    /// The branch's gust phase, 0..1.
    pub phase: f32,
    /// Amplitude of whole-tree sway: small near the base, growing with height.
    pub trunk: f32,
    /// Amplitude of branch sway: 0 on the trunk, growing with branch depth and distance from the fork.
    pub branch: f32,
    /// Amplitude of leaf motion: 0 on bark, 1 on leaf cards.
    pub leaf: f32,
    /// Flutter weight across a leaf: 0 at the stem, 1 at its edge; 0 on bark.
    pub edge: f32,
}

/// One vertex of a plant mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// Position in the plant's own space (base at the origin, +y up).
    pub position: [f32; 3],
    /// Unit normal.
    pub normal: [f32; 3],
    /// Texture coordinates: bark wraps u around the branch and v along it (in metres over the bark scale); leaves cover [0,1] squared.
    pub uv: [f32; 2],
    /// Unit tangent (xyz) and handedness (w, +1 or -1).
    pub tangent: [f32; 4],
    /// Baked wind data.
    pub wind: WindData,
}

/// A run of indices that draws one surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterialRange {
    /// Which surface.
    pub kind: Surface,
    /// First index of the run.
    pub first_index: u32,
    /// Number of indices (a multiple of 3).
    pub index_count: u32,
}

/// A plant as vertex and index arrays.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlantMesh {
    /// Vertices.
    pub vertices: Vec<Vertex>,
    /// Triangle indices, counter-clockwise seen from outside.
    pub indices: Vec<u32>,
    /// Runs of indices per surface, in index order, covering `indices` exactly.
    pub material_ranges: Vec<MaterialRange>,
}

/// How leaves are drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LeafCards {
    /// One quad per leaf, bent along its length.
    Quads {
        /// Width over length.
        aspect: f32,
        /// How far the quad bends along its length, in degrees.
        bend_deg: f32,
    },
    /// Two perpendicular quads per leaf (reads from any side).
    Crossed {
        /// Width over length.
        aspect: f32,
    },
    /// Several quads fanned around each leaf point (a leaf cluster).
    Clumps {
        /// Quads per leaf point (2 to 8).
        leaves: u32,
        /// How far the quads fan out, in degrees.
        spread: f32,
        /// Width over length.
        aspect: f32,
    },
}

/// Meshing settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Meshing {
    /// Sides of a branch tube at its thickest, 3..=16 (thin twigs get fewer, down to 3).
    pub radial_segments: u32,
    /// Longest a tube segment may be, in metres; longer ones are subdivided.
    pub max_segment_length: f32,
    /// Metres of bark one texture repeat covers across (u) and along (v).
    pub bark_uv_scale: [f32; 2],
    /// How leaves are drawn.
    pub leaf_cards: LeafCards,
    /// 0..=1: how far child tubes blend into the parent near a fork to hide the intersection.
    pub fork_blend: f32,
    /// Extra rings between two skeleton nodes (0..=6) placed on a smooth curve through the nodes, so limbs bend instead of kinking; 0 keeps
    /// the straight segments.
    pub smooth: u32,
    /// 0..=3: how much the trunk swells into lobed roots at the ground (0 is a plain cylinder; 1 roughly doubles the radius at the soil).
    pub flare: f32,
}

/// A chain of meshes of falling detail.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LodChain {
    /// Level 0 is the full mesh.
    pub levels: Vec<PlantMesh>,
    /// For each level after the first: the distance over plant height at which it takes over from the one before (rising).
    pub switch_distance_per_size: Vec<f32>,
}

/// One view of a billboard impostor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImpostorView {
    /// Unit direction the camera looks along for this view.
    pub direction: [f32; 3],
    /// Unit up vector of the view.
    pub up: [f32; 3],
    /// Half the width of the view's frame, in metres.
    pub half_width: f32,
    /// Half the height of the view's frame, in metres.
    pub half_height: f32,
}

/// The views a renderer captures into an impostor atlas.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImpostorSpec {
    /// The views.
    pub views: Vec<ImpostorView>,
    /// Centre of the plant's bounding sphere.
    pub bounds_centre: [f32; 3],
    /// Radius of the bounding sphere.
    pub bounds_radius: f32,
    /// Atlas layout in cells across and down; at least as many cells as views.
    pub atlas_grid: [u32; 2],
}
