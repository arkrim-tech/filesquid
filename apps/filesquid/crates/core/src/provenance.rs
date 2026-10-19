//! C2PA import records and PROV-O JSON-LD sidecar types.
//!
//! Digest algorithm is SHA-256. This crate does not call the `c2pa` SDK.

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

use crate::{DomainError, FileResourceId, JournalLink, OperationId, Sha256Digest, VersionRef};

/// The PROV namespace IRI the profile's `prov:` prefix maps to, as the
/// [PROV-O ontology](https://www.w3.org/ns/prov-o) declares it.
pub const PROV_O_CONTEXT: &str = "http://www.w3.org/ns/prov#";

/// Version of FileSquid's PROV-O JSON-LD profile: version 1 is
/// [`PROV_O_PROFILE`].
pub const PROV_O_PROFILE_VERSION: u32 = 1;

/// The name of FileSquid's PROV-O JSON-LD profile ([ADR 040](../../../../../resources/architecture/040-prov-o-history.md) Decision 4).
pub const PROV_O_PROFILE: &str = "filesquid-prov/1";

/// Digest algorithm used for import records and PROV-O linkage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DigestAlgorithm {
    /// SHA-256 (ADR 009 / ADR 015).
    #[default]
    Sha256,
}

impl DigestAlgorithm {
    /// Return the algorithm token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sha256 => "sha-256",
        }
    }
}

/// RFC 3339 UTC timestamp ending in `Z`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Rfc3339Utc(String);

impl Rfc3339Utc {
    /// Parse a UTC timestamp such as `2026-09-23T12:00:00Z`.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::InvalidTimestamp`] when `value` is not RFC 3339 UTC.
    pub fn parse(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if !is_rfc3339_utc(&value) {
            return Err(DomainError::InvalidTimestamp);
        }
        Ok(Self(value))
    }

    /// Borrow the timestamp.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Rfc3339Utc {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(de::Error::custom)
    }
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => {
            29
        }
        2 => 28,
        _ => 0,
    }
}

fn two_digit(part: &str) -> Option<u32> {
    (part.len() == 2).then(|| part.parse().ok()).flatten()
}

fn is_rfc3339_date(date: &str) -> bool {
    let mut parts = date.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    if year.len() != 4 {
        return false;
    }
    let Ok(year) = year.parse::<u32>() else {
        return false;
    };
    let Some(month) = two_digit(month) else {
        return false;
    };
    let Some(day) = two_digit(day) else {
        return false;
    };
    let max_day = days_in_month(year, month);
    max_day != 0 && (1..=max_day).contains(&day)
}

fn is_rfc3339_time(time: &str) -> bool {
    let (hms, frac) = match time.split_once('.') {
        Some((hms, frac)) => (hms, Some(frac)),
        None => (time, None),
    };
    if let Some(frac) = frac {
        if frac.is_empty() || !frac.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    let mut parts = hms.split(':');
    let (Some(hour), Some(minute), Some(second), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    let Some(hour) = two_digit(hour) else {
        return false;
    };
    let Some(minute) = two_digit(minute) else {
        return false;
    };
    let Some(second) = two_digit(second) else {
        return false;
    };
    hour <= 23 && minute <= 59 && second <= 60
}

fn is_rfc3339_utc(value: &str) -> bool {
    if !value.is_ascii() || !value.ends_with('Z') {
        return false;
    }
    let Some((date, time)) = value[..value.len() - 1].split_once('T') else {
        return false;
    };
    is_rfc3339_date(date) && is_rfc3339_time(time)
}

/// C2PA validation status. Unsigned is prepared, not a valid signed manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum C2paValidationStatus {
    /// Prepared claim/assertion data before signing.
    Unsigned,
    /// Signed with explicit validation.
    Signed,
    /// Failed or invalid binding.
    Invalid,
    /// Check unavailable (offline, missing ingredients).
    Unavailable,
}

impl C2paValidationStatus {
    /// Return whether this status is a valid signed C2PA manifest.
    pub fn is_valid_signed_manifest(self) -> bool {
        self == Self::Signed
    }

    /// Return the status token.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unsigned => "unsigned",
            Self::Signed => "signed",
            Self::Invalid => "invalid",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Resource-bound import-time C2PA record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct C2paImportRecord {
    resource_id: FileResourceId,
    digest: Sha256Digest,
    digest_algorithm: DigestAlgorithm,
    imported_at: Rfc3339Utc,
    status: C2paValidationStatus,
    invents_capture: bool,
    signed_manifest: bool,
}

impl C2paImportRecord {
    /// Initialize an unsigned import record. Does not invent a capture event.
    pub fn initialize(
        resource_id: FileResourceId,
        digest: Sha256Digest,
        imported_at: Rfc3339Utc,
    ) -> Self {
        Self {
            resource_id,
            digest,
            digest_algorithm: DigestAlgorithm::Sha256,
            imported_at,
            status: C2paValidationStatus::Unsigned,
            invents_capture: false,
            signed_manifest: false,
        }
    }

    /// Return the bound resource id.
    pub fn resource_id(&self) -> &FileResourceId {
        &self.resource_id
    }

    /// Return the bound digest.
    pub fn digest(&self) -> &Sha256Digest {
        &self.digest
    }

    /// Return the digest algorithm.
    pub fn digest_algorithm(&self) -> DigestAlgorithm {
        self.digest_algorithm
    }

    /// Return import time.
    pub fn imported_at(&self) -> &Rfc3339Utc {
        &self.imported_at
    }

    /// Return validation status.
    pub fn status(&self) -> C2paValidationStatus {
        self.status
    }

    /// Set validation status.
    ///
    /// Once the record has been signed, [`Self::mutate_signed_in_place`] keeps
    /// failing even if later observations are `invalid` or `unavailable`.
    pub fn set_status(&mut self, status: C2paValidationStatus) {
        if status == C2paValidationStatus::Signed {
            self.signed_manifest = true;
        }
        self.status = status;
    }

    /// Return whether this import invents an original capture event.
    pub fn invents_capture(&self) -> bool {
        self.invents_capture
    }

    /// Reject in-place mutation of a signed manifest.
    ///
    /// # Errors
    ///
    /// Returns [`DomainError::SignedManifestImmutable`] after a signed manifest
    /// has been recorded.
    pub fn mutate_signed_in_place(&self) -> Result<(), DomainError> {
        if self.signed_manifest {
            return Err(DomainError::SignedManifestImmutable);
        }
        Ok(())
    }
}

/// Append-only C2PA action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct C2paAction {
    pixel_change: bool,
}

impl C2paAction {
    /// Catalog-only organization. MUST NOT claim a pixel change.
    pub fn catalog_only() -> Self {
        Self {
            pixel_change: false,
        }
    }

    /// Return whether this action claims a pixel change.
    pub fn claims_pixel_change(&self) -> bool {
        self.pixel_change
    }
}

/// Versioned PROV-O JSON-LD sidecar profile: the FileSquid-facing summary
/// of one activity. It keeps the T-037 gate (an ephemeral cache is not
/// permanent history) and links the activity to the object version it
/// committed, its operation and its journal entries. The full PROV-O graph
/// is the `filesquid-prov` module's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvOSidecar {
    time: Rfc3339Utc,
    context: String,
    profile_version: u32,
    activity: String,
    ephemeral: bool,
    retained: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    version: Option<VersionRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    operation: Option<OperationId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    journal: Option<JournalLink>,
}

impl ProvOSidecar {
    /// Construct a retained PROV-O JSON-LD sidecar at `time`.
    pub fn retained(time: Rfc3339Utc) -> Self {
        Self {
            time,
            context: PROV_O_CONTEXT.to_string(),
            profile_version: PROV_O_PROFILE_VERSION,
            activity: "prov:Activity".to_string(),
            ephemeral: false,
            retained: true,
            version: None,
            operation: None,
            journal: None,
        }
    }

    /// Construct an ephemeral cache activity. Not permanent history.
    pub fn ephemeral_cache(time: Rfc3339Utc) -> Self {
        Self {
            time,
            context: PROV_O_CONTEXT.to_string(),
            profile_version: PROV_O_PROFILE_VERSION,
            activity: "prov:Activity".to_string(),
            ephemeral: true,
            retained: false,
            version: None,
            operation: None,
            journal: None,
        }
    }

    /// Return the activity time.
    pub fn time(&self) -> &Rfc3339Utc {
        &self.time
    }

    /// Return the pinned JSON-LD context.
    pub fn context(&self) -> &str {
        &self.context
    }

    /// Return the sidecar profile version.
    pub fn profile_version(&self) -> u32 {
        self.profile_version
    }

    /// Return the PROV-O activity type.
    pub fn activity(&self) -> &str {
        &self.activity
    }

    /// Return whether this sidecar uses the pinned JSON-LD PROV-O profile.
    pub fn is_json_ld(&self) -> bool {
        self.context == PROV_O_CONTEXT && self.profile_version == PROV_O_PROFILE_VERSION
    }

    /// Return whether this activity is permanent PROV-O history.
    pub fn is_permanent_history(&self) -> bool {
        !self.ephemeral || self.retained
    }

    /// The same sidecar naming the object version the activity committed.
    #[must_use]
    pub fn with_version(mut self, version: VersionRef) -> Self {
        self.version = Some(version);
        self
    }

    /// The same sidecar linking the activity's journal entries; the link's
    /// operation becomes the sidecar's operation.
    #[must_use]
    pub fn with_journal(mut self, journal: JournalLink) -> Self {
        self.operation = Some(journal.operation_id().clone());
        self.journal = Some(journal);
        self
    }

    /// The object version the activity committed, when linked.
    pub fn version(&self) -> Option<&VersionRef> {
        self.version.as_ref()
    }

    /// The operation the activity belongs to, when linked.
    pub fn operation(&self) -> Option<&OperationId> {
        self.operation.as_ref()
    }

    /// The journal entries of the activity's commit, when linked.
    pub fn journal(&self) -> Option<&JournalLink> {
        self.journal.as_ref()
    }
}
