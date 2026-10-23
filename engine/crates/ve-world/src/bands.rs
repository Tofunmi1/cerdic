//! Deterministic distance bands for spatial chunks of instance sets (ADR 0058).

/// The rendering representation selected for a chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    /// Full detail, with one entity per copy.
    Near,
    /// Reduced detail, merged into one chunk mesh.
    Mid,
    /// Impostor representation, merged into one chunk mesh.
    Far,
    /// Outside the draw range.
    Culled,
}

/// Distance boundaries, outward hysteresis, and the full-detail copy budget.
#[derive(Clone, Copy, Debug)]
pub struct BandConfig {
    /// Chunks closer than this distance are candidates for [`Band::Near`].
    pub near: f32,
    /// Chunks closer than this distance are candidates for [`Band::Mid`].
    pub mid: f32,
    /// Chunks closer than this distance are candidates for [`Band::Far`].
    pub far: f32,
    /// Fraction by which a band's outer edge is extended while moving outward.
    pub hysteresis: f32,
    /// Maximum total copies represented by [`Band::Near`].
    pub near_budget: usize,
}

/// Spatial bounds and copy count for one chunk.
#[derive(Clone, Copy, Debug)]
pub struct ChunkInfo {
    /// Chunk centre in world coordinates.
    pub centre: [f32; 3],
    /// Bounding sphere radius.
    pub radius: f32,
    /// Number of copies contained in the chunk.
    pub copies: usize,
}

/// Stateful band scheduler. Previous bands are tracked by chunk index.
#[derive(Debug)]
pub struct Scheduler {
    config: BandConfig,
    previous: Vec<Band>,
}

impl Scheduler {
    /// Creates a scheduler with the supplied distance thresholds and budget.
    #[must_use]
    pub fn new(config: BandConfig) -> Self {
        Self {
            config,
            previous: Vec::new(),
        }
    }

    /// Selects a band for every chunk, using the closest eye and prior-frame hysteresis.
    #[must_use]
    pub fn update(&mut self, chunks: &[ChunkInfo], eyes: &[[f32; 3]]) -> Vec<Band> {
        let mut distances = Vec::with_capacity(chunks.len());
        let mut bands = Vec::with_capacity(chunks.len());

        for (index, chunk) in chunks.iter().enumerate() {
            let Some(distance) = nearest_distance(chunk, eyes) else {
                distances.push(f32::INFINITY);
                bands.push(Band::Culled);
                continue;
            };
            distances.push(distance);
            let previous = self.previous.get(index).copied();
            bands.push(self.classify(distance, previous));
        }

        self.apply_near_budget(chunks, &distances, &mut bands);
        self.previous.clone_from(&bands);
        bands
    }

    fn classify(&self, distance: f32, previous: Option<Band>) -> Band {
        let config = self.config;
        match previous {
            Some(Band::Near)
                if distance < config.near
                    || distance <= config.near * (1.0 + config.hysteresis) =>
            {
                Band::Near
            }
            Some(Band::Mid)
                if distance >= config.near
                    && distance <= config.mid * (1.0 + config.hysteresis) =>
            {
                Band::Mid
            }
            Some(Band::Far)
                if distance >= config.mid && distance <= config.far * (1.0 + config.hysteresis) =>
            {
                Band::Far
            }
            _ if distance < config.near => Band::Near,
            _ if distance < config.mid => Band::Mid,
            _ if distance < config.far => Band::Far,
            _ => Band::Culled,
        }
    }

    fn apply_near_budget(&self, chunks: &[ChunkInfo], distances: &[f32], bands: &mut [Band]) {
        let mut candidates: Vec<usize> = bands
            .iter()
            .enumerate()
            .filter_map(|(index, band)| (*band == Band::Near).then_some(index))
            .collect();
        candidates.sort_by(|left, right| {
            distances[*left]
                .total_cmp(&distances[*right])
                .then_with(|| left.cmp(right))
        });

        let mut used = 0usize;
        for index in candidates {
            let copies = chunks[index].copies;
            if copies > self.config.near_budget.saturating_sub(used) {
                bands[index] = Band::Mid;
            } else {
                used += copies;
            }
        }
    }
}

/// Counts copy and chunk costs for an already selected band list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BandCounts {
    /// Copies drawn at full detail.
    pub near_copies: usize,
    /// Chunks drawn with reduced detail.
    pub mid_chunks: usize,
    /// Chunks drawn as impostors.
    pub far_chunks: usize,
    /// Chunks outside the draw range.
    pub culled_chunks: usize,
}

/// Summarizes the rendering cost of the supplied bands.
#[must_use]
pub fn summary(chunks: &[ChunkInfo], bands: &[Band]) -> BandCounts {
    let mut counts = BandCounts::default();
    for (chunk, band) in chunks.iter().zip(bands) {
        match band {
            Band::Near => counts.near_copies = counts.near_copies.saturating_add(chunk.copies),
            Band::Mid => counts.mid_chunks = counts.mid_chunks.saturating_add(1),
            Band::Far => counts.far_chunks = counts.far_chunks.saturating_add(1),
            Band::Culled => counts.culled_chunks = counts.culled_chunks.saturating_add(1),
        }
    }
    counts
}

fn nearest_distance(chunk: &ChunkInfo, eyes: &[[f32; 3]]) -> Option<f32> {
    eyes.iter()
        .map(|eye| {
            let dx = chunk.centre[0] - eye[0];
            let dy = chunk.centre[1] - eye[1];
            let dz = chunk.centre[2] - eye[2];
            dx.hypot(dy).hypot(dz)
        })
        .reduce(f32::min)
        .map(|distance| (distance - chunk.radius).max(0.0))
}
