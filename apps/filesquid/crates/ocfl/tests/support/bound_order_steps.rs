//! Step definitions for the write-order scenario of
//! `version_bound_files_OCFL-S-024.feature` (IO-1.5; OCFL-S-018,
//! OCFL-S-024): a commit carrying a version-bound record follows the write
//! order on the in-memory backend and on `FsBackend`. Each scenario builds
//! its own rig on the named backend; the filesystem rig owns its scratch
//! directory, removed when the scenario ends.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use cucumber::{given, then, when};
use filesquid_ocfl::storage::{FsBackend, MemoryBackend, StorageBackend};
use filesquid_ocfl::write::BoundContext;

use crate::change::{commit_files, meta, model};
use crate::rig::{ready, Rig, OBJECT_ID};
use crate::{OcflWorld, Scratch};

/// The rig of one write-order scenario, on the backend it names.
enum OrderRig {
    Memory(Rig<MemoryBackend>),
    Fs {
        rig: Rig<FsBackend>,
        _scratch: Scratch,
    },
}

/// What the write-order scenarios carry between steps.
#[derive(Default)]
pub struct BoundOrderState {
    rig: Option<OrderRig>,
    /// The write log when the record function ran.
    at_call: Vec<String>,
    /// The write log of the whole commit.
    writes: Vec<String>,
}

impl fmt::Debug for BoundOrderState {
    /// Show the logs; the rig holds closures that are not `Debug`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BoundOrderState")
            .field("at_call", &self.at_call)
            .field("writes", &self.writes)
            .finish_non_exhaustive()
    }
}

/// Run `$body` with `$r` bound to the scenario's rig on its backend.
macro_rules! on_rig {
    ($world:expr, $r:ident => $body:expr) => {
        match $world.bound_order.rig.as_ref().expect("Given a backend") {
            OrderRig::Memory($r) => $body,
            OrderRig::Fs { rig: $r, .. } => $body,
        }
    };
}

/// A fresh `FsBackend` rig on its own scratch directory.
fn fs_rig() -> OrderRig {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("ocfl-bound-order-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let backend = FsBackend::new(dir.join("root"), dir.join("staging")).expect("fs backend");
    OrderRig::Fs {
        rig: Rig::new(backend),
        _scratch: Scratch(dir),
    }
}

/// Commit `v1` with `a.txt` on `rig`.
fn commit_v1<B: StorageBackend>(rig: &Rig<B>) {
    let m = model(rig, "writer-a");
    commit_files(&m, rig, &[("a.txt", b"alpha")], "v1").expect("commit v1");
}

/// Commit what `writer-a` staged with a record naming the version, and
/// return the write log when the record function ran and of the whole
/// commit.
fn commit_with_record<B: StorageBackend>(rig: &Rig<B>) -> (Vec<String>, Vec<String>) {
    rig.clear_log();
    let at_call = Mutex::new(Vec::new());
    let record = |context: &BoundContext<'_>| {
        at_call.lock().expect("lock").extend(rig.writes());
        let body = format!("{} {}", context.version, context.created);
        let path = format!("record/{}.txt", context.version);
        Ok(BTreeMap::from([(path, body.into_bytes())]))
    };
    let m = model(rig, "writer-a");
    ready(m.commit_with(meta(rig, "bound"), Some(&record))).expect("commit");
    let at_call = at_call.into_inner().expect("lock");
    (at_call, rig.writes())
}

/// The lease reservation's log line for the rig's object.
fn reservation<B: StorageBackend>(rig: &Rig<B>) -> String {
    let lease = rig.leases.lease_key(OBJECT_ID).expect("lease key");
    format!("leases put_if {lease}")
}

#[given(expr = "an object committed at \"v1\" through the change model on the {word} backend")]
fn object_on_backend(world: &mut OcflWorld, backend: String) {
    let rig = match backend.as_str() {
        "memory" => OrderRig::Memory(Rig::new(MemoryBackend::new())),
        "filesystem" => fs_rig(),
        other => panic!("unknown backend {other}"),
    };
    world.bound_order.rig = Some(rig);
    on_rig!(world, r => commit_v1(r));
}

#[given(expr = "a writer staged new bytes for {string} on that backend")]
fn staged_on_backend(world: &mut OcflWorld, path: String) {
    on_rig!(world, r => ready(model(r, "writer-a").stage_bytes(&path, b"bravo")).expect("stage"));
}

#[when("the writer commits the staged changes with a version-bound record on that backend")]
fn commit_on_backend(world: &mut OcflWorld) {
    let (at_call, writes) = on_rig!(world, r => commit_with_record(r));
    world.bound_order.at_call = at_call;
    world.bound_order.writes = writes;
}

#[then(
    expr = "the version should be reserved before the record function ran and before any {string} byte"
)]
fn reserved_first(world: &mut OcflWorld, version: String) {
    let reserve = on_rig!(world, r => reservation(r));
    let state = &world.bound_order;
    assert_eq!(
        state.at_call,
        std::slice::from_ref(&reserve),
        "only the reservation"
    );
    assert_eq!(state.writes.first(), Some(&reserve));
    assert_eq!(state.writes.last(), Some(&reserve), "released last");
    let first_byte = state
        .writes
        .iter()
        .position(|w| w.contains(&format!("obj/{version}/")))
        .expect("a version byte");
    assert!(first_byte > 0, "{:?}", state.writes);
}

#[then(
    expr = "{string} content should be written before its version inventory and the root inventory last"
)]
fn content_then_inventories(world: &mut OcflWorld, version: String) {
    let storage: Vec<&str> = world
        .bound_order
        .writes
        .iter()
        .filter_map(|w| w.strip_prefix("storage "))
        .filter_map(|w| w.split_once(' ').map(|(_, key)| key))
        .collect();
    let content = format!("obj/{version}/content/");
    let (written, published) = storage.split_at(2);
    let mut written = written.to_vec();
    written.sort_unstable();
    let expected = [
        format!("{content}b.txt"),
        format!("{content}record/{version}.txt"),
    ];
    assert_eq!(written, expected, "{storage:?}");
    let inventories = [
        format!("obj/{version}/inventory.json"),
        format!("obj/{version}/inventory.json.sha256"),
        "obj/inventory.json".to_string(),
        "obj/inventory.json.sha256".to_string(),
    ];
    assert_eq!(published, inventories, "{storage:?}");
}

#[then("the backend's object should validate with zero errors")]
fn backend_valid(world: &mut OcflWorld) {
    on_rig!(world, r => r.assert_valid());
}
