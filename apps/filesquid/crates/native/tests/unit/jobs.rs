//! Task DE-0.4: the SQLite job store in the host's operational database (API-S-033,
//! DE-S-014; NFR-003, ADR 007). Runs against SQLite files on disk.

use filesquid_api::jobs::{Job, JobError, JobState, JobStore};
use filesquid_native::{SqliteJobs, JOURNAL_FILE};
use std::path::{Path, PathBuf};

/// A fresh, empty directory for one test (filesystem I/O).
fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fs-native-jobs-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

/// A pending job `id` submitted at `created_ms`.
fn job(id: &str, created_ms: i64) -> Job {
    Job {
        id: id.into(),
        kind: "prov.backfill".into(),
        operation_id: "op-1".into(),
        source: "installation:a".into(),
        state: JobState::Pending,
        checkpoint: None,
        done: 0,
        total: None,
        attempts: 0,
        fence: 0,
        holder: None,
        lease_expires_ms: None,
        created_ms,
        updated_ms: created_ms,
        error_code: None,
    }
}

fn open(dir: &Path) -> SqliteJobs {
    SqliteJobs::open(&dir.join(JOURNAL_FILE)).expect("job store")
}

#[test]
fn a_job_and_its_checkpoint_survive_closing_the_store() {
    let dir = dir("reopen");
    let store = open(&dir);
    store.insert(&job("j1", 10)).unwrap();
    let mut running = job("j1", 10);
    running.state = JobState::Running;
    running.checkpoint = Some(r#"{"done":["a"]}"#.into());
    running.done = 1;
    running.total = Some(2);
    running.fence = 3;
    running.attempts = 2;
    running.holder = Some("launch-1".into());
    running.lease_expires_ms = Some(99);
    running.error_code = None;
    store
        .update("j1", &mut |_| Ok(running.clone()))
        .expect("updated");
    drop(store);
    let reopened = open(&dir);
    assert_eq!(reopened.get("j1").unwrap(), Some(running));
    assert_eq!(reopened.get("nope").unwrap(), None);
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn a_refused_update_stores_nothing_and_a_known_id_is_not_inserted_twice() {
    let dir = dir("refused");
    let store = open(&dir);
    store.insert(&job("j1", 10)).unwrap();
    assert_eq!(
        store.insert(&job("j1", 11)),
        Err(JobError::Conflict("job_exists"))
    );
    let refused = store.update("j1", &mut |_| Err(JobError::Fenced));
    assert_eq!(refused, Err(JobError::Fenced));
    assert_eq!(store.get("j1").unwrap(), Some(job("j1", 10)));
    assert_eq!(
        store.update("missing", &mut |j| Ok(j)),
        Err(JobError::NotFound)
    );
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn unfinished_jobs_are_listed_oldest_first_and_bounded() {
    let dir = dir("unfinished");
    let store = open(&dir);
    store.insert(&job("b", 20)).unwrap();
    store.insert(&job("a", 20)).unwrap();
    store.insert(&job("c", 5)).unwrap();
    let mut done = job("d", 1);
    done.state = JobState::Completed;
    store.insert(&done).unwrap();
    let ids: Vec<String> = store
        .unfinished(10)
        .unwrap()
        .into_iter()
        .map(|j| j.id)
        .collect();
    assert_eq!(ids, ["c", "a", "b"]);
    assert_eq!(store.unfinished(2).unwrap().len(), 2);
    std::fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn the_job_store_shares_the_owner_only_operational_database() {
    let dir = dir("owner");
    let _store = open(&dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.join(JOURNAL_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    std::fs::remove_dir_all(dir).expect("cleanup");
}
