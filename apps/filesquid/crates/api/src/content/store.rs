//! The in-memory store of live content handles (API-S-018). It keeps each handle's SHA-256
//! digest, never the handle, and never writes anything to storage. Its only I/O is reading
//! the operating system's random source when it mints.

use super::encoding::{encode, well_formed};
use super::{ABSOLUTE_LIFETIME_MS, HANDLE_BYTES, IDLE_TIMEOUT_MS, MAX_HANDLES_PER_CREDENTIAL};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

/// A SHA-256 digest: of a handle, or of the credential that minted it.
type Digest32 = [u8; 32];

/// The clock content handles are timed by: Unix milliseconds (UTC).
pub type HandleClock = Arc<dyn Fn() -> i64 + Send + Sync>;

/// The one file version a handle names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentTarget {
    /// The library.
    pub library_id: String,
    /// The OCFL object.
    pub object_id: String,
    /// The OCFL version (`v1`, `v2`, ...).
    pub version: String,
    /// The logical path of the file in that version.
    pub path: String,
}

/// A freshly minted handle: the only place the handle text ever appears.
#[derive(Clone, PartialEq, Eq)]
pub struct Minted {
    /// The handle, 43 base64url characters. Answered once, to the minting caller.
    pub handle: String,
    /// The absolute expiry, Unix milliseconds (UTC).
    pub expires_ms: i64,
}

impl std::fmt::Debug for Minted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Minted")
            .field("handle", &"[redacted]")
            .field("expires_ms", &self.expires_ms)
            .finish()
    }
}

/// One live handle.
struct Entry {
    credential: Digest32,
    target: ContentTarget,
    minted_ms: i64,
    last_ms: i64,
    /// When it was last minted or accepted, in store order: the least recently used handle
    /// has the smallest value.
    used: u64,
}

impl Entry {
    /// True once the idle timeout or the absolute lifetime has passed at `now`, and also
    /// when the clock has moved back before the handle's last use: a handle fails closed
    /// rather than living longer than its bounds.
    fn expired(&self, now: i64) -> bool {
        now < self.last_ms
            || now.saturating_sub(self.last_ms) >= IDLE_TIMEOUT_MS
            || now.saturating_sub(self.minted_ms) >= ABSOLUTE_LIFETIME_MS
    }
}

/// The handles and the use counter, under one lock.
#[derive(Default)]
struct Handles {
    by_digest: HashMap<Digest32, Entry>,
    uses: u64,
}

impl Handles {
    /// The next use stamp.
    fn next_use(&mut self) -> u64 {
        self.uses += 1;
        self.uses
    }

    /// Drop every handle expired at `now`.
    fn purge_expired(&mut self, now: i64) {
        self.by_digest.retain(|_, entry| !entry.expired(now));
    }

    /// Drop the least recently used handle of `credential` while it holds the maximum.
    fn make_room(&mut self, credential: &Digest32) {
        let owned = self
            .by_digest
            .iter()
            .filter(|(_, e)| e.credential == *credential);
        if owned.clone().count() < MAX_HANDLES_PER_CREDENTIAL {
            return;
        }
        let oldest = owned.min_by_key(|(_, e)| e.used).map(|(digest, _)| *digest);
        if let Some(digest) = oldest {
            self.by_digest.remove(&digest);
        }
    }
}

/// The live content handles of one API instance. They live in memory only, so a process
/// restart (a desktop relaunch, a server restart) ends all of them. `Debug` shows only how
/// many are live.
pub struct ContentHandles {
    clock: HandleClock,
    handles: Mutex<Handles>,
}

impl std::fmt::Debug for ContentHandles {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentHandles")
            .field("live", &self.live())
            .finish()
    }
}

impl ContentHandles {
    /// An empty store timed by `clock`.
    pub fn new(clock: HandleClock) -> Self {
        Self {
            clock,
            handles: Mutex::new(Handles::default()),
        }
    }

    /// Now, by the store's clock.
    pub fn now_ms(&self) -> i64 {
        (self.clock)()
    }

    /// Mint a handle for `target` on behalf of the credential with SHA-256 digest
    /// `credential`. Expired handles are removed first, and the credential's least recently
    /// used handle is evicted when it already holds [`MAX_HANDLES_PER_CREDENTIAL`].
    ///
    /// # Errors
    ///
    /// A message when the operating system's random source fails; nothing is minted.
    pub fn mint(&self, credential: Digest32, target: ContentTarget) -> Result<Minted, String> {
        let mut bytes = [0u8; HANDLE_BYTES];
        getrandom::fill(&mut bytes).map_err(|e| format!("random source failed: {e}"))?;
        let handle = encode(&bytes);
        let now = self.now_ms();
        let mut handles = self.lock();
        handles.purge_expired(now);
        handles.make_room(&credential);
        let used = handles.next_use();
        let entry = Entry {
            credential,
            target,
            minted_ms: now,
            last_ms: now,
            used,
        };
        handles.by_digest.insert(digest(&handle), entry);
        Ok(Minted {
            handle,
            expires_ms: now.saturating_add(ABSOLUTE_LIFETIME_MS),
        })
    }

    /// Accept `handle` at the start of a request: it must be well formed, live and unexpired,
    /// and `still_allowed` must say that its minting credential (given by digest) may still
    /// read files. An accepted handle's idle timer restarts; an expired or no longer allowed
    /// one is removed. Answers the file version it names, or `None` for every unusable
    /// handle alike. `still_allowed` runs under the store's lock and MUST NOT do I/O.
    pub fn accept(
        &self,
        handle: &str,
        still_allowed: impl FnOnce(&Digest32) -> bool,
    ) -> Option<ContentTarget> {
        if !well_formed(handle) {
            return None;
        }
        let key = digest(handle);
        let now = self.now_ms();
        let mut handles = self.lock();
        let entry = handles.by_digest.get(&key)?;
        if entry.expired(now) || !still_allowed(&entry.credential) {
            handles.by_digest.remove(&key);
            return None;
        }
        let used = handles.next_use();
        let entry = handles.by_digest.get_mut(&key)?;
        entry.last_ms = now;
        entry.used = used;
        Some(entry.target.clone())
    }

    /// End every handle minted by the credential with SHA-256 digest `credential` (logout,
    /// session end or rotation). Answers how many ended.
    pub fn end_credential(&self, credential: &Digest32) -> usize {
        let mut handles = self.lock();
        let before = handles.by_digest.len();
        handles.by_digest.retain(|_, e| e.credential != *credential);
        before - handles.by_digest.len()
    }

    /// How many handles are held, expired ones not yet removed included.
    pub fn live(&self) -> usize {
        self.lock().by_digest.len()
    }

    /// The handles, recovering from a poisoned lock: every update leaves them consistent.
    fn lock(&self) -> MutexGuard<'_, Handles> {
        self.handles
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The SHA-256 digest of `text`.
fn digest(text: &str) -> Digest32 {
    Sha256::digest(text.as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};

    fn store() -> (ContentHandles, Arc<AtomicI64>) {
        let now = Arc::new(AtomicI64::new(1_000));
        let clock = now.clone();
        let store = ContentHandles::new(Arc::new(move || clock.load(Ordering::SeqCst)));
        (store, now)
    }

    fn target() -> ContentTarget {
        ContentTarget {
            library_id: "lib".into(),
            object_id: "obj".into(),
            version: "v1".into(),
            path: "a.mp4".into(),
        }
    }

    #[test]
    fn a_minted_handle_is_accepted_until_it_idles_out() {
        let (store, now) = store();
        let minted = store.mint([1; 32], target()).expect("mint");
        assert_eq!(minted.expires_ms, 1_000 + ABSOLUTE_LIFETIME_MS);
        assert_eq!(store.accept(&minted.handle, |_| true), Some(target()));
        now.fetch_add(IDLE_TIMEOUT_MS, Ordering::SeqCst);
        assert_eq!(store.accept(&minted.handle, |_| true), None);
        assert_eq!(store.live(), 0, "an expired handle is removed");
    }

    #[test]
    fn a_clock_that_moves_back_ends_the_handle() {
        let (store, now) = store();
        let minted = store.mint([1; 32], target()).expect("mint");
        now.fetch_sub(1, Ordering::SeqCst);
        assert_eq!(store.accept(&minted.handle, |_| true), None);
    }

    #[test]
    fn a_handle_no_longer_allowed_is_refused_and_removed() {
        let (store, _) = store();
        let minted = store.mint([1; 32], target()).expect("mint");
        assert_eq!(store.accept(&minted.handle, |c| *c == [2; 32]), None);
        assert_eq!(store.accept(&minted.handle, |_| true), None);
    }

    #[test]
    fn ending_a_credential_ends_only_its_handles() {
        let (store, _) = store();
        let mine = store.mint([1; 32], target()).expect("mint");
        let theirs = store.mint([2; 32], target()).expect("mint");
        assert_eq!(store.end_credential(&[1; 32]), 1);
        assert_eq!(store.accept(&mine.handle, |_| true), None);
        assert!(store.accept(&theirs.handle, |_| true).is_some());
    }

    #[test]
    fn the_store_keeps_digests_and_debug_shows_no_handle() {
        let (store, _) = store();
        let minted = store.mint([1; 32], target()).expect("mint");
        assert!(!format!("{store:?}").contains(&minted.handle));
        assert!(!format!("{minted:?}").contains(&minted.handle));
        let handles = store.lock();
        assert!(handles.by_digest.contains_key(&digest(&minted.handle)));
    }
}
