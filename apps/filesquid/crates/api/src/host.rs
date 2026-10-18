//! The host descriptor (API-S-020): which host the API runs on, who operates it, and the
//! state of encrypted backup and sync to the ciphertext-only hosted service
//! ([ADR 032](../../../../../resources/architecture/032-hosting-and-privacy-model.md)
//! Decisions 4, 6 and 7).
//!
//! - **Host type and operator** are supplied by the host when it starts, as a
//!   [`HostIdentity`], and never derived from a request. Only a server identity can carry an
//!   [`OperatorName`].
//! - **Hosted service** state comes from an injected [`EnrolmentPort`]. [`NotEnrolled`] is the
//!   port of a host with no enrolment state; enrolment (API-F4.1) brings a real one.
//! - **Change event:** [`HostInfo::refresh_hosted_service`] rereads the port and publishes one
//!   [`topic::HOST_CHANGE`] event carrying the whole new [`HostDescriptor`] when anything
//!   changed. The host type and operator name are fixed for the life of a process, so only the
//!   hosted-service state can change.
//! - **Trigger:** whoever changes the enrolment state calls
//!   [`HostInfo::refresh_hosted_service`] right after the change (a push, not a poll). Until
//!   enrolment exists (API-F4.1) every host uses [`NotEnrolled`], which never changes, so no
//!   host calls it yet; API-F4.1 calls it after every enrol, unenrol, backup switch and sync
//!   switch on both hosts.
//! - **Privacy:** the descriptor holds only the host type, the operator's display name, three
//!   flags and the service's DNS host name: no token, content handle, key or key identifier,
//!   account or device identifier, e-mail address, quota, object count, size, timestamp,
//!   logical or filesystem path or file name.
//!
//! Pure data and validation, plus one publish on the in-process event bus; no I/O.

use crate::events::{topic, ApiEvent, EventBus};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Version of the [`HostDescriptor`] schema.
pub const DESCRIPTOR_SCHEMA_VERSION: u32 = 1;

/// Most characters (Unicode scalar values) in an [`OperatorName`], after trimming.
pub const MAX_OPERATOR_NAME_CHARS: usize = 256;

/// Most characters in a DNS host name.
const MAX_HOST_NAME_CHARS: usize = 253;

/// Most characters in one label of a DNS host name.
const MAX_LABEL_CHARS: usize = 63;

/// The kind of host the API runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HostType {
    /// `filesquid-desktop`: the owner's own device.
    Desktop,
    /// `filesquid-server`: a server run by an operator, who can read what it stores.
    Server,
    /// The encrypted browser host.
    Browser,
}

/// The display name of a server's operator: trimmed, 1 to [`MAX_OPERATOR_NAME_CHARS`]
/// characters, no control characters. Build it with [`OperatorName::parse`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct OperatorName(String);

/// Why a configured operator name is refused. The message never contains the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperatorNameError {
    /// Longer than [`MAX_OPERATOR_NAME_CHARS`] characters after trimming.
    TooLong,
    /// Contains a control character.
    ControlCharacter,
}

impl std::fmt::Display for OperatorNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLong => write!(
                f,
                "the operator name is longer than {MAX_OPERATOR_NAME_CHARS} characters"
            ),
            Self::ControlCharacter => f.write_str("the operator name contains a control character"),
        }
    }
}

impl std::error::Error for OperatorNameError {}

impl OperatorName {
    /// Parse a configured operator name: surrounding whitespace is trimmed, and a blank value
    /// is `Ok(None)` (no operator name). Pure.
    ///
    /// # Errors
    /// [`OperatorNameError::TooLong`] or [`OperatorNameError::ControlCharacter`].
    pub fn parse(raw: &str) -> Result<Option<Self>, OperatorNameError> {
        let name = raw.trim();
        if name.is_empty() {
            return Ok(None);
        }
        if name.chars().count() > MAX_OPERATOR_NAME_CHARS {
            return Err(OperatorNameError::TooLong);
        }
        if name.chars().any(char::is_control) {
            return Err(OperatorNameError::ControlCharacter);
        }
        Ok(Some(Self(name.to_string())))
    }

    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a service host is refused. The message never contains the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceHostError {
    /// Not a bare DNS host name (a scheme, port, path, user information, query, IP address,
    /// empty label or other character).
    NotAHostName,
}

impl std::fmt::Display for ServiceHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "the hosted service must be named by a bare DNS host name (no scheme, port, path, user information or query)",
        )
    }
}

impl std::error::Error for ServiceHostError {}

/// The state of encrypted backup and sync to the ciphertext-only hosted service. Fields are
/// private so the invariants hold: not enrolled means both switches off and no host, and an
/// enrolled host is a lowercase bare DNS host name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HostedService {
    enrolled: bool,
    backup_enabled: bool,
    sync_enabled: bool,
    service_host: Option<String>,
}

impl HostedService {
    /// Not enrolled: backup and sync off, no service host.
    pub fn not_enrolled() -> Self {
        Self {
            enrolled: false,
            backup_enabled: false,
            sync_enabled: false,
            service_host: None,
        }
    }

    /// Enrolled with the service at `service_host` (lowercased), with backup and sync switched
    /// separately, because they are distinct guarantees.
    ///
    /// # Errors
    /// [`ServiceHostError::NotAHostName`] unless `service_host` is a bare DNS host name.
    pub fn enrolled_at(
        service_host: &str,
        backup_enabled: bool,
        sync_enabled: bool,
    ) -> Result<Self, ServiceHostError> {
        let host = service_host.to_ascii_lowercase();
        if !is_dns_host_name(&host) {
            return Err(ServiceHostError::NotAHostName);
        }
        Ok(Self {
            enrolled: true,
            backup_enabled,
            sync_enabled,
            service_host: Some(host),
        })
    }

    /// Whether the host is enrolled with the hosted service.
    pub fn enrolled(&self) -> bool {
        self.enrolled
    }

    /// Whether encrypted backup is on.
    pub fn backup_enabled(&self) -> bool {
        self.backup_enabled
    }

    /// Whether encrypted sync is on.
    pub fn sync_enabled(&self) -> bool {
        self.sync_enabled
    }

    /// The service's DNS host name, when enrolled.
    pub fn service_host(&self) -> Option<&str> {
        self.service_host.as_deref()
    }
}

/// True for a lowercase bare DNS host name: at most 253 characters of dot-separated labels,
/// each 1 to 63 of `a-z`, `0-9` and `-`, not starting or ending with `-`, and a last label
/// that is not all digits (so not an IPv4 address). Pure.
fn is_dns_host_name(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    let last_is_numeric = labels
        .last()
        .is_some_and(|l| l.bytes().all(|b| b.is_ascii_digit()));
    host.len() <= MAX_HOST_NAME_CHARS && !last_is_numeric && labels.iter().all(|l| is_label(l))
}

/// True for one DNS label: 1 to 63 of `a-z`, `0-9` and `-`, not starting or ending with `-`.
fn is_label(label: &str) -> bool {
    let allowed = label
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    (1..=MAX_LABEL_CHARS).contains(&label.len())
        && allowed
        && !label.starts_with('-')
        && !label.ends_with('-')
}

/// Where a host reads its hosted-service state. API-F4.1 implements it over the real
/// enrolment; until then every host uses [`NotEnrolled`]. Implementations MUST NOT do slow
/// I/O: the route and [`HostInfo::refresh_hosted_service`] call it inline. Whoever changes the
/// state behind a port MUST call [`HostInfo::refresh_hosted_service`] afterwards, so
/// subscribers receive the `host.change` event.
pub trait EnrolmentPort: Send + Sync + 'static {
    /// The current hosted-service state.
    fn hosted_service(&self) -> HostedService;
}

/// The enrolment port of a host with no enrolment state: always not enrolled.
#[derive(Clone, Copy, Debug, Default)]
pub struct NotEnrolled;

impl EnrolmentPort for NotEnrolled {
    fn hosted_service(&self) -> HostedService {
        HostedService::not_enrolled()
    }
}

/// What a host says about itself when it starts: its type and, for a server only, its
/// operator's name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostIdentity {
    host_type: HostType,
    operator_name: Option<OperatorName>,
}

impl HostIdentity {
    /// The desktop host: no operator name.
    pub fn desktop() -> Self {
        Self {
            host_type: HostType::Desktop,
            operator_name: None,
        }
    }

    /// The encrypted browser host: no operator name.
    pub fn browser() -> Self {
        Self {
            host_type: HostType::Browser,
            operator_name: None,
        }
    }

    /// A server, with its operator's name when one is configured.
    pub fn server(operator_name: Option<OperatorName>) -> Self {
        Self {
            host_type: HostType::Server,
            operator_name,
        }
    }

    /// The host type.
    pub fn host_type(&self) -> HostType {
        self.host_type
    }

    /// The operator's name (always `None` except on a named server).
    pub fn operator_name(&self) -> Option<&OperatorName> {
        self.operator_name.as_ref()
    }
}

/// The answer of `GET /api/v1/host` and the payload of a [`topic::HOST_CHANGE`] event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HostDescriptor {
    /// Always [`DESCRIPTOR_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The host type, supplied by the host.
    pub host_type: HostType,
    /// The server operator's name; `None` on desktop and browser hosts and on unnamed servers.
    pub operator_name: Option<OperatorName>,
    /// Encrypted backup and sync state.
    pub hosted_service: HostedService,
}

/// A described host: its identity, its enrolment port and the last hosted-service state it
/// published. Cheap to clone; clones share state.
#[derive(Clone)]
pub struct HostInfo {
    identity: Arc<HostIdentity>,
    enrolment: Arc<dyn EnrolmentPort>,
    published: Arc<Mutex<HostedService>>,
    events: EventBus,
}

impl std::fmt::Debug for HostInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostInfo")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl HostInfo {
    /// Describe a host as `identity`, reading hosted-service state from `enrolment` and
    /// publishing changes on `events`.
    pub fn new(
        identity: HostIdentity,
        enrolment: Arc<dyn EnrolmentPort>,
        events: EventBus,
    ) -> Self {
        let current = enrolment.hosted_service();
        Self {
            identity: Arc::new(identity),
            enrolment,
            published: Arc::new(Mutex::new(current)),
            events,
        }
    }

    /// The current descriptor, with the hosted-service state read from the port now.
    pub fn descriptor(&self) -> HostDescriptor {
        self.descriptor_with(self.enrolment.hosted_service())
    }

    /// Reread the enrolment port and, when the state changed since the last publish, publish
    /// one [`topic::HOST_CHANGE`] event with the whole new descriptor to every subscriber.
    /// Returns whether an event was published. The enrolment code calls it after every enrol,
    /// unenrol, backup switch or sync switch; concurrent calls publish in state order.
    pub fn refresh_hosted_service(&self) -> bool {
        let mut published = self.published.lock().unwrap_or_else(|p| p.into_inner());
        let current = self.enrolment.hosted_service();
        if *published == current {
            return false;
        }
        *published = current.clone();
        self.publish_change(self.descriptor_with(current));
        true
    }

    /// Publish `descriptor` as a [`topic::HOST_CHANGE`] event on the bus (never blocks).
    fn publish_change(&self, descriptor: HostDescriptor) {
        match serde_json::to_value(descriptor) {
            Ok(data) => {
                self.events.publish(ApiEvent::new(topic::HOST_CHANGE, data));
            }
            Err(err) => {
                tracing::error!(%err, operation = "host.change", "descriptor not encodable")
            }
        }
    }

    /// The descriptor of this host with `hosted_service`.
    fn descriptor_with(&self, hosted_service: HostedService) -> HostDescriptor {
        HostDescriptor {
            schema_version: DESCRIPTOR_SCHEMA_VERSION,
            host_type: self.identity.host_type,
            operator_name: self.identity.operator_name.clone(),
            hosted_service,
        }
    }
}
