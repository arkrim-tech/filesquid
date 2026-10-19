//! Health rules (SRS-F-061): derive `ok` / `degraded` / `failing` with reasons from the
//! process registry, host checks and open OCFL validation findings (API-S-022).
//! [`evaluate`] is pure; gathering inputs is done by [`crate::monitor::Monitor`].

use serde::Serialize;

/// One gibibyte.
pub const GIB: u64 = 1 << 30;
/// Free space below this is `degraded`.
pub const DEGRADED_FREE_BYTES: u64 = 5 * GIB;
/// Free space below this is `failing`.
pub const FAILING_FREE_BYTES: u64 = GIB;
/// Journal queue fill above this ratio is `degraded`.
pub const QUEUE_DEGRADED_FILL: f64 = 0.8;
/// Errors in the window at or above this are `degraded`.
pub const ERRORS_DEGRADED: u64 = 5;
/// Errors in the window at or above this are `failing`.
pub const ERRORS_FAILING: u64 = 50;
/// Error-rate window in milliseconds (5 minutes).
pub const ERROR_WINDOW_MS: i64 = 5 * 60 * 1000;

/// Overall health.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Everything within limits.
    Ok,
    /// Working, but something needs attention.
    Degraded,
    /// Not working correctly.
    Failing,
}

impl HealthStatus {
    /// Numeric gauge value: 0 ok, 1 degraded, 2 failing.
    pub fn gauge(self) -> u64 {
        self as u64
    }
}

/// One reason behind a non-`ok` status.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HealthReason {
    /// Machine-readable code, for example `process_stalled`.
    pub code: String,
    /// Severity this reason contributes.
    pub severity: HealthStatus,
    /// Short human-readable detail (no private data).
    pub detail: String,
    /// Machine-readable codes behind the reason, when it has any (for
    /// `ocfl_validation_errors`, the OCFL error codes of the open findings, sorted).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub codes: Vec<String>,
    /// How many objects the reason concerns, when it is about objects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objects: Option<usize>,
}

/// Facts the rules evaluate.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HealthInputs {
    /// Number of stalled processes.
    pub stalled: usize,
    /// The operational store answered a ping.
    pub store_ok: bool,
    /// The data directory accepted a test write.
    pub data_dir_writable: bool,
    /// Free bytes on the data directory's volume, when measurable.
    pub free_bytes: Option<u64>,
    /// Journal queue fill ratio.
    pub queue_fill: f64,
    /// Errors journaled in the last [`ERROR_WINDOW_MS`].
    pub errors_last_5m: u64,
    /// Distinct OCFL error codes of the open validation findings, sorted (API-S-022).
    pub ocfl_error_codes: Vec<String>,
    /// Objects with at least one open OCFL error finding.
    pub ocfl_error_objects: usize,
}

/// Health status, reasons and the inputs they came from.
#[derive(Clone, Debug, Serialize)]
pub struct HealthReport {
    /// Worst severity among the reasons, or `ok`.
    pub status: HealthStatus,
    /// Reasons in rule order.
    pub reasons: Vec<HealthReason>,
    /// The evaluated inputs.
    pub checks: HealthInputs,
}

/// Apply the health rules to `inputs`.
pub fn evaluate(inputs: &HealthInputs) -> HealthReport {
    let mut reasons = Vec::new();
    let mut add = |code: &str, severity, detail: String| {
        reasons.push(HealthReason {
            code: code.into(),
            severity,
            detail,
            codes: Vec::new(),
            objects: None,
        })
    };
    if inputs.stalled > 0 {
        add(
            "process_stalled",
            HealthStatus::Degraded,
            format!("{} process(es) stalled", inputs.stalled),
        );
    }
    match inputs.free_bytes {
        Some(free) if free < FAILING_FREE_BYTES => add(
            "critical_free_space",
            HealthStatus::Failing,
            format!("{free} bytes free"),
        ),
        Some(free) if free < DEGRADED_FREE_BYTES => add(
            "low_free_space",
            HealthStatus::Degraded,
            format!("{free} bytes free"),
        ),
        _ => {}
    }
    if inputs.queue_fill > QUEUE_DEGRADED_FILL {
        add(
            "journal_queue_high",
            HealthStatus::Degraded,
            format!("journal queue {:.0}% full", inputs.queue_fill * 100.0),
        );
    }
    if inputs.errors_last_5m >= ERRORS_FAILING {
        add(
            "error_rate",
            HealthStatus::Failing,
            format!("{} errors in 5 minutes", inputs.errors_last_5m),
        );
    } else if inputs.errors_last_5m >= ERRORS_DEGRADED {
        add(
            "error_rate",
            HealthStatus::Degraded,
            format!("{} errors in 5 minutes", inputs.errors_last_5m),
        );
    }
    if !inputs.store_ok {
        add(
            "store_unreachable",
            HealthStatus::Failing,
            "operational store did not answer".into(),
        );
    }
    if !inputs.data_dir_writable {
        add(
            "data_dir_not_writable",
            HealthStatus::Failing,
            "data directory is not writable".into(),
        );
    }
    if inputs.ocfl_error_objects > 0 {
        reasons.push(ocfl_reason(inputs));
    }
    let status = reasons
        .iter()
        .map(|r| r.severity)
        .max()
        .unwrap_or(HealthStatus::Ok);
    HealthReport {
        status,
        reasons,
        checks: inputs.clone(),
    }
}

/// The `ocfl_validation_errors` reason: at least `degraded` while any OCFL error finding is
/// open, with the codes and the number of affected objects (API-S-022). It names codes and a
/// count only, never an object, path or file.
fn ocfl_reason(inputs: &HealthInputs) -> HealthReason {
    HealthReason {
        code: "ocfl_validation_errors".into(),
        severity: HealthStatus::Degraded,
        detail: format!(
            "OCFL validation errors ({}) in {} object(s)",
            inputs.ocfl_error_codes.join(", "),
            inputs.ocfl_error_objects
        ),
        codes: inputs.ocfl_error_codes.clone(),
        objects: Some(inputs.ocfl_error_objects),
    }
}
