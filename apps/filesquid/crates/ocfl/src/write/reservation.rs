//! Version reservations: the fence every `vN` write passes through
//! (OCFL-S-018; ADR 007 exclusive leases; ADR 012 conflict guard; ADR 016
//! write step 2).
//!
//! A writer reserves `vN` of an object before it writes any `vN` byte, and
//! confirms the reservation before every later write. A reservation is
//! **fenced**: each new holder gets a larger token, and confirming checks
//! the token, so a writer whose reservation expired or passed to another
//! writer is refused at its next write. The shipped implementation is the
//! lease record of [`LeaseStore`](super::LeaseStore); a host that keeps a
//! PostgreSQL generation per object implements [`VersionReservations`]
//! over it instead.

use std::future::Future;

use super::error::ReservationError;
use crate::inventory::VersionNum;
use crate::storage::{MaybeSend, MaybeSync};

/// A held reservation of one version of one object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reservation {
    object_id: String,
    version: VersionNum,
    holder: String,
    token: u64,
}

impl Reservation {
    /// A reservation of `version` of `object_id` for `holder`, fenced by
    /// `token`. For [`VersionReservations`] implementations: the token MUST
    /// grow every time the object's reservation passes to a new holder.
    pub fn new(
        object_id: impl Into<String>,
        version: VersionNum,
        holder: impl Into<String>,
        token: u64,
    ) -> Self {
        Self {
            object_id: object_id.into(),
            version,
            holder: holder.into(),
            token,
        }
    }

    /// The object identifier.
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// The reserved version.
    pub fn version(&self) -> &VersionNum {
        &self.version
    }

    /// The writer that holds the reservation.
    pub fn holder(&self) -> &str {
        &self.holder
    }

    /// The fencing token: larger for every later holder of the object.
    pub fn token(&self) -> u64 {
        self.token
    }
}

/// Reserves versions of objects for one writer at a time.
///
/// The store MUST be shared by every writer of a storage root and MUST NOT
/// live inside the storage root (OCFL allows no extra files there). Its
/// records are engine working data, never needed to read or recover an
/// object.
pub trait VersionReservations: MaybeSend + MaybeSync {
    /// Reserve `version` of `object_id` for `holder`.
    ///
    /// # Errors
    ///
    /// [`ReservationError::Held`] when another reservation of the object is
    /// live, or a store error.
    fn reserve(
        &self,
        object_id: &str,
        version: &VersionNum,
        holder: &str,
    ) -> impl Future<Output = Result<Reservation, ReservationError>> + MaybeSend;

    /// The fence check: succeed only while `reservation` is still the
    /// object's live reservation, renewing it when the store needs that.
    ///
    /// # Errors
    ///
    /// [`ReservationError::Lost`] when it expired, was released or passed
    /// to another holder, or a store error.
    fn confirm(
        &self,
        reservation: &Reservation,
    ) -> impl Future<Output = Result<(), ReservationError>> + MaybeSend;

    /// Give `reservation` up, so another writer can reserve the object at
    /// once.
    ///
    /// # Errors
    ///
    /// [`ReservationError::Lost`] when it is no longer held, or a store
    /// error.
    fn release(
        &self,
        reservation: &Reservation,
    ) -> impl Future<Output = Result<(), ReservationError>> + MaybeSend;
}
