//! Test rig for the `ocfl` store (MD-0.2): an OCFL storage root, a lease
//! store and a change-staging area, each on its own backend, so staging and
//! leases stay outside the storage root as `filesquid-ocfl` requires.
//! Objects are seeded through the IO-0.10 change model, never by hand.

#![allow(dead_code)]

use std::collections::BTreeMap;

use filesquid_metadata::store::ocfl::OcflStore;
use filesquid_ocfl::change::{ChangeModel, StagingArea, VersionMeta};
use filesquid_ocfl::read::Object;
use filesquid_ocfl::storage::{
    complete_immediately, read_verified, MemoryBackend, StorageBackend, StorageKey, MAX_CHUNK_BYTES,
};
use filesquid_ocfl::write::{rfc3339_utc, LeaseConfig, LeaseStore, SystemClock};
use filesquid_ocfl::VersionNum;

/// The object most tests describe.
pub const OBJECT_ID: &str = "urn:example:photo";

/// The lease store type the rig uses.
pub type Leases = LeaseStore<MemoryBackend, SystemClock>;

/// The `ocfl` store type the rig hands out.
pub type Store<'r, B> = OcflStore<'r, B, Leases, MemoryBackend, SystemClock, fn(&str) -> String>;

/// Most listing pages one snapshot reads (AGENTS.md §22).
const MAX_PAGES: usize = 100;

/// Drive a future that completes on its first poll (memory and filesystem backends).
pub fn ready<F: std::future::Future>(future: F) -> F::Output {
    complete_immediately(future).expect("backend futures complete at once")
}

/// The object root of `object_id`: `objects/<id with ':' as '_'>`.
pub fn object_root(object_id: &str) -> String {
    format!("objects/{}", object_id.replace(':', "_"))
}

/// A version number from its name.
pub fn v(name: &str) -> VersionNum {
    VersionNum::parse(name).expect("version name")
}

/// Storage root, leases and staging for one test.
pub struct Rig<B: StorageBackend> {
    /// The OCFL storage root.
    pub storage: B,
    /// Version reservations, outside the storage root.
    pub leases: Leases,
    /// Change staging, outside the storage root.
    pub staging: MemoryBackend,
}

impl Rig<MemoryBackend> {
    /// A rig whose storage root is in memory.
    pub fn memory() -> Self {
        Self::over(MemoryBackend::new())
    }
}

impl<B: StorageBackend> Rig<B> {
    /// A rig over `storage`.
    pub fn over(storage: B) -> Self {
        let leases = LeaseStore::new(
            MemoryBackend::new(),
            "leases",
            SystemClock,
            LeaseConfig::default(),
        )
        .expect("lease store");
        Self {
            storage,
            leases,
            staging: MemoryBackend::new(),
        }
    }

    /// The metadata store over this rig, writing as `metadata-test`.
    pub fn store(&self) -> Store<'_, B> {
        OcflStore {
            storage: &self.storage,
            reservations: &self.leases,
            staging: &self.staging,
            staging_prefix: "metadata-changes".to_string(),
            holder: "metadata-test".to_string(),
            clock: SystemClock,
            object_root: object_root as fn(&str) -> String,
        }
    }

    /// Commit `files` to `object_id` as one version through the change model.
    pub fn seed(&self, object_id: &str, files: &[(&str, &[u8])]) -> VersionNum {
        let staging = StagingArea::new(&self.staging, "seed").expect("seed prefix");
        let root = object_root(object_id);
        let model = ChangeModel::new(
            &self.storage,
            &self.leases,
            "seeder",
            &root,
            object_id,
            staging,
        );
        for (path, bytes) in files {
            ready(model.stage_bytes(path, bytes)).expect("stage");
        }
        let meta = VersionMeta::new(rfc3339_utc(1_791_590_400_000)).with_message("seed");
        ready(model.commit(meta)).expect("seed commit").version
    }

    /// The committed head of `object_id`.
    pub fn head(&self, object_id: &str) -> VersionNum {
        let object = ready(Object::open(&self.storage, &object_root(object_id))).expect("open");
        object.head().clone()
    }

    /// Every version of `object_id`, oldest first.
    pub fn versions(&self, object_id: &str) -> Vec<String> {
        let object = ready(Object::open(&self.storage, &object_root(object_id))).expect("open");
        object
            .history()
            .into_iter()
            .map(|info| info.version.to_string())
            .collect()
    }

    /// Every key under `prefix` with its bytes.
    pub fn snapshot(&self, prefix: &str) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut after: Option<StorageKey> = None;
        for _ in 0..MAX_PAGES {
            let page = ready(self.storage.list(prefix, after.as_ref(), 1_000)).expect("list");
            for key in &page.keys {
                let bytes =
                    ready(read_verified(&self.storage, key, MAX_CHUNK_BYTES)).expect("read");
                out.insert(key.as_str().to_string(), bytes);
            }
            match page.next {
                Some(next) => after = Some(next),
                None => return out,
            }
        }
        panic!("more than {MAX_PAGES} pages under {prefix}");
    }

    /// Delete `rel` inside the object root of `object_id`.
    pub fn delete(&self, object_id: &str, rel: &str) {
        let key = StorageKey::new(&format!("{}/{rel}", object_root(object_id))).expect("key");
        ready(self.storage.delete(&key)).expect("delete");
    }
}
