//! [`LeaseStore`]: fenced per-object leases kept in any storage backend that
//! supports compare-and-swap (OCFL-S-018; ADR 007, ADR 012, ADR 016).
//!
//! Each object has one lease record, `<prefix>/<sha256 of the object
//! id>.json`, written only with [`put_if`](StorageBackend::put_if): `IfAbsent`
//! for the first reservation, `IfMatch` the record's entity tag for every
//! later change. Two writers can therefore never both take, renew or
//! release one lease. Records are never deleted, so the fencing token only
//! grows: each new holder gets the previous token plus one.
//!
//! Where the records live: a backend **outside** the storage root, shared
//! by every writer of that root. For a local storage root that is a
//! directory such as the writers' staging directory
//! ([`FsBackend`](crate::storage::FsBackend) over it); for an S3-compatible
//! root, another prefix or bucket with conditional writes.
//!
//! Fencing is checked, not enforced by the storage: a writer confirms its
//! lease before every write, and refuses to start one when no more than the
//! safety margin is left, so a writer would have to stall for longer than
//! the margin between the check and the write to write after its lease
//! expired. Choose the margin well above the longest single write (one part
//! of at most 64 MiB, or one inventory).

use std::fmt;

use serde_json::{json, Value};

use super::clock::Clock;
use super::error::ReservationError;
use super::reservation::{Reservation, VersionReservations};
use crate::digest::DigestAlgorithm;
use crate::inventory::VersionNum;
use crate::storage::{
    is_consistent, read_once, validate_prefix, ETag, Precondition, StorageBackend, StorageError,
    StorageKey, MAX_CHUNK_BYTES, READ_ATTEMPTS,
};

/// Largest lease record read: 4 KiB.
const MAX_RECORD_BYTES: u64 = 4 * 1024;

/// How long a lease lasts and how close to its end a writer stops writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaseConfig {
    /// Lease length in milliseconds. A confirm renews the lease to this
    /// length once less than half of it is left.
    pub ttl_ms: u64,
    /// Safety margin in milliseconds: a lease with no more than this left
    /// fails its fence check. MUST be less than half of `ttl_ms`.
    pub margin_ms: u64,
}

impl Default for LeaseConfig {
    /// 60 seconds, with a 10-second margin.
    fn default() -> Self {
        Self {
            ttl_ms: 60_000,
            margin_ms: 10_000,
        }
    }
}

/// Fenced per-object leases stored in `B`, timed by `C`.
pub struct LeaseStore<B, C> {
    backend: B,
    prefix: String,
    clock: C,
    config: LeaseConfig,
}

/// One lease record as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LeaseRecord {
    object_id: String,
    version: String,
    holder: String,
    token: u64,
    expires_ms: u64,
    released: bool,
}

impl<B: StorageBackend, C: Clock> LeaseStore<B, C> {
    /// Keep leases under the directory `prefix` of `backend`.
    ///
    /// # Errors
    ///
    /// [`ReservationError::InvalidConfig`] when `ttl_ms` is zero or the
    /// margin is not less than half of it;
    /// [`ReservationError::Storage`] for a prefix that could escape the
    /// backend root.
    pub fn new(
        backend: B,
        prefix: &str,
        clock: C,
        config: LeaseConfig,
    ) -> Result<Self, ReservationError> {
        validate_prefix(prefix)?;
        if config.ttl_ms == 0 || config.margin_ms.saturating_mul(2) >= config.ttl_ms {
            return Err(ReservationError::InvalidConfig {
                reason: "the lease TTL must be positive and more than twice the margin".to_string(),
            });
        }
        Ok(Self {
            backend,
            prefix: prefix.trim_end_matches('/').to_string(),
            clock,
            config,
        })
    }

    /// The backend that holds the lease records.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// The key of `object_id`'s lease record.
    ///
    /// # Errors
    ///
    /// [`StorageError::InvalidKey`] when the prefix and name make no key.
    pub fn lease_key(&self, object_id: &str) -> Result<StorageKey, StorageError> {
        let name = DigestAlgorithm::Sha256.digest_hex(object_id.as_bytes());
        if self.prefix.is_empty() {
            return StorageKey::new(&format!("{name}.json"));
        }
        StorageKey::new(&format!("{}/{name}.json", self.prefix))
    }

    /// Take the lease when it is free (absent, expired or released).
    async fn take(
        &self,
        object_id: &str,
        version: &VersionNum,
        holder: &str,
    ) -> Result<Reservation, ReservationError> {
        let key = self.lease_key(object_id)?;
        let now = self.now()?;
        let current = self.read_record(&key).await?;
        let (token, precondition) = match &current {
            None => (1, Precondition::IfAbsent),
            Some((record, _)) if record.is_live(now) => {
                return Err(held(object_id, &record.holder))
            }
            Some((record, tag)) => (
                next_token(record, &key)?,
                Precondition::IfMatch(tag.clone()),
            ),
        };
        let record = LeaseRecord {
            object_id: object_id.to_string(),
            version: version.to_string(),
            holder: holder.to_string(),
            token,
            expires_ms: now.saturating_add(self.config.ttl_ms),
            released: false,
        };
        self.write_record(&key, &record, &precondition)
            .await
            .map_err(|err| lost_race(err, object_id, "another writer"))?;
        log_take(current.as_ref().map(|(r, _)| r), &record);
        Ok(Reservation::new(object_id, version.clone(), holder, token))
    }

    /// Check that `reservation` still holds the lease with more than the
    /// margin left, renewing it when less than half the TTL is left.
    async fn check(&self, reservation: &Reservation) -> Result<(), ReservationError> {
        let key = self.lease_key(reservation.object_id())?;
        let now = self.now()?;
        let (mut record, tag) = self.owned_record(&key, reservation).await?;
        let left = record.expires_ms.saturating_sub(now);
        if left <= self.config.margin_ms {
            return Err(lost(reservation));
        }
        if left >= self.config.ttl_ms / 2 {
            return Ok(());
        }
        record.expires_ms = now.saturating_add(self.config.ttl_ms);
        self.write_record(&key, &record, &Precondition::IfMatch(tag))
            .await
            .map_err(|err| lost_or(err, reservation))
    }

    /// Mark the lease released, so the next reservation takes it at once.
    /// An expired lease is no longer this holder's to release.
    async fn give_up(&self, reservation: &Reservation) -> Result<(), ReservationError> {
        let key = self.lease_key(reservation.object_id())?;
        let now = self.now()?;
        let (mut record, tag) = self.owned_record(&key, reservation).await?;
        if !record.is_live(now) {
            return Err(lost(reservation));
        }
        record.released = true;
        self.write_record(&key, &record, &Precondition::IfMatch(tag))
            .await
            .map_err(|err| lost_or(err, reservation))?;
        tracing::debug!(
            object = %reservation.object_id(),
            version = %reservation.version(),
            token = reservation.token(),
            "released the version reservation"
        );
        Ok(())
    }

    /// The current time.
    fn now(&self) -> Result<u64, ReservationError> {
        self.clock
            .now_millis()
            .ok_or(ReservationError::ClockUnavailable)
    }

    /// The lease record, when `reservation` still owns it.
    async fn owned_record(
        &self,
        key: &StorageKey,
        reservation: &Reservation,
    ) -> Result<(LeaseRecord, ETag), ReservationError> {
        match self.read_record(key).await? {
            Some((record, tag)) if record.is_owned_by(reservation) => Ok((record, tag)),
            _ => Err(lost(reservation)),
        }
    }

    /// Read the record at `key` and its entity tag as one consistent
    /// version, or `None` when there is none.
    async fn read_record(
        &self,
        key: &StorageKey,
    ) -> Result<Option<(LeaseRecord, ETag)>, ReservationError> {
        for _ in 0..READ_ATTEMPTS {
            let before = match self.backend.head(key).await {
                Ok(meta) => meta,
                Err(StorageError::NotFound { .. }) => return Ok(None),
                Err(err) => return Err(err.into()),
            };
            if before.size > MAX_RECORD_BYTES {
                return Err(corrupt(key, "the record is larger than 4 KiB"));
            }
            let bytes = read_once(&self.backend, key, MAX_CHUNK_BYTES, before.size).await?;
            let after = self.backend.head(key).await?;
            if is_consistent(&self.backend, &before, &after, &bytes) {
                return LeaseRecord::parse(&bytes, key).map(|r| Some((r, before.etag)));
            }
        }
        Err(StorageError::Backend {
            reason: format!("{key} changed during each of {READ_ATTEMPTS} read attempts"),
        }
        .into())
    }

    /// Write `record` at `key` if `precondition` holds. A record larger
    /// than [`read_record`](Self::read_record) accepts (long object ids or
    /// holder names) is refused before it is written.
    async fn write_record(
        &self,
        key: &StorageKey,
        record: &LeaseRecord,
        precondition: &Precondition,
    ) -> Result<(), StorageError> {
        let bytes = record.to_json();
        if bytes.len() as u64 > MAX_RECORD_BYTES {
            return Err(StorageError::LimitExceeded {
                what: "lease record size",
                limit: MAX_RECORD_BYTES,
            });
        }
        self.backend
            .put_if(key, &bytes, precondition)
            .await
            .map(|_| ())
    }
}

impl<B: StorageBackend, C: Clock> VersionReservations for LeaseStore<B, C> {
    async fn reserve(
        &self,
        object_id: &str,
        version: &VersionNum,
        holder: &str,
    ) -> Result<Reservation, ReservationError> {
        self.take(object_id, version, holder).await
    }

    async fn confirm(&self, reservation: &Reservation) -> Result<(), ReservationError> {
        self.check(reservation).await
    }

    async fn release(&self, reservation: &Reservation) -> Result<(), ReservationError> {
        self.give_up(reservation).await
    }
}

impl<B, C> fmt::Debug for LeaseStore<B, C> {
    /// Show the store; backends and clocks need not implement `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LeaseStore")
            .field("prefix", &self.prefix)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl LeaseRecord {
    /// Whether the lease is held and unexpired at `now`.
    fn is_live(&self, now: u64) -> bool {
        !self.released && now < self.expires_ms
    }

    /// Whether this record is `reservation`'s lease, still held.
    fn is_owned_by(&self, reservation: &Reservation) -> bool {
        !self.released
            && self.token == reservation.token()
            && self.holder == reservation.holder()
            && self.object_id == reservation.object_id()
            && self.version == reservation.version().to_string()
    }

    /// The record as JSON.
    fn to_json(&self) -> Vec<u8> {
        let value = json!({
            "objectId": self.object_id,
            "version": self.version,
            "holder": self.holder,
            "token": self.token,
            "expires": self.expires_ms,
            "released": self.released,
        });
        value.to_string().into_bytes()
    }

    /// Read a record from the bytes stored at `key`.
    fn parse(bytes: &[u8], key: &StorageKey) -> Result<Self, ReservationError> {
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|err| corrupt(key, &format!("not JSON: {err}")))?;
        let text = |name: &str| value.get(name).and_then(Value::as_str).map(str::to_string);
        let number = |name: &str| value.get(name).and_then(Value::as_u64);
        let record = (|| {
            Some(Self {
                object_id: text("objectId")?,
                version: text("version")?,
                holder: text("holder")?,
                token: number("token")?,
                expires_ms: number("expires")?,
                released: value.get("released")?.as_bool()?,
            })
        })();
        record.ok_or_else(|| corrupt(key, "a field is missing or has the wrong type"))
    }
}

/// Log a taken lease; a takeover of a lease that was still held when it
/// expired means its writer crashed or stalled.
fn log_take(previous: Option<&LeaseRecord>, record: &LeaseRecord) {
    match previous {
        Some(old) if !old.released => tracing::warn!(
            object = %record.object_id,
            version = %record.version,
            previous_holder = %old.holder,
            token = record.token,
            "took over an expired version reservation"
        ),
        _ => tracing::debug!(
            object = %record.object_id,
            version = %record.version,
            token = record.token,
            "reserved a version"
        ),
    }
}

/// The token after `record`'s; a record at the largest token can have no
/// later holder and is reported rather than wrapped.
fn next_token(record: &LeaseRecord, key: &StorageKey) -> Result<u64, ReservationError> {
    record
        .token
        .checked_add(1)
        .ok_or_else(|| corrupt(key, "the fencing token cannot grow any further"))
}

/// The refusal for a lease that `holder` holds.
fn held(object_id: &str, holder: &str) -> ReservationError {
    ReservationError::Held {
        object_id: object_id.to_string(),
        holder: holder.to_string(),
    }
}

/// The refusal for a reservation that no longer holds its lease.
fn lost(reservation: &Reservation) -> ReservationError {
    ReservationError::Lost {
        object_id: reservation.object_id().to_string(),
        version: reservation.version().to_string(),
    }
}

/// A failed compare-and-swap while taking a lease means another writer took
/// it first.
fn lost_race(err: StorageError, object_id: &str, holder: &str) -> ReservationError {
    match err {
        StorageError::PreconditionFailed { .. } => held(object_id, holder),
        other => other.into(),
    }
}

/// A failed compare-and-swap on an owned lease means it changed hands.
fn lost_or(err: StorageError, reservation: &Reservation) -> ReservationError {
    match err {
        StorageError::PreconditionFailed { .. } => lost(reservation),
        other => other.into(),
    }
}

/// The error for an unreadable record at `key`.
fn corrupt(key: &StorageKey, reason: &str) -> ReservationError {
    ReservationError::Corrupt {
        key: key.as_str().to_string(),
        reason: reason.to_string(),
    }
}
