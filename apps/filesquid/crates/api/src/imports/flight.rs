//! The operation IDs of the import plan requests in flight (API-S-036): the plan route claims
//! a request's operation ID before it asks the host's replay, and keeps the claim until the
//! host answered, so two requests of one operation ID never both find nothing to replay and
//! open two dialogs. The second is answered `dialog_open` while the first runs, and its own
//! replay once the first ended.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};

/// The operation IDs claimed by plan requests that have not answered yet. Holds only requests
/// in flight, so it is bounded by the requests the host serves at once.
#[derive(Debug, Default)]
pub(crate) struct InFlight {
    operations: Mutex<HashSet<String>>,
}

impl InFlight {
    /// No request in flight.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Claim `operation_id` for one request; `None` while another request holds it. The
    /// claim is released when the returned [`Claim`] is dropped, on every path.
    pub(crate) fn claim(self: &Arc<Self>, operation_id: &str) -> Option<Claim> {
        let fresh = self.lock().insert(operation_id.to_string());
        fresh.then(|| Claim {
            flights: Arc::clone(self),
            operation_id: operation_id.to_string(),
        })
    }

    /// The claimed operations, locked; a poisoned lock is recovered (plain data).
    fn lock(&self) -> MutexGuard<'_, HashSet<String>> {
        self.operations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// One request's claim of its operation ID, released on drop.
#[derive(Debug)]
pub(crate) struct Claim {
    flights: Arc<InFlight>,
    operation_id: String,
}

impl Drop for Claim {
    fn drop(&mut self) {
        self.flights.lock().remove(&self.operation_id);
    }
}
