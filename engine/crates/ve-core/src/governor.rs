//! The frame-time governor: picks a quality level from measured frame times.
//!
//! The only measurement is the interval between frames. On a display-paced loop that interval stays
//! at one refresh period however cheap the frame is, so spare capacity is invisible; only a missed
//! refresh shows (the interval jumps to two periods). The governor therefore works in two moves:
//!
//! - **Down, quickly**: when the 95th-percentile interval exceeds the refresh period by more than a
//!   small margin (the margin absorbs timer jitter), it steps down one level.
//! - **Up, as a trial**: after a long stretch with no overload it steps up one level, then judges the
//!   result like any other. If the trial misses refreshes it steps back down, and each step down
//!   lengthens the next calm wait (backoff), so a level that is almost affordable is not retried
//!   every few seconds.
//!
//! All waits are measured in elapsed milliseconds, not frames, and a pause (a hidden tab, a
//! suspended session) restarts the judgement instead of counting as a very slow frame. Levels are
//! plain indices; the caller maps them to quality settings.

use crate::frame_stats::FrameStats;

/// How often the window is summarised, in frames (sorting every frame would be wasted work).
const CHECK_EVERY: u32 = 6;

/// The calm wait doubles for each backoff step, up to this many doublings.
const MAX_BACKOFF: u32 = 4;

/// Frames of history the decisions are based on.
const WINDOW_FRAMES: usize = 120;

/// An interval longer than this is a pause (hidden tab, suspended session), not a slow frame.
const PAUSE_MS: f32 = 1000.0;

/// Tuning for a [`Governor`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GovernorConfig {
    /// The frame time the display needs, in milliseconds (13.9 for 72 Hz).
    pub budget_ms: f32,
    /// Frames the window must hold before any decision.
    pub min_samples: usize,
    /// Milliseconds to wait after a change before the next one.
    pub cooldown_ms: f32,
    /// The 95th-percentile interval counts as overload above `budget_ms * overload_margin`.
    pub overload_margin: f32,
    /// Milliseconds without overload before a step up (before backoff).
    pub up_hold_ms: f32,
}

impl GovernorConfig {
    /// Defaults for a display refreshing at `hz`: judge after about 1.25 seconds of frames (at most
    /// 90), wait 3 seconds between changes, call overload at 10% over the refresh period and wait
    /// 8 seconds without overload before trying a higher level.
    #[must_use]
    pub fn for_refresh_rate(hz: f32) -> Self {
        let hz = if hz.is_finite() && hz > 0.0 { hz } else { 60.0 };
        Self {
            budget_ms: 1000.0 / hz,
            min_samples: frames_for_seconds(1.25, hz).clamp(8, 90) as usize,
            cooldown_ms: 3000.0,
            overload_margin: 1.1,
            up_hold_ms: 8000.0,
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
    since_change_ms: f32,
    since_check: u32,
    check_ms: f32,
    calm_ms: f32,
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
            since_change_ms: 0.0,
            since_check: 0,
            check_ms: 0.0,
            calm_ms: 0.0,
            backoff: 0,
        }
    }

    /// The current level.
    #[must_use]
    pub fn level(&self) -> usize {
        self.level
    }

    /// Forgets what has been measured, for example when the page became visible again or the
    /// refresh rate changed. The level and the backoff stay.
    pub fn restart(&mut self) {
        self.restart_judgement();
        self.since_check = 0;
        self.check_ms = 0.0;
    }

    /// Records one frame interval and returns the new level if it changed. Non-finite or negative
    /// intervals are ignored; an interval over a second is a pause and restarts the judgement.
    pub fn observe(&mut self, frame_ms: f32) -> Option<usize> {
        if !frame_ms.is_finite() || frame_ms < 0.0 {
            return None;
        }
        if frame_ms > PAUSE_MS {
            self.restart();
            return None;
        }
        self.stats.record(frame_ms);
        self.since_change_ms += frame_ms;
        self.check_ms += frame_ms;
        self.since_check += 1;
        if self.since_check < CHECK_EVERY {
            return None;
        }
        self.since_check = 0;
        let waited = std::mem::take(&mut self.check_ms);
        let summary = self.stats.summary();
        if self.since_change_ms < self.config.cooldown_ms
            || summary.frames < self.config.min_samples
        {
            return None;
        }
        if summary.p95_ms > self.config.budget_ms * self.config.overload_margin {
            self.calm_ms = 0.0;
            return self.step_down();
        }
        self.calm_ms += waited;
        let hold = self.config.up_hold_ms * f32::from(1_u16 << self.backoff.min(MAX_BACKOFF));
        if self.calm_ms >= hold {
            if self.level + 1 < self.levels {
                return Some(self.step_up());
            }
            // Settled at the top: forget past trouble.
            self.backoff = self.backoff.saturating_sub(1);
            self.calm_ms = 0.0;
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
        self.since_change_ms = 0.0;
        self.calm_ms = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const HZ: f32 = 72.0;
    /// One refresh period at 72 Hz.
    const PERIOD: f32 = 1000.0 / HZ;

    fn governor(levels: usize, start: usize) -> Governor {
        Governor::new(levels, start, GovernorConfig::for_refresh_rate(HZ))
    }

    /// The interval a display-paced loop reports for a scene whose cost grows with the level: never
    /// shorter than one refresh period, however cheap the frame.
    fn paced(level: usize) -> f32 {
        #[expect(clippy::cast_precision_loss, reason = "tiny level index")]
        let cost = 8.0 + 4.0 * level as f32;
        cost.max(PERIOD)
    }

    #[test]
    fn steady_good_frames_at_the_top_change_nothing() {
        let mut governor = governor(3, 2);
        assert!((0..5000).all(|_| governor.observe(PERIOD).is_none()));
        assert_eq!(governor.level(), 2);
    }

    #[test]
    fn timer_jitter_around_the_refresh_period_is_not_overload() {
        let mut governor = governor(3, 2);
        for i in 0..20_000 {
            let jitter = if i % 2 == 0 { 0.1 } else { -0.1 };
            assert!(governor.observe(PERIOD + jitter).is_none(), "frame {i}");
        }
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
    fn a_few_missed_refreshes_are_not_overload() {
        // One missed refresh in twenty is under the 95th percentile.
        let mut governor = governor(3, 2);
        for i in 0..20_000 {
            let ms = if i % 25 == 0 { 2.0 * PERIOD } else { PERIOD };
            assert!(governor.observe(ms).is_none(), "frame {i}");
        }
    }

    #[test]
    fn calm_paced_frames_step_up_one_level_at_a_time() {
        let mut governor = governor(3, 0);
        let mut seen = Vec::new();
        for _ in 0..20_000 {
            if let Some(level) = governor.observe(PERIOD) {
                seen.push(level);
            }
        }
        assert_eq!(seen, [1, 2]);
    }

    #[test]
    fn a_borderline_level_is_not_retried_in_a_loop() {
        // Levels cost 8, 12 and 16 ms against a 13.9 ms period: level 1 is right.
        let mut governor = governor(3, 2);
        let mut changes = 0;
        let mut ms_at_top = 0.0_f32;
        let mut total = 0.0_f32;
        // About nine minutes: the trials back off from 16 s to 128 s apart, so a trial (up, then
        // down again) happens only a handful of times.
        for _ in 0..40_000 {
            let ms = paced(governor.level());
            if governor.observe(ms).is_some() {
                changes += 1;
            }
            total += ms;
            if governor.level() == 2 {
                ms_at_top += ms;
            }
        }
        assert_eq!(
            governor.level(),
            1,
            "it should settle on the affordable level"
        );
        assert!(
            changes <= 14,
            "{changes} changes: the governor is oscillating"
        );
        assert!(
            ms_at_top < total / 5.0,
            "{ms_at_top} ms spent over budget of {total}"
        );
    }

    #[test]
    fn it_recovers_at_a_healthy_72_hz_cadence_after_an_overload() {
        // The regression from the audit: the load goes away but the loop stays paced at one period.
        let mut governor = governor(3, 2);
        for _ in 0..3000 {
            governor.observe(30.0);
        }
        assert_eq!(governor.level(), 0);
        for _ in 0..60_000 {
            governor.observe(PERIOD);
        }
        assert_eq!(governor.level(), 2);
    }

    #[test]
    fn a_pause_restarts_the_judgement_instead_of_counting_as_overload() {
        let mut governor = governor(3, 2);
        for round in 0..200 {
            for _ in 0..50 {
                assert!(governor.observe(PERIOD).is_none());
            }
            // The tab was hidden for a while.
            assert!(governor.observe(60_000.0).is_none(), "pause {round}");
        }
        assert_eq!(governor.level(), 2);
    }

    #[test]
    fn restart_forgets_the_window_but_not_the_level() {
        let mut governor = governor(3, 2);
        for _ in 0..80 {
            governor.observe(PERIOD);
        }
        governor.restart();
        // Slow frames right after a restart cannot count until the window has refilled.
        for _ in 0..50 {
            assert!(governor.observe(40.0).is_none());
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
        assert!((config.cooldown_ms - 3000.0).abs() < f32::EPSILON);
        assert!((config.up_hold_ms - 8000.0).abs() < f32::EPSILON);
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
                Just(5_000.0_f32),
            ], 0..3000),
            levels in 1_usize..6,
            start in 0_usize..8,
        ) {
            let config = GovernorConfig::for_refresh_rate(HZ);
            let mut governor = Governor::new(levels, start, config);
            let mut elapsed = 0.0_f32;
            let mut last_change: Option<f32> = None;
            for ms in samples {
                let before = governor.level();
                let changed = governor.observe(ms);
                if ms.is_finite() && ms >= 0.0 {
                    elapsed += ms;
                }
                prop_assert!(governor.level() < levels);
                if let Some(level) = changed {
                    prop_assert_eq!(level, governor.level());
                    prop_assert_eq!(before.abs_diff(level), 1);
                    if let Some(previous) = last_change {
                        prop_assert!(elapsed - previous >= config.cooldown_ms - 1.0);
                    }
                    last_change = Some(elapsed);
                } else {
                    prop_assert_eq!(before, governor.level());
                }
            }
        }
    }
}
