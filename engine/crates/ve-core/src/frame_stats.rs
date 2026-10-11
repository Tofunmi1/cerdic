//! Rolling frame-time statistics: what the frame-time overlay and the quality governor read.

use std::collections::VecDeque;

/// A frame counts as dropped when it takes this many times the target frame time: the display
/// missed at least one refresh.
pub const DROPPED_FRAME_FACTOR: f32 = 1.5;

/// A summary of the most recent frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameSummary {
    /// Frames in the window the figures below cover.
    pub frames: usize,
    /// Mean frame time in milliseconds.
    pub mean_ms: f32,
    /// Median frame time in milliseconds.
    pub p50_ms: f32,
    /// 95th percentile frame time in milliseconds.
    pub p95_ms: f32,
    /// Slowest frame in the window, in milliseconds.
    pub max_ms: f32,
    /// Dropped frames in the window.
    pub dropped: usize,
}

impl FrameSummary {
    /// Frames per second implied by the mean frame time (zero when there are no frames).
    #[must_use]
    pub fn fps(&self) -> f32 {
        if self.mean_ms > 0.0 {
            1000.0 / self.mean_ms
        } else {
            0.0
        }
    }
}

/// The last `capacity` frame times, plus lifetime counters.
#[derive(Clone, Debug)]
pub struct FrameStats {
    window: VecDeque<f32>,
    capacity: usize,
    target_ms: f32,
    total_frames: u64,
    total_dropped: u64,
    total_missed_periods: u64,
}

impl FrameStats {
    /// Statistics over a window of `capacity` frames for a display refreshing at `target_hz`.
    /// A capacity of zero is treated as one, and a non-positive rate as 60 Hz.
    #[must_use]
    pub fn new(capacity: usize, target_hz: f32) -> Self {
        let hz = if target_hz.is_finite() && target_hz > 0.0 {
            target_hz
        } else {
            60.0
        };
        let capacity = capacity.max(1);
        Self {
            window: VecDeque::with_capacity(capacity),
            capacity,
            target_ms: 1000.0 / hz,
            total_frames: 0,
            total_dropped: 0,
            total_missed_periods: 0,
        }
    }

    /// The frame time the display needs, in milliseconds.
    #[must_use]
    pub fn target_ms(&self) -> f32 {
        self.target_ms
    }

    /// Records one frame. Non-finite or negative times are ignored (a clock glitch must not poison
    /// the statistics).
    pub fn record(&mut self, frame_ms: f32) {
        if !frame_ms.is_finite() || frame_ms < 0.0 {
            return;
        }
        if self.window.len() == self.capacity {
            self.window.pop_front();
        }
        self.window.push_back(frame_ms);
        self.total_frames += 1;
        if frame_ms > self.target_ms * DROPPED_FRAME_FACTOR {
            self.total_dropped += 1;
            // A long interval spans several refreshes, and each one was a frame the display missed.
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a non-negative count of refresh periods"
            )]
            let periods = (frame_ms / self.target_ms).round() as u64;
            self.total_missed_periods += periods.saturating_sub(1);
        }
    }

    /// Refresh periods the display missed since the start, counting every period a long frame
    /// spanned (a 50 ms frame at 72 Hz is three missed periods, though it is one dropped frame).
    /// This estimates compositor behaviour from frame intervals; it is not compositor telemetry.
    #[must_use]
    pub fn missed_periods(&self) -> u64 {
        self.total_missed_periods
    }

    /// Forgets the window (not the lifetime counters), for example after a quality change when old
    /// frames no longer describe the new settings.
    pub fn clear_window(&mut self) {
        self.window.clear();
    }

    /// Frames recorded since the start, and how many of them were dropped.
    #[must_use]
    pub fn totals(&self) -> (u64, u64) {
        (self.total_frames, self.total_dropped)
    }

    /// Summary of the frames currently in the window.
    #[must_use]
    pub fn summary(&self) -> FrameSummary {
        if self.window.is_empty() {
            return FrameSummary::default();
        }
        let mut sorted: Vec<f32> = self.window.iter().copied().collect();
        sorted.sort_by(f32::total_cmp);
        let count = sorted.len();
        let sum: f32 = sorted.iter().sum();
        let dropped_over = self.target_ms * DROPPED_FRAME_FACTOR;
        FrameSummary {
            frames: count,
            #[expect(
                clippy::cast_precision_loss,
                reason = "the window holds a few hundred frames"
            )]
            mean_ms: sum / count as f32,
            p50_ms: percentile(&sorted, 50),
            p95_ms: percentile(&sorted, 95),
            max_ms: sorted.last().copied().unwrap_or(0.0),
            dropped: sorted.iter().filter(|ms| **ms > dropped_over).count(),
        }
    }
}

/// Nearest-rank percentile of a sorted, non-empty slice.
fn percentile(sorted: &[f32], percent: usize) -> f32 {
    let rank = (percent * sorted.len())
        .div_ceil(100)
        .clamp(1, sorted.len());
    sorted[rank - 1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_stats_summarise_to_zero() {
        let stats = FrameStats::new(10, 72.0);
        assert_eq!(stats.summary(), FrameSummary::default());
        assert!(stats.summary().fps().abs() < f32::EPSILON);
        assert_eq!(stats.totals(), (0, 0));
    }

    #[test]
    fn percentiles_use_nearest_rank() {
        let mut stats = FrameStats::new(100, 72.0);
        for ms in 1..=100_u8 {
            stats.record(f32::from(ms));
        }
        let summary = stats.summary();
        assert!((summary.p50_ms - 50.0).abs() < f32::EPSILON);
        assert!((summary.p95_ms - 95.0).abs() < f32::EPSILON);
        assert!((summary.max_ms - 100.0).abs() < f32::EPSILON);
        assert!((summary.mean_ms - 50.5).abs() < 1e-4);
    }

    #[test]
    fn the_window_forgets_old_frames_but_totals_do_not() {
        let mut stats = FrameStats::new(4, 72.0);
        for _ in 0..10 {
            stats.record(30.0);
        }
        for _ in 0..4 {
            stats.record(10.0);
        }
        let summary = stats.summary();
        assert_eq!(summary.frames, 4);
        assert!((summary.max_ms - 10.0).abs() < f32::EPSILON);
        assert_eq!(summary.dropped, 0);
        assert_eq!(stats.totals(), (14, 10));
    }

    #[test]
    fn a_frame_over_one_and_a_half_targets_is_dropped() {
        let mut stats = FrameStats::new(10, 60.0);
        stats.record(16.0);
        stats.record(24.0);
        stats.record(26.0);
        assert_eq!(stats.summary().dropped, 1);
        assert_eq!(stats.totals(), (3, 1));
    }

    #[test]
    fn a_long_frame_counts_every_refresh_it_spanned() {
        let mut stats = FrameStats::new(10, 72.0);
        let period = 1000.0 / 72.0;
        stats.record(period);
        stats.record(period * 1.2);
        assert_eq!(stats.missed_periods(), 0);
        // One frame, three periods long: it missed two refreshes.
        stats.record(period * 3.0);
        assert_eq!(stats.missed_periods(), 2);
        assert_eq!(stats.totals(), (3, 1));
        stats.record(period * 2.0);
        assert_eq!(stats.missed_periods(), 3);
    }

    #[test]
    fn bad_samples_are_ignored() {
        let mut stats = FrameStats::new(10, 60.0);
        stats.record(f32::NAN);
        stats.record(f32::INFINITY);
        stats.record(-1.0);
        assert_eq!(stats.totals(), (0, 0));
        assert_eq!(stats.summary().frames, 0);
    }

    #[test]
    fn degenerate_construction_is_safe() {
        let mut stats = FrameStats::new(0, 0.0);
        stats.record(10.0);
        stats.record(12.0);
        assert_eq!(stats.summary().frames, 1);
        assert!((stats.target_ms() - 1000.0 / 60.0).abs() < 1e-4);
    }

    #[test]
    fn clearing_the_window_keeps_the_totals() {
        let mut stats = FrameStats::new(10, 60.0);
        stats.record(40.0);
        stats.clear_window();
        assert_eq!(stats.summary().frames, 0);
        assert_eq!(stats.totals(), (1, 1));
    }
}
