//! The frame-time governor: picks a quality level from measured frame times.
//!
//! It steps down one level quickly when the 95th-percentile frame time exceeds the budget, and
//! steps up one level only after a long calm stretch with headroom. Each step down that follows a
//! step up lengthens the next calm wait (backoff), so a level that is almost affordable is not
//! retried every few seconds. Levels are plain indices; the caller maps them to quality settings.

use crate::frame_stats::FrameStats;

/// How often the window is summarised, in frames (sorting every frame would be wasted work).
const CHECK_EVERY: u32 = 6;

/// The calm wait doubles for each backoff step, up to this many doublings.
const MAX_BACKOFF: u32 = 4;

/// Frames of history the decisions are based on.
const WINDOW_FRAMES: usize = 120;

/// Tuning for a [`Governor`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GovernorConfig {
    /// The frame time the display needs, in milliseconds (13.9 for 72 Hz).
    pub budget_ms: f32,
    /// Frames the window must hold before any decision.
    pub min_samples: usize,
    /// Frames to wait after a change before the next one.
    pub cooldown_frames: u32,
    /// A step up needs p95 below `budget_ms * up_headroom`.
    pub up_headroom: f32,
    /// Calm frames needed for a step up (before backoff).
    pub up_hold_frames: u32,
}

impl GovernorConfig {
    /// Defaults for a display refreshing at `hz`: judge after about 1.25 seconds of frames (at most
    /// 90), wait 3 seconds between changes and 8 seconds of 25% headroom before stepping up.
    #[must_use]
    pub fn for_refresh_rate(hz: f32) -> Self {
        let hz = if hz.is_finite() && hz > 0.0 { hz } else { 60.0 };
        Self {
            budget_ms: 1000.0 / hz,
            min_samples: frames_for_seconds(1.25, hz).clamp(8, 90) as usize,
            cooldown_frames: frames_for_seconds(3.0, hz),
            up_headroom: 0.75,
            up_hold_frames: frames_for_seconds(8.0, hz),
        }
    }
}

fn frames_for_seconds(seconds: f32, hz: f32) -> u32 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a small positive frame count"
    )]
    let frames = (seconds * hz).round() as u32;
    frames.max(1)
}

/// Chooses a level in `0..levels` (higher is better quality) from frame times.
#[derive(Clone, Debug)]
pub struct Governor {
    config: GovernorConfig,
    levels: usize,
    level: usize,
    stats: FrameStats,
    since_change: u32,
    since_check: u32,
    calm_frames: u32,
    backoff: u32,
}

impl Governor {
    /// A governor over `levels` levels (at least one), starting at `start` (clamped into range).
    #[must_use]
    pub fn new(levels: usize, start: usize, config: GovernorConfig) -> Self {
        let levels = levels.max(1);
        Self {
            config,
            levels,
            level: start.min(levels - 1),
            stats: FrameStats::new(WINDOW_FRAMES, 1000.0 / config.budget_ms.max(1.0)),
            since_change: 0,
            since_check: 0,
            calm_frames: 0,
            backoff: 0,
        }
    }

    /// The current level.
    #[must_use]
    pub fn level(&self) -> usize {
        self.level
    }

    /// Records one frame time and returns the new level if it changed.
    pub fn observe(&mut self, frame_ms: f32) -> Option<usize> {
        self.stats.record(frame_ms);
        self.since_change = self.since_change.saturating_add(1);
        self.since_check += 1;
        if self.since_check < CHECK_EVERY {
            return None;
        }
        self.since_check = 0;
        let summary = self.stats.summary();
        if self.since_change < self.config.cooldown_frames
            || summary.frames < self.config.min_samples
        {
            return None;
        }
        if summary.p95_ms > self.config.budget_ms {
            self.calm_frames = 0;
            return self.step_down();
        }
        if summary.p95_ms < self.config.budget_ms * self.config.up_headroom {
            self.calm_frames = self.calm_frames.saturating_add(CHECK_EVERY);
            let hold = self.config.up_hold_frames << self.backoff.min(MAX_BACKOFF);
            if self.calm_frames >= hold {
                if self.level + 1 < self.levels {
                    return Some(self.step_up());
                }
                // Settled at the top: forget past trouble.
                self.backoff = self.backoff.saturating_sub(1);
                self.calm_frames = 0;
            }
        } else {
            self.calm_frames = 0;
        }
        None
    }

    fn step_down(&mut self) -> Option<usize> {
        if self.level == 0 {
            return None;
        }
        self.level -= 1;
        self.backoff = (self.backoff + 1).min(MAX_BACKOFF);
        self.restart_judgement();
        Some(self.level)
    }

    fn step_up(&mut self) -> usize {
        self.level += 1;
        self.restart_judgement();
        self.level
    }

    /// Restarts the judgement after a change: old frames describe the old level.
    fn restart_judgement(&mut self) {
        self.stats.clear_window();
        self.since_change = 0;
        self.calm_frames = 0;
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const HZ: f32 = 72.0;

    fn governor(levels: usize, start: usize) -> Governor {
        Governor::new(levels, start, GovernorConfig::for_refresh_rate(HZ))
    }

    /// Frame time of a scene whose cost grows linearly with the level.
    fn cost(level: usize) -> f32 {
        #[expect(clippy::cast_precision_loss, reason = "tiny level index")]
        let level = level as f32;
        8.0 + 4.0 * level
    }

    #[test]
    fn steady_good_frames_at_the_top_change_nothing() {
        let mut governor = governor(3, 2);
        assert!((0..5000).all(|_| governor.observe(8.0).is_none()));
        assert_eq!(governor.level(), 2);
    }

    #[test]
    fn overload_steps_down_one_level_at_a_time_and_stops_at_zero() {
        let mut governor = governor(3, 2);
        let mut seen = Vec::new();
        for _ in 0..5000 {
            if let Some(level) = governor.observe(30.0) {
                seen.push(level);
            }
        }
        assert_eq!(seen, [1, 0]);
        assert_eq!(governor.level(), 0);
    }

    #[test]
    fn calm_frames_step_up_one_level_at_a_time() {
        let mut governor = governor(3, 0);
        let mut seen = Vec::new();
        for _ in 0..20_000 {
            if let Some(level) = governor.observe(6.0) {
                seen.push(level);
            }
        }
        assert_eq!(seen, [1, 2]);
    }

    #[test]
    fn a_borderline_level_is_not_retried_in_a_loop() {
        // Level 0 costs 8 ms, 1 costs 12 ms, 2 costs 16 ms against a 13.9 ms budget: level 1 is right.
        let mut governor = governor(3, 2);
        let mut changes = 0;
        let mut frames_at_top = 0;
        let total = 40_000;
        for _ in 0..total {
            if governor.observe(cost(governor.level())).is_some() {
                changes += 1;
            }
            if governor.level() == 2 {
                frames_at_top += 1;
            }
        }
        assert_eq!(
            governor.level(),
            1,
            "it should settle on the affordable level"
        );
        assert!(
            changes <= 8,
            "{changes} changes: the governor is oscillating"
        );
        assert!(
            frames_at_top < total / 5,
            "{frames_at_top} frames spent over budget"
        );
    }

    #[test]
    fn it_recovers_when_the_load_goes_away() {
        let mut governor = governor(3, 2);
        for _ in 0..3000 {
            governor.observe(30.0);
        }
        assert_eq!(governor.level(), 0);
        for _ in 0..60_000 {
            governor.observe(6.0);
        }
        assert_eq!(governor.level(), 2);
    }

    #[test]
    fn construction_clamps_the_start_level_and_the_level_count() {
        assert_eq!(governor(3, 99).level(), 2);
        assert_eq!(governor(0, 5).level(), 0);
    }

    #[test]
    fn config_matches_the_refresh_rate() {
        let config = GovernorConfig::for_refresh_rate(72.0);
        assert!((config.budget_ms - 13.888_889).abs() < 1e-3);
        assert_eq!(config.cooldown_frames, 216);
        assert_eq!(config.up_hold_frames, 576);
        assert_eq!(config.min_samples, 90);
        // A slow display needs fewer frames to judge, but never fewer than eight.
        assert_eq!(GovernorConfig::for_refresh_rate(5.0).min_samples, 8);
        assert!((GovernorConfig::for_refresh_rate(0.0).budget_ms - 1000.0 / 60.0).abs() < 1e-3);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]

        #[test]
        fn any_input_keeps_the_level_in_range_and_changes_spaced_by_the_cooldown(
            samples in prop::collection::vec(prop_oneof![
                0.0_f32..60.0,
                Just(f32::NAN),
                Just(f32::INFINITY),
                Just(-5.0_f32),
            ], 0..3000),
            levels in 1_usize..6,
            start in 0_usize..8,
        ) {
            let config = GovernorConfig::for_refresh_rate(HZ);
            let mut governor = Governor::new(levels, start, config);
            let mut last_change: Option<usize> = None;
            for (frame, ms) in samples.into_iter().enumerate() {
                let before = governor.level();
                let changed = governor.observe(ms);
                prop_assert!(governor.level() < levels);
                if let Some(level) = changed {
                    prop_assert_eq!(level, governor.level());
                    prop_assert_eq!(before.abs_diff(level), 1);
                    if let Some(previous) = last_change {
                        prop_assert!(frame - previous >= config.cooldown_frames as usize);
                    }
                    last_change = Some(frame);
                } else {
                    prop_assert_eq!(before, governor.level());
                }
            }
        }
    }
}
