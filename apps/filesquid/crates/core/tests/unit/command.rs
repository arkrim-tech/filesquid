//! Unit tests for versioned command and query ports.

use filesquid_core::{
    CommandEnvelope, CommandOutcome, CommandOutcomeKind, DomainError, ExpectedRevision,
    OperationId, OperationScope, PageSize, PortError, QueryEnvelope, UiProgress, MAX_PAGE_SIZE,
    PROTOCOL_SCHEMA_VERSION,
};

fn op(id: &str) -> OperationId {
    OperationId::new(id).expect("operation")
}

#[test]
fn command_envelope_is_versioned_with_operation_revision_and_scope() {
    let envelope = CommandEnvelope::new(
        op("op-1"),
        ExpectedRevision::new("rev-3").expect("rev"),
        OperationScope::new("library:lib-1").expect("scope"),
    );
    assert_eq!(envelope.schema_version(), PROTOCOL_SCHEMA_VERSION);
    assert_eq!(envelope.operation_id().as_str(), "op-1");
    assert_eq!(envelope.expected_revision().as_str(), "rev-3");
    assert_eq!(envelope.scope().as_str(), "library:lib-1");
}

#[test]
fn empty_operation_id_is_rejected() {
    assert!(OperationId::new("").is_err());
}

#[test]
fn empty_revision_and_scope_are_rejected() {
    assert_eq!(
        ExpectedRevision::new("").unwrap_err(),
        DomainError::EmptyScope
    );
    assert_eq!(
        OperationScope::new("").unwrap_err(),
        DomainError::EmptyScope
    );
}

#[test]
fn page_size_rejects_zero_and_values_above_max() {
    assert_eq!(PageSize::new(0).unwrap_err(), DomainError::InvalidPageSize);
    assert_eq!(
        PageSize::new(MAX_PAGE_SIZE + 1).unwrap_err(),
        DomainError::InvalidPageSize
    );
    assert_eq!(PageSize::new(50).expect("page").get(), 50);
}

#[test]
fn page_size_bound_is_one_to_one_thousand() {
    assert_eq!(MAX_PAGE_SIZE, 1000);
    assert_eq!(PageSize::new(1).expect("min").get(), 1);
    assert_eq!(PageSize::new(1000).expect("max").get(), 1000);
    assert_eq!(
        PageSize::new(1001).unwrap_err(),
        DomainError::InvalidPageSize
    );
    assert_eq!(
        DomainError::InvalidPageSize.to_string(),
        "page size must be between 1 and 1000 inclusive"
    );
}

#[test]
fn query_envelope_carries_bounded_page_size() {
    let query = QueryEnvelope::new(
        PageSize::new(50).expect("page"),
        OperationScope::new("library:lib-1").expect("scope"),
    );
    assert_eq!(query.schema_version(), 1);
    assert_eq!(query.page_size().get(), 50);
}

#[test]
fn committed_outcome_is_durable_completion() {
    let outcome = CommandOutcome::new(op("op-1"), CommandOutcomeKind::Committed);
    assert!(outcome.is_durable_completion());
    assert_eq!(outcome.kind(), CommandOutcomeKind::Committed);
}

#[test]
fn pending_and_conflict_are_explicit_and_not_completion() {
    let pending = CommandOutcome::new(op("op-1"), CommandOutcomeKind::Pending);
    let conflict = CommandOutcome::new(op("op-1"), CommandOutcomeKind::Conflict);
    assert!(!pending.is_durable_completion());
    assert!(!conflict.is_durable_completion());
}

#[test]
fn port_error_records_retryability() {
    let err = PortError::new("busy", true).expect("error");
    assert_eq!(err.code(), "busy");
    assert!(err.retryable());
}

#[test]
fn ui_progress_is_not_durable_completion() {
    let progress = UiProgress::new(op("op-1"));
    assert!(!progress.is_durable_completion());
}

#[test]
fn deserialize_rejects_unbounded_page_size() {
    let parsed: Result<PageSize, _> = serde_json::from_str("0");
    assert!(parsed.is_err());
    let parsed: Result<PageSize, _> = serde_json::from_str("1001");
    assert!(parsed.is_err());
    let parsed: Result<PageSize, _> = serde_json::from_str("1000");
    assert_eq!(parsed.expect("1000 is within the bound").get(), 1000);
    let parsed: Result<ExpectedRevision, _> = serde_json::from_str("\"\"");
    assert!(parsed.is_err());
}

#[test]
fn error_outcome_carries_port_error_and_is_not_completion() {
    let err = PortError::new("busy", true).expect("error");
    let outcome = CommandOutcome::failed(op("op-1"), err);
    assert_eq!(outcome.kind(), CommandOutcomeKind::Error);
    assert!(!outcome.is_durable_completion());
    let port = outcome.port_error().expect("port");
    assert_eq!(port.code(), "busy");
    assert!(port.retryable());
}
