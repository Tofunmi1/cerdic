//! What the renderer actually submitted last frame, counted where the draw calls are issued. A
//! benchmark must report these rather than the scene's authored object count: a stereo frame draws
//! every batch once per eye, and the shadow pass draws the batches again.

use std::cell::Cell;

/// Draw calls and triangles submitted by one pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PassCounts {
    /// Draw calls issued (one per batch per view).
    pub draws: u32,
    /// Triangles those calls rasterised, counting each instance.
    pub triangles: u64,
}

/// The counts for one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameCounts {
    /// The sun's shadow pass.
    pub shadow: PassCounts,
    /// The main forward pass, summed over its views (both eyes in stereo).
    pub main: PassCounts,
}

thread_local! {
    static CURRENT: Cell<FrameCounts> = const { Cell::new(FrameCounts {
        shadow: PassCounts { draws: 0, triangles: 0 },
        main: PassCounts { draws: 0, triangles: 0 },
    }) };
    static LAST: Cell<FrameCounts> = const { Cell::new(FrameCounts {
        shadow: PassCounts { draws: 0, triangles: 0 },
        main: PassCounts { draws: 0, triangles: 0 },
    }) };
}

/// The counts of the last completed frame (the engine runs on one thread in the browser).
#[must_use]
pub fn last_frame_counts() -> FrameCounts {
    LAST.with(Cell::get)
}

/// Closes the frame being counted and starts a new one. The shadow pass runs first in each frame.
pub(crate) fn begin_frame() {
    CURRENT.with(|current| LAST.with(|last| last.set(current.replace(FrameCounts::default()))));
}

/// Which pass a draw belongs to.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Pass {
    Shadow,
    Main,
}

/// Records one draw call of `triangles` triangles.
pub(crate) fn record(pass: Pass, triangles: u64) {
    CURRENT.with(|current| {
        let mut counts = current.get();
        let target = match pass {
            Pass::Shadow => &mut counts.shadow,
            Pass::Main => &mut counts.main,
        };
        target.draws = target.draws.saturating_add(1);
        target.triangles = target.triangles.saturating_add(triangles);
        current.set(counts);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_belong_to_the_frame_that_was_closed() {
        begin_frame();
        record(Pass::Shadow, 100);
        record(Pass::Main, 100);
        record(Pass::Main, 100);
        // Still being counted: the last completed frame is the empty one before it.
        assert_eq!(last_frame_counts(), FrameCounts::default());
        begin_frame();
        let counts = last_frame_counts();
        assert_eq!(
            counts.shadow,
            PassCounts {
                draws: 1,
                triangles: 100
            }
        );
        assert_eq!(
            counts.main,
            PassCounts {
                draws: 2,
                triangles: 200
            }
        );
        begin_frame();
        assert_eq!(last_frame_counts(), FrameCounts::default());
    }
}
