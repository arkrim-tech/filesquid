//! The per-credential rate bound of the client-events route (API-S-030): at most
//! [`RATE_LIMIT`] requests in any rolling [`RATE_WINDOW_MS`]. In memory only; it ends with the
//! API state, which is what the bound needs (it only delays events).
//!
//! Credentials are tracked by the SHA-256 digest of their token, never the token. At most
//! [`MAX_TRACKED_CREDENTIALS`] are tracked: when a new one arrives and the table is full,
//! credentials with no request left in the window are forgotten first, then the one whose
//! last request is oldest.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use super::{RATE_LIMIT, RATE_WINDOW_MS};
use crate::content::store::HandleClock;

/// Most credentials tracked at once.
pub const MAX_TRACKED_CREDENTIALS: usize = 1_024;

/// The answer of [`RateLimiter::admit`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateDecision {
    /// The request is within the bound and was counted.
    Admitted,
    /// The request is over the bound and was not counted; retry after this many seconds.
    Limited {
        /// Whole seconds until the oldest counted request leaves the window (at least 1).
        retry_after_s: u64,
    },
}

/// Request times per credential digest.
pub struct RateLimiter {
    clock: HandleClock,
    windows: Mutex<HashMap<[u8; 32], VecDeque<i64>>>,
}

impl std::fmt::Debug for RateLimiter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tracked = self.windows.lock().map(|w| w.len()).unwrap_or(0);
        f.debug_struct("RateLimiter")
            .field("tracked", &tracked)
            .finish()
    }
}

impl RateLimiter {
    /// A limiter timed by `clock` (Unix milliseconds, UTC).
    pub fn new(clock: HandleClock) -> Self {
        Self {
            clock,
            windows: Mutex::new(HashMap::new()),
        }
    }

    /// The host time now (Unix milliseconds, UTC).
    pub fn now_ms(&self) -> i64 {
        (self.clock)()
    }

    /// Count one request of the credential whose token digest is `digest`, or refuse it
    /// when that credential already sent [`RATE_LIMIT`] requests in the window.
    pub fn admit(&self, digest: [u8; 32]) -> RateDecision {
        let now = self.now_ms();
        let mut windows = self.windows.lock().unwrap_or_else(|p| p.into_inner());
        if !windows.contains_key(&digest) {
            make_room(&mut windows, now);
        }
        let window = windows.entry(digest).or_default();
        while window.front().is_some_and(|t| now - t >= RATE_WINDOW_MS) {
            window.pop_front();
        }
        if window.len() < RATE_LIMIT {
            window.push_back(now);
            return RateDecision::Admitted;
        }
        let oldest = window.front().copied().unwrap_or(now);
        let wait_ms = u64::try_from(oldest + RATE_WINDOW_MS - now).unwrap_or(1);
        RateDecision::Limited {
            retry_after_s: wait_ms.div_ceil(1000).max(1),
        }
    }
}

/// Free one slot when `windows` is full: forget credentials without a request in the
/// window, else the one whose newest request is oldest. Bounded by the table size.
fn make_room(windows: &mut HashMap<[u8; 32], VecDeque<i64>>, now: i64) {
    if windows.len() < MAX_TRACKED_CREDENTIALS {
        return;
    }
    windows.retain(|_, w| w.back().is_some_and(|t| now - t < RATE_WINDOW_MS));
    if windows.len() < MAX_TRACKED_CREDENTIALS {
        return;
    }
    let stalest = windows
        .iter()
        .min_by_key(|(_, w)| w.back().copied().unwrap_or(i64::MIN))
        .map(|(k, _)| *k);
    if let Some(key) = stalest {
        windows.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::Arc;

    #[test]
    fn the_table_never_tracks_more_than_its_bound() {
        let now = Arc::new(AtomicI64::new(0));
        let clock = now.clone();
        let limiter = RateLimiter::new(Arc::new(move || clock.load(Ordering::SeqCst)));
        for n in 0..(MAX_TRACKED_CREDENTIALS + 10) {
            let mut digest = [0u8; 32];
            digest[..8].copy_from_slice(&(n as u64).to_le_bytes());
            assert_eq!(limiter.admit(digest), RateDecision::Admitted);
            now.fetch_add(1, Ordering::SeqCst);
        }
        let tracked = limiter.windows.lock().expect("lock").len();
        assert_eq!(tracked, MAX_TRACKED_CREDENTIALS);
    }

    #[test]
    fn retry_after_counts_whole_seconds_to_the_oldest_request_leaving() {
        let now = Arc::new(AtomicI64::new(10_000));
        let clock = now.clone();
        let limiter = RateLimiter::new(Arc::new(move || clock.load(Ordering::SeqCst)));
        for _ in 0..RATE_LIMIT {
            limiter.admit([1; 32]);
        }
        now.fetch_add(500, Ordering::SeqCst);
        assert_eq!(
            limiter.admit([1; 32]),
            RateDecision::Limited { retry_after_s: 60 }
        );
        now.fetch_add(59_500, Ordering::SeqCst);
        assert_eq!(limiter.admit([1; 32]), RateDecision::Admitted);
    }
}
