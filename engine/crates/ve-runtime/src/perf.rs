//! Frame-time measurement and the quality governor, for the status report and for benchmarking on a
//! headset. The statistics and the governor's decisions live in `ve-core`; this is the glue.

use std::cell::{Cell, RefCell};

use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Res, ResMut};
use bevy_time::{Real, Time};
use serde_json::json;
use ve_core::{FrameStats, Governor, GovernorConfig};
use ve_render::{QualityTier, RenderQuality};
use ve_xr_web::{SessionState, refresh_hz, session_state};

use crate::quality;

/// Refresh rate assumed for a headset session whose runtime does not report one.
const DEFAULT_HEADSET_HZ: f32 = 72.0;

/// Refresh rate assumed for the page's own window unless the page says otherwise.
const DEFAULT_WINDOW_HZ: f32 = 60.0;

/// Frames a governor request may stay unapplied before it is considered lost.
const PENDING_FRAMES: u64 = 120;

/// Tier changes kept for the report.
const MAX_CHANGES: usize = 24;

thread_local! {
    static LAST_SESSION: RefCell<Option<serde_json::Value>> = const { RefCell::new(None) };
    static GOVERNOR_ON: Cell<bool> = const { Cell::new(false) };
    static WINDOW_HZ: Cell<f32> = const { Cell::new(DEFAULT_WINDOW_HZ) };
}

/// Sets the refresh rate the page's own window is judged against (frame budget and governor
/// timing). Non-positive or non-finite values restore the default.
pub(crate) fn set_window_hz(hz: f32) {
    WINDOW_HZ.with(|cell| {
        cell.set(if hz.is_finite() && hz > 0.0 {
            hz
        } else {
            DEFAULT_WINDOW_HZ
        });
    });
}

/// Turns the automatic quality governor on or off.
pub(crate) fn set_governor(enabled: bool) {
    GOVERNOR_ON.with(|flag| flag.set(enabled));
}

fn level_of(tier: QualityTier) -> usize {
    QualityTier::ALL
        .iter()
        .position(|candidate| *candidate == tier)
        .unwrap_or(0)
}

struct TierChange {
    frame: u64,
    tier: QualityTier,
}

/// Frame-time statistics, the governor and what the governor did.
#[derive(Resource)]
pub(crate) struct Perf {
    stats: FrameStats,
    governor: Governor,
    hz: f32,
    presenting: bool,
    frame: u64,
    /// The tier the governor asked for that has not been applied yet.
    pending: Option<QualityTier>,
    pending_since: u64,
    changes: Vec<TierChange>,
}

impl Default for Perf {
    fn default() -> Self {
        Self::new(WINDOW_HZ.with(Cell::get), false, QualityTier::default())
    }
}

impl Perf {
    fn new(hz: f32, presenting: bool, tier: QualityTier) -> Self {
        Self {
            stats: FrameStats::new(240, hz),
            governor: Governor::new(
                QualityTier::ALL.len(),
                level_of(tier),
                GovernorConfig::for_refresh_rate(hz),
            ),
            hz,
            presenting,
            frame: 0,
            pending: None,
            pending_since: 0,
            changes: Vec::new(),
        }
    }

    /// The figures for the status report, plus those of the last headset session that ended.
    pub(crate) fn report(&self) -> serde_json::Value {
        let mut value = self.summary();
        value["last_session"] = LAST_SESSION.with(|slot| slot.borrow().clone().into());
        value
    }

    /// The current figures alone.
    fn summary(&self) -> serde_json::Value {
        let summary = self.stats.summary();
        let (total_frames, total_dropped) = self.stats.totals();
        json!({
            "target_hz": self.hz,
            "presenting": self.presenting,
            "window_frames": summary.frames,
            "fps": summary.fps(),
            "mean_ms": summary.mean_ms,
            "p50_ms": summary.p50_ms,
            "p95_ms": summary.p95_ms,
            "max_ms": summary.max_ms,
            "dropped": summary.dropped,
            "total_frames": total_frames,
            "total_dropped": total_dropped,
            "missed_periods": self.stats.missed_periods(),
            "governor": {
                "enabled": GOVERNOR_ON.with(Cell::get),
                "level": self.governor.level(),
                "changes": self
                    .changes
                    .iter()
                    .map(|c| json!({ "frame": c.frame, "tier": c.tier.name() }))
                    .collect::<Vec<_>>(),
            },
        })
    }
}

/// Records each frame's real duration, and lets the governor change the quality tier when enabled.
///
/// Statistics restart when the app moves between the page's window and a headset session, because
/// the two have different refresh rates and are not comparable.
pub(crate) fn track_frame_time(
    time: Res<Time<Real>>,
    quality: Res<RenderQuality>,
    mut perf: ResMut<Perf>,
) {
    let presenting = session_state() == SessionState::Presenting;
    let hz = if presenting {
        refresh_hz().unwrap_or(DEFAULT_HEADSET_HZ)
    } else {
        WINDOW_HZ.with(Cell::get)
    };
    if presenting != perf.presenting || (hz - perf.hz).abs() > f32::EPSILON {
        // Keep the figures of a headset session that just ended, exactly as they stood: the page
        // polls too slowly to be sure it saw the last frames before the statistics restart.
        if perf.presenting {
            let ended = perf.summary();
            LAST_SESSION.with(|slot| *slot.borrow_mut() = Some(ended));
        }
        *perf = Perf::new(hz, presenting, quality.tier);
    }
    perf.frame += 1;
    let frame_ms = time.delta_secs() * 1000.0;
    // The first frame has no previous frame to measure against.
    if perf.frame <= 1 {
        return;
    }
    perf.stats.record(frame_ms);

    if let Some(pending) = perf.pending {
        // A request the page overrode (or that never lands) must not block the governor for good.
        if quality.tier == pending || perf.frame - perf.pending_since > PENDING_FRAMES {
            perf.pending = None;
        }
        return;
    }
    if !GOVERNOR_ON.with(Cell::get) {
        return;
    }
    // Someone else changed the tier (the page, a test): follow it instead of fighting it.
    if level_of(quality.tier) != perf.governor.level() {
        perf.governor = Governor::new(
            QualityTier::ALL.len(),
            level_of(quality.tier),
            GovernorConfig::for_refresh_rate(perf.hz),
        );
    }
    if let Some(level) = perf.governor.observe(frame_ms) {
        let tier = QualityTier::ALL[level.min(QualityTier::ALL.len() - 1)];
        quality::request(tier);
        perf.pending = Some(tier);
        let frame = perf.frame;
        perf.pending_since = frame;
        if perf.changes.len() == MAX_CHANGES {
            perf.changes.remove(0);
        }
        perf.changes.push(TierChange { frame, tier });
    }
}
