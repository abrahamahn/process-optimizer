#![cfg(windows)]
//! Only read our process or mutate fixtures created by this test.
use process_optimizer::{
    manual::StartupValueKind,
    windows::{process, startup, storage},
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct TempDirectory(PathBuf);
impl TempDirectory {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "optimizer-contract-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn live_identity_contains_user_logon_and_file_identifier() {
    let id = process::current_identity().unwrap();
    let provenance = id
        .provenance
        .expect("real native identity must have provenance");
    assert!(provenance.owner_sid.starts_with("S-1-"));
    assert_ne!(provenance.logon_id, 0);
    assert!(!provenance.image_file_id.is_empty());
    assert_eq!(
        process::image_file_id(&id.path).unwrap(),
        provenance.image_file_id
    );
}

#[test]
fn exclusive_file_lock_outlives_filename_and_releases_with_handle() {
    let temp = TempDirectory::new();
    let path = temp.0.join("controller.lock");
    let first = process::SessionLock::acquire_at(&path).unwrap();
    assert!(process::SessionLock::acquire_at(&path).is_err());
    drop(first);
    // The filename is still present. Only the live handle grants ownership.
    assert!(path.exists());
    let second = process::SessionLock::acquire_at(&path).unwrap();
    drop(second);
}

#[test]
fn owner_restricted_directory_remains_usable_by_its_owner() {
    let temp = TempDirectory::new();
    storage::secure_directory(&temp.0).unwrap();
    std::fs::write(temp.0.join("state.sqlite3"), b"test-owned placeholder").unwrap();
    storage::verify_store_entries(&temp.0).unwrap();
}

#[test]
fn storage_rejects_a_directory_disguised_as_database() {
    let temp = TempDirectory::new();
    std::fs::create_dir(temp.0.join("state.sqlite3")).unwrap();
    assert!(storage::verify_store_entries(&temp.0).is_err());
}

#[test]
fn current_user_run_entry_round_trips_without_touching_existing_startup() {
    let name = format!(
        "ProcessOptimizer-Test-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let exe = std::env::current_exe().unwrap();
    let command = format!("\"{}\" --startup-fixture", exe.display());

    // Create only this test-owned value using the same restoration API that the
    // product uses, then prove disable and restore are exact and reversible.
    startup::restore(&name, &command, &StartupValueKind::String).unwrap();
    let entry = startup::entries()
        .unwrap()
        .into_iter()
        .find(|e| e.value_name == name)
        .expect("test-owned startup value must be enumerable");
    assert_eq!(entry.command, command);

    startup::disable(&entry).unwrap();
    assert!(!startup::entries()
        .unwrap()
        .iter()
        .any(|e| e.value_name == name));

    startup::restore(&name, &command, &StartupValueKind::String).unwrap();
    let restored = startup::entries()
        .unwrap()
        .into_iter()
        .find(|e| e.value_name == name)
        .expect("restored test-owned startup value must exist");
    assert_eq!(restored.command, command);

    // Always leave the host as we found it except for our unique temporary key.
    startup::disable(&restored).unwrap();
}
