//! Opt-in end-to-end tests on a disposable, non-administrator CI account.
//! Only owned fixtures are changed; each case uses an isolated local store.
#![cfg(windows)]
use process_optimizer::{
    engine::Backend,
    journal::Database,
    model::*,
    windows::process::{self, WindowsBackend},
};
use std::os::windows::process::CommandExt;
use std::{
    path::PathBuf,
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

struct OwnedChild(Child);

impl OwnedChild {
    fn exit(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }

    fn done(&mut self) -> bool {
        self.0.try_wait().expect("owned child wait").is_some()
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        self.exit();
    }
}

struct Harness {
    root: PathBuf,
    profile: PathBuf,
    app: PathBuf,
    fixture: PathBuf,
    game: PathBuf,
}

impl Harness {
    fn new() -> Self {
        assert_eq!(std::env::var("OPTIMIZER_ALLOW_WORKER_E2E").as_deref(), Ok("1"));
        assert!(
            !process::is_elevated().unwrap(),
            "Run unelevated, without a production safety bypass."
        );
        let binaries = PathBuf::from(
            std::env::var_os("OPTIMIZER_E2E_BIN_DIR").expect("staged fixture directory"),
        );
        let root = std::env::temp_dir().join(format!(
            "optimizer-e2e-{}",
            process_optimizer::windows::runner::fresh_id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let profile = root.join("profile");
        std::fs::create_dir(&profile).unwrap();
        let fixture = root.join("background-fixture.exe");
        let game = root.join("game-fixture.exe");
        std::fs::copy(binaries.join("optimizer-test-fixture.exe"), &fixture).unwrap();
        std::fs::copy(&fixture, &game).unwrap();
        Self {
            app: binaries.join("process-optimizer.exe"),
            root,
            profile,
            fixture,
            game,
        }
    }

    fn spawn(&self, game: bool) -> OwnedChild {
        OwnedChild(
            Command::new(if game { &self.game } else { &self.fixture })
                .creation_flags(0x08000000)
                .spawn()
                .unwrap(),
        )
    }

    fn db(&self) -> Database {
        let dir = self.profile.join("ProcessOptimizer");
        std::fs::create_dir_all(&dir).unwrap();
        Database::open(&dir.join("state.sqlite3")).unwrap()
    }

    fn worker(&self, id: Option<&str>) -> OwnedChild {
        let mut command = Command::new(&self.app);
        command
            .env("LOCALAPPDATA", &self.profile)
            .creation_flags(0x08000000);
        if let Some(id) = id {
            command.arg("--session").arg(id);
        } else {
            command.arg("--recover");
        }
        OwnedChild(command.spawn().unwrap())
    }

    fn plan(&self, game: &OwnedChild, background: &OwnedChild) -> Session {
        let game_id = process::identity(game.0.id()).unwrap();
        Session::new(
            process_optimizer::windows::runner::fresh_id(),
            Plan {
                game_path: game_id.path.clone(),
                game: Some(game_id),
                actions: vec![ApprovedAction {
                    target: process::identity(background.0.id()).unwrap(),
                    action: ActionKind::LowerPriorities,
                }],
                protected_paths: vec![],
                options: Options {
                    cpu_priority: true,
                    ..Options::default()
                },
                consent: true,
                force_consent: false,
                experimental_consent: false,
            },
        )
    }

    fn stage(&self, expected: Stage, worker: &mut OwnedChild) -> Session {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(s) = self.db().latest().unwrap() {
                if s.stage == expected {
                    return s;
                }
                assert!(
                    !worker.done(),
                    "worker exited at {:?}: {:?}",
                    s.stage,
                    s.events
                );
            }
            assert!(Instant::now() < deadline, "timeout waiting for {expected:?}");
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn stop(&self, id: &str) {
        self.db().request_stop(id).unwrap();
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn priority(id: &Identity) -> Value {
    WindowsBackend::new(None, vec![], None)
        .unwrap()
        .read(id, Property::CpuPriority)
        .unwrap()
}

#[test]
#[ignore = "requires an isolated, explicitly opted-in normal-user CI account"]
fn game_exit_automatically_restores_without_ui() {
    let h = Harness::new();
    let mut game = h.spawn(true);
    let background = h.spawn(false);
    let session = h.plan(&game, &background);
    let id = session.plan.actions[0].target.clone();
    let original = priority(&id);
    h.db().create(&session).unwrap();
    let mut worker = h.worker(Some(&session.id));
    h.stage(Stage::Active, &mut worker);
    assert_eq!(priority(&id), original.background());
    game.exit();
    let restored = h.stage(Stage::Restored, &mut worker);
    assert_eq!(priority(&id), original);
    assert_eq!(restored.changes[0].state, ChangeState::Restored);
}

#[test]
#[ignore = "requires an isolated, explicitly opted-in normal-user CI account"]
fn manual_restore_leaves_game_running() {
    let h = Harness::new();
    let game = h.spawn(true);
    let background = h.spawn(false);
    let session = h.plan(&game, &background);
    let id = session.plan.actions[0].target.clone();
    let original = priority(&id);
    h.db().create(&session).unwrap();
    let mut worker = h.worker(Some(&session.id));
    h.stage(Stage::Active, &mut worker);
    h.stop(&session.id);
    h.stage(Stage::Restored, &mut worker);
    assert!(process::alive(session.plan.game.as_ref().unwrap()).unwrap());
    assert_eq!(priority(&id), original);
}

#[test]
#[ignore = "requires an isolated, explicitly opted-in normal-user CI account"]
fn crashed_worker_recovers_from_disk_without_replaying_actions() {
    let h = Harness::new();
    let game = h.spawn(true);
    let background = h.spawn(false);
    let session = h.plan(&game, &background);
    let id = session.plan.actions[0].target.clone();
    let original = priority(&id);
    h.db().create(&session).unwrap();
    let mut worker = h.worker(Some(&session.id));
    h.stage(Stage::Active, &mut worker);
    worker.exit();
    assert_eq!(priority(&id), original.background());
    let mut recovery = h.worker(None);
    h.stage(Stage::Restored, &mut recovery);
    assert_eq!(priority(&id), original);
}

#[test]
#[ignore = "requires an isolated, explicitly opted-in normal-user CI account"]
fn durable_cancel_before_start_performs_no_mutation() {
    let h = Harness::new();
    let game = h.spawn(true);
    let background = h.spawn(false);
    let session = h.plan(&game, &background);
    let id = session.plan.actions[0].target.clone();
    let original = priority(&id);
    h.db().create(&session).unwrap();
    h.stop(&session.id);
    let mut worker = h.worker(Some(&session.id));
    let result = h.stage(Stage::Restored, &mut worker);
    assert!(result.changes.is_empty());
    assert_eq!(priority(&id), original);
}

#[test]
#[ignore = "requires an isolated, explicitly opted-in normal-user CI account"]
fn duplicate_controller_cannot_reapply_an_active_session() {
    let h = Harness::new();
    let game = h.spawn(true);
    let background = h.spawn(false);
    let session = h.plan(&game, &background);
    h.db().create(&session).unwrap();
    let mut worker = h.worker(Some(&session.id));
    h.stage(Stage::Active, &mut worker);
    let mut duplicate = h.worker(Some(&session.id));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !duplicate.done() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(50));
    }
    assert!(!duplicate.0.wait().unwrap().success());
    assert_eq!(h.db().get(&session.id).unwrap().changes.len(), 1);
    h.stop(&session.id);
    h.stage(Stage::Restored, &mut worker);
}

#[test]
#[ignore = "requires an isolated, explicitly opted-in normal-user CI account"]
fn restart_does_not_inherit_original_process_settings() {
    let h = Harness::new();
    let game = h.spawn(true);
    let mut background = h.spawn(false);
    let session = h.plan(&game, &background);
    h.db().create(&session).unwrap();
    let mut worker = h.worker(Some(&session.id));
    h.stage(Stage::Active, &mut worker);
    background.exit();
    let replacement = h.spawn(false);
    let id = process::identity(replacement.0.id()).unwrap();
    let before = priority(&id);
    h.stop(&session.id);
    let result = h.stage(Stage::Restored, &mut worker);
    assert_eq!(priority(&id), before);
    assert_eq!(result.changes[0].state, ChangeState::ProcessGone);
}
