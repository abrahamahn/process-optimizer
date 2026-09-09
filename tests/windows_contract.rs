#![cfg(windows)]
use process_optimizer::{
    engine::{self, Backend},
    journal::{Database, Journal},
    model::*,
    windows::process::{self, WindowsBackend},
};
use std::os::windows::process::CommandExt;
use std::{
    path::Path,
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

struct Fixture(Child);
impl Fixture {
    fn start(window: bool) -> Self {
        let mut c = Command::new(env!("CARGO_BIN_EXE_optimizer-test-fixture"));
        if window {
            c.arg("--window");
        }
        Self(
            c.creation_flags(0x08000000)
                .spawn()
                .expect("spawn test-owned fixture"),
        )
    }
    fn identity(&self) -> Identity {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(id) = process::identity(self.0.id()) {
                return id;
            }
            assert!(Instant::now() < deadline, "fixture identity timeout");
            thread::sleep(Duration::from_millis(20));
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn session(target: Identity, action: ActionKind) -> Session {
    Session::new(
        format!("test-{}", target.pid),
        Plan {
            game_path: r"C:\TestOnly\NotAnActualGame.exe".into(),
            game: None,
            actions: vec![ApprovedAction { target, action }],
            protected_paths: vec![],
            options: Options {
                gpu_priority: false,
                cpu_priority: true,
                close_timeout_ms: 1000,
                ..Options::default()
            },
            consent: true,
            force_consent: action == ActionKind::ForceClose,
        },
    )
}

#[test]
fn priority_round_trip_on_our_own_fixture_only() {
    let fixture = Fixture::start(false);
    let id = fixture.identity();
    let mut backend = WindowsBackend::new(None, vec![], None).unwrap();
    backend.authorize(&id).unwrap();
    let original = backend.read(&id, Property::CpuPriority).unwrap();
    let mut s = session(id.clone(), ActionKind::LowerPriorities);
    let mut db = Database::open(Path::new(":memory:")).unwrap();
    db.create(&s).unwrap();
    engine::apply(&mut s, &mut backend, &mut db).unwrap();
    assert_eq!(
        backend.read(&id, Property::CpuPriority).unwrap(),
        original.background()
    );
    engine::restore(&mut s, &mut backend, &mut db).unwrap();
    assert_eq!(backend.read(&id, Property::CpuPriority).unwrap(), original);
    assert_eq!(s.stage, Stage::Restored);
}

#[test]
fn incorrect_creation_time_is_rejected_before_any_write() {
    let fixture = Fixture::start(false);
    let mut id = fixture.identity();
    id.created += 1;
    let mut backend = WindowsBackend::new(None, vec![], None).unwrap();
    assert_eq!(backend.authorize(&id).unwrap_err().kind, FaultKind::Gone);
}

#[test]
fn protected_application_cannot_be_selected_for_termination() {
    let fixture = Fixture::start(false);
    let id = fixture.identity();
    let mut backend = WindowsBackend::new(None, vec![id.path.clone()], None).unwrap();
    assert_eq!(backend.authorize(&id).unwrap_err().kind, FaultKind::Denied);
    assert!(process::alive(&id).unwrap());
}

#[test]
fn graceful_request_does_not_kill_windowless_fixture() {
    let fixture = Fixture::start(false);
    let id = fixture.identity();
    let mut backend = WindowsBackend::new(None, vec![], None).unwrap();
    assert_eq!(
        backend.close(&id, false, 1000).unwrap(),
        CloseState::KeptOpen
    );
    assert!(process::alive(&id).unwrap());
}

#[test]
fn explicitly_approved_force_terminates_only_owned_fixture() {
    let fixture = Fixture::start(false);
    let id = fixture.identity();
    let mut backend = WindowsBackend::new(None, vec![], None).unwrap();
    let mut s = session(id.clone(), ActionKind::ForceClose);
    let mut db = Database::open(Path::new(":memory:")).unwrap();
    db.create(&s).unwrap();
    engine::apply(&mut s, &mut backend, &mut db).unwrap();
    assert_eq!(s.closed[0].state, CloseState::Terminated);
    assert!(!process::alive(&id).unwrap());
    // Recovery cannot relaunch or terminate a replacement process.
    engine::restore(&mut s, &mut backend, &mut db).unwrap();
}

#[test]
fn wm_close_reaches_our_test_window() {
    let fixture = Fixture::start(true);
    let id = fixture.identity();
    let mut backend = WindowsBackend::new(None, vec![], None).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match backend.close(&id, false, 1000) {
            Ok(CloseState::ClosedGracefully) => break,
            _ => {
                assert!(
                    Instant::now() < deadline,
                    "test window did not accept WM_CLOSE"
                );
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

#[test]
fn gpu_scheduling_query_is_capability_gated() {
    let fixture = Fixture::start(false);
    let id = fixture.identity();
    let mut backend = WindowsBackend::new(None, vec![], None).unwrap();
    match backend.read(&id, Property::GpuPriority) {
        Ok(Value::GpuPriority(v)) => assert!((0..=5).contains(&v)),
        Err(e) => println!("GPU scheduling not available on this runner: {}", e.message),
        _ => panic!("wrong property type"),
    }
}

#[test]
fn sqlite_durable_record_can_be_reopened() {
    let path = std::env::temp_dir().join(format!(
        "optimizer-journal-test-{}-{}.sqlite3",
        std::process::id(),
        process_optimizer::windows::runner::fresh_id()
    ));
    let fixture = Fixture::start(false);
    let mut s = session(fixture.identity(), ActionKind::LowerPriorities);
    {
        let mut db = Database::open(&path).unwrap();
        db.create(&s).unwrap();
        s.stage = Stage::RecoveryNeeded;
        db.save(&s).unwrap();
    }
    {
        let db = Database::open(&path).unwrap();
        assert_eq!(db.active().unwrap().unwrap().stage, Stage::RecoveryNeeded);
    }
    let _ = std::fs::remove_file(&path);
}
