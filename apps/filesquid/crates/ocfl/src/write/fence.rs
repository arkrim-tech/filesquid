//! The fence check a commit runs before every write it makes under its
//! reservation (OCFL-S-018; ADR 007: an expired worker MUST NOT commit after
//! a replacement owns the lease).

use super::error::CommitError;
use super::reservation::{Reservation, VersionReservations};

/// One commit's reservation and the store that confirms it.
pub(crate) struct Fence<'a, R> {
    reservations: &'a R,
    reservation: &'a Reservation,
}

impl<'a, R: VersionReservations> Fence<'a, R> {
    /// The fence for `reservation`, held in `reservations`.
    pub fn new(reservations: &'a R, reservation: &'a Reservation) -> Self {
        Self {
            reservations,
            reservation,
        }
    }

    /// Confirm the reservation is still held before the next write.
    ///
    /// # Errors
    ///
    /// [`CommitError::Reservation`] with
    /// [`ReservationError::Lost`](super::ReservationError::Lost) once it
    /// expired or passed to another writer, or a store error.
    pub async fn check(&self) -> Result<(), CommitError> {
        self.reservations
            .confirm(self.reservation)
            .await
            .map_err(CommitError::from)
    }
}
