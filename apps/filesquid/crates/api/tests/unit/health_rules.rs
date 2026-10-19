//! Unit tests for health rules (K7, K8): every threshold has a test.

use filesquid_api::health::{evaluate, HealthInputs, HealthStatus, GIB};

fn healthy() -> HealthInputs {
    HealthInputs {
        stalled: 0,
        store_ok: true,
        data_dir_writable: true,
        free_bytes: Some(100 * GIB),
        queue_fill: 0.0,
        errors_last_5m: 0,
        ocfl_error_codes: Vec::new(),
        ocfl_error_objects: 0,
    }
}

fn status(i: HealthInputs) -> (HealthStatus, Vec<String>) {
    let r = evaluate(&i);
    (r.status, r.reasons.into_iter().map(|r| r.code).collect())
}

#[test]
fn all_clear_is_ok_without_reasons() {
    assert_eq!(status(healthy()), (HealthStatus::Ok, vec![]));
}

#[test]
fn stalled_processes_degrade() {
    assert_eq!(
        status(HealthInputs {
            stalled: 1,
            ..healthy()
        }),
        (HealthStatus::Degraded, vec!["process_stalled".into()])
    );
}

#[test]
fn free_space_thresholds() {
    assert_eq!(
        status(HealthInputs {
            free_bytes: Some(5 * GIB),
            ..healthy()
        })
        .0,
        HealthStatus::Ok
    );
    assert_eq!(
        status(HealthInputs {
            free_bytes: Some(5 * GIB - 1),
            ..healthy()
        }),
        (HealthStatus::Degraded, vec!["low_free_space".into()])
    );
    assert_eq!(
        status(HealthInputs {
            free_bytes: Some(GIB),
            ..healthy()
        }),
        (HealthStatus::Degraded, vec!["low_free_space".into()])
    );
    assert_eq!(
        status(HealthInputs {
            free_bytes: Some(GIB - 1),
            ..healthy()
        }),
        (HealthStatus::Failing, vec!["critical_free_space".into()])
    );
}

#[test]
fn queue_fill_threshold() {
    assert_eq!(
        status(HealthInputs {
            queue_fill: 0.8,
            ..healthy()
        })
        .0,
        HealthStatus::Ok
    );
    assert_eq!(
        status(HealthInputs {
            queue_fill: 0.81,
            ..healthy()
        }),
        (HealthStatus::Degraded, vec!["journal_queue_high".into()])
    );
}

#[test]
fn error_rate_thresholds() {
    assert_eq!(
        status(HealthInputs {
            errors_last_5m: 4,
            ..healthy()
        })
        .0,
        HealthStatus::Ok
    );
    assert_eq!(
        status(HealthInputs {
            errors_last_5m: 5,
            ..healthy()
        }),
        (HealthStatus::Degraded, vec!["error_rate".into()])
    );
    assert_eq!(
        status(HealthInputs {
            errors_last_5m: 50,
            ..healthy()
        }),
        (HealthStatus::Failing, vec!["error_rate".into()])
    );
}

#[test]
fn unreachable_store_or_read_only_data_dir_fail() {
    assert_eq!(
        status(HealthInputs {
            store_ok: false,
            ..healthy()
        }),
        (HealthStatus::Failing, vec!["store_unreachable".into()])
    );
    assert_eq!(
        status(HealthInputs {
            data_dir_writable: false,
            ..healthy()
        }),
        (HealthStatus::Failing, vec!["data_dir_not_writable".into()])
    );
}

#[test]
fn worst_condition_wins_and_all_reasons_are_listed() {
    let (s, reasons) = status(HealthInputs {
        stalled: 2,
        store_ok: false,
        ..healthy()
    });
    assert_eq!(s, HealthStatus::Failing);
    assert_eq!(reasons, ["process_stalled", "store_unreachable"]);
}

/// API-S-022: open OCFL error findings make health at least `degraded`, with the codes and
/// the number of affected objects; none leave it `ok`.
#[test]
fn open_ocfl_error_findings_degrade_health_with_codes_and_object_count() {
    let report = evaluate(&HealthInputs {
        ocfl_error_codes: vec!["E023".into(), "E092".into()],
        ocfl_error_objects: 2,
        ..healthy()
    });
    assert_eq!(report.status, HealthStatus::Degraded);
    let reason = &report.reasons[0];
    assert_eq!(reason.code, "ocfl_validation_errors");
    assert_eq!(reason.codes, ["E023", "E092"]);
    assert_eq!(reason.objects, Some(2));
    assert_eq!(evaluate(&healthy()).status, HealthStatus::Ok);
}
