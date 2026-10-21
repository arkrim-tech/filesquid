//! Worker caps (API-S-036; CORE-S-006, ADR 007): a host may classify a job kind as a transfer
//! or as CPU-heavy work, and no more jobs of a class run at once than its cap allows.

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

    /// The class of `kind` and its cap, if the kind is classified. Pure.
    pub(super) fn class_of(&self, kind: &str) -> Option<(JobClass, usize)> {
        let class = self.classes.get(kind).copied()?;
        let cap = usize::try_from(self.limits.of(class)).unwrap_or(usize::MAX);
        Some((class, cap))
    }

    /// The kinds that share the class of `kind`, `kind` included; none for an unclassified
    /// kind. Pure.
    pub(super) fn kinds_sharing(&self, kind: &str) -> Vec<String> {
        let Some(class) = self.classes.get(kind).copied() else {
            return Vec::new();
        };
        self.classes
            .iter()
            .filter(|(_, of)| **of == class)
            .map(|(kind, _)| kind.clone())
            .collect()
    }
}
