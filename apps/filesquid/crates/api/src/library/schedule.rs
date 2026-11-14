//! When a host's next expiry run falls due (Task API-F2.30; API-S-019 **Routes and expiry
//! runs**, DE-S-014).
//!
//! Expiry runs at the host's launch and then at the installation's expiry interval
//! ([`super::InstallationSettings::expiry_interval_minutes`]). A host keeps one
//! [`ExpirySchedule`] for its lifetime and asks it, with the interval it reads from the
//! settings store at that moment, whether a run is due. The schedule keeps only the start of
//! the last run, never an interval, so a changed interval takes effect for the next run
//! without a restart: that run falls due one new interval after the start of the last run,
//! or at once when that time has already passed.
//!
//! Everything here is pure: the host supplies the times (milliseconds on any one clock that
//! does not go backwards, for example the time since launch) and does the run.

use super::policy::{MAX_EXPIRY_INTERVAL_MINUTES, MIN_EXPIRY_INTERVAL_MINUTES};

/// Milliseconds in one minute.
const MINUTE_MS: i64 = 60_000;

/// The start of a host's last expiry run, from which the next one falls due.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExpirySchedule {
    /// When the last run started, in the host's clock milliseconds; `None` before the first
    /// run of this launch.
    last_started_ms: Option<i64>,
}

impl ExpirySchedule {
    /// The schedule of a host that has just launched: its first run is due at once.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that a run started at `now_ms`.
    pub fn record_start(&mut self, now_ms: i64) {
        self.last_started_ms = Some(now_ms);
    }

    /// When the last run started, or `None` before the first run of this launch.
    pub fn last_started_ms(&self) -> Option<i64> {
        self.last_started_ms
    }

    /// When the next run falls due under `interval_minutes`: one interval after the start of
    /// the last run, or `None` when nothing ran yet (the run at launch is due at once). An
    /// interval outside 5 to 1,440 minutes is held to those bounds, so a damaged store can
    /// neither run expiry at every tick nor stop it. Pure.
    pub fn next_due_ms(&self, interval_minutes: u32) -> Option<i64> {
        let minutes =
            interval_minutes.clamp(MIN_EXPIRY_INTERVAL_MINUTES, MAX_EXPIRY_INTERVAL_MINUTES);
        let interval_ms = i64::from(minutes) * MINUTE_MS;
        self.last_started_ms
            .map(|started| started.saturating_add(interval_ms))
    }

    /// Whether a run is due at `now_ms` under `interval_minutes`: at launch, or once
    /// [`Self::next_due_ms`] has been reached. Pure.
    pub fn is_due(&self, interval_minutes: u32, now_ms: i64) -> bool {
        self.next_due_ms(interval_minutes)
            .is_none_or(|due| now_ms >= due)
    }
}
