//! Worker caps (API-S-036; CORE-S-006, ADR 007): a host may classify a job kind as a transfer
//! or as CPU-heavy work, and no more jobs of a class run at once than its cap allows.

use super::model::Job;
use super::rules;
use std::collections::BTreeMap;

/// The class of work a job kind does, each with its own cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum JobClass {
    /// Moving bytes: uploads, downloads, imports, sync.
    Transfer,
    /// CPU-heavy work: media rendering, hashing at scale.
    CpuHeavy,
}

/// How many jobs of each class may run at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JobLimits {
    /// Running transfers at most.
    pub transfers: u32,
    /// Running CPU-heavy workers at most.
    pub cpu_heavy: u32,
}

impl Default for JobLimits {
    /// The defaults of CORE-S-006: transfers 4, CPU-heavy workers 2.
    fn default() -> Self {
        Self {
            transfers: filesquid_core::DEFAULT_ACTIVE_TRANSFERS,
            cpu_heavy: filesquid_core::DEFAULT_CPU_HEAVY_WORKERS,
        }
    }
}

impl JobLimits {
    /// The cap of `class`. Pure.
    pub fn of(&self, class: JobClass) -> u32 {
        match class {
            JobClass::Transfer => self.transfers,
            JobClass::CpuHeavy => self.cpu_heavy,
        }
    }
}

/// The classified kinds of a job service and their caps.
#[derive(Clone, Debug, Default)]
pub(super) struct Slots {
    classes: BTreeMap<String, JobClass>,
    limits: JobLimits,
}

impl Slots {
    /// Classify `kind` as `class`.
    pub(super) fn classify(&mut self, kind: &str, class: JobClass) {
        self.classes.insert(kind.to_string(), class);
    }

    /// Use `limits` as the caps.
    pub(super) fn limit(&mut self, limits: JobLimits) {
        self.limits = limits;
    }

    /// Whether job `id` of `kind` may start beside the `open` jobs at `now_ms`: true for an
    /// unclassified kind, else while fewer jobs of its class hold a live lease than its cap.
    /// Pure.
    pub(super) fn admits(&self, (id, kind): (&str, &str), open: &[Job], now_ms: i64) -> bool {
        let Some(class) = self.classes.get(kind).copied() else {
            return true;
        };
        let running = open
            .iter()
            .filter(|job| job.id != id && self.classes.get(&job.kind) == Some(&class))
            .filter(|job| rules::holds_slot(job, now_ms))
            .count();
        (running as u64) < u64::from(self.limits.of(class))
    }

    /// Whether any kind is classified.
    pub(super) fn any(&self) -> bool {
        !self.classes.is_empty()
    }
}
