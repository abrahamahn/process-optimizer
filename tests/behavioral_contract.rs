//! Deterministic contract tests. No Windows processes or real application data.
use process_optimizer::{
    engine::{self, Backend},
    gpu,
    journal::{Database, Journal},
    model::*,
    policy,
};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

fn identity(pid: u32) -> Identity {
    Identity {
        pid,
        created: 10,
        path: format!(r"C:\Fixture\{pid}.exe"),
        session_id: 1,
        provenance: Some(Provenance {
            owner_sid: "test-user".into(),
            logon_id: 10,
            image_file_id: format!("file-{pid}"),
        }),
    }
}
fn session() -> Session {
    Session::new(
        "contract".into(),
        Plan {
            manual_mode: false,
            game_path: r"C:\Game\game.exe".into(),
            game: Some(Identity {
                path: r"C:\Game\game.exe".into(),
                ..identity(99)
            }),
            actions: vec![ApprovedAction {
                reopen: None,
                target: identity(10),
                action: ActionKind::LowerPriorities,
            }],
            protected_paths: vec![],
            options: Options {
                gpu_priority: true,
                ..Options::default()
            },
            consent: true,
            force_consent: false,
            experimental_consent: true,
        },
    )
}
struct BackendFixture {
    cancel: Arc<AtomicBool>,
    writes: usize,
    closes: usize,
    reads: usize,
    gpu: i32,
    cpu: u32,
    blocked: Option<u32>,
    cancel_after_write: bool,
    drift_after_read: bool,
}
impl Backend for BackendFixture {
    fn authorize(&mut self, id: &Identity) -> NativeResult<()> {
        if self.cancel.load(Ordering::SeqCst) || self.blocked == Some(id.pid) {
            Err(Fault::new(FaultKind::Denied, "cancelled/protected"))
        } else {
            Ok(())
        }
    }
    fn read(&mut self, _: &Identity, property: Property) -> NativeResult<Value> {
        self.reads += 1;
        if self.drift_after_read && self.reads == 2 {
            self.gpu = 4;
        }
        Ok(match property {
            Property::GpuPriority => Value::GpuPriority(self.gpu),
            Property::CpuPriority => Value::CpuPriority(self.cpu),
            _ => return Err(Fault::new(FaultKind::Unsupported, "fixture property")),
        })
    }
    fn write(&mut self, _: &Identity, value: &Value) -> NativeResult<()> {
        self.writes += 1;
        match value {
            Value::GpuPriority(v) => self.gpu = *v,
            Value::CpuPriority(v) => self.cpu = *v,
            _ => return Err(Fault::new(FaultKind::Unsupported, "fixture property")),
        }
        if self.cancel_after_write {
            self.cancel.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
    fn close(&mut self, _: &Identity, _: bool, _: u32) -> NativeResult<CloseState> {
        self.closes += 1;
        Ok(CloseState::RequestedPending)
    }
}
struct StoreFixture {
    saves: usize,
    cancel_at: Option<usize>,
    cancel: Arc<AtomicBool>,
    saved: Option<Session>,
}
impl Journal for StoreFixture {
    fn save(&mut self, s: &Session) -> AppResult<()> {
        self.saves += 1;
        self.saved = Some(s.clone());
        if self.cancel_at == Some(self.saves) {
            self.cancel.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}
fn setup() -> (Session, BackendFixture, StoreFixture) {
    let cancel = Arc::new(AtomicBool::new(false));
    (
        session(),
        BackendFixture {
            cancel: cancel.clone(),
            writes: 0,
            closes: 0,
            reads: 0,
            gpu: 2,
            cpu: 0x20,
            blocked: None,
            cancel_after_write: false,
            drift_after_read: false,
        },
        StoreFixture {
            saves: 0,
            cancel_at: None,
            cancel,
            saved: None,
        },
    )
}
#[test]
fn first_run_has_no_experimental_or_launch_policy() {
    let o = Options::default();
    assert!(
        !o.gpu_priority && !o.launch_game && !o.cpu_priority && !o.eco_qos && !o.memory_priority
    );
}
#[test]
fn gpu_opt_in_is_independent_of_normal_approval() {
    let mut s = session();
    s.plan.experimental_consent = false;
    assert!(policy::validate(&s.plan).is_err());
    s.plan.options.gpu_priority = false;
    s.plan.options.cpu_priority = true;
    assert!(policy::validate(&s.plan).is_ok());
}
#[test]
fn protected_later_target_prevents_earlier_mutation() {
    let (mut s, mut b, mut j) = setup();
    s.plan.actions.push(ApprovedAction {
        reopen: None,
        target: identity(20),
        action: ActionKind::Close,
    });
    b.blocked = Some(20);
    assert!(engine::apply(&mut s, &mut b, &mut j).is_err());
    assert_eq!((b.writes, b.closes, j.saves), (0, 0, 0));
}
#[test]
fn cancellation_after_durable_intent_prevents_setting_call() {
    let (mut s, mut b, mut j) = setup();
    j.cancel_at = Some(2);
    assert!(engine::apply(&mut s, &mut b, &mut j).is_err());
    assert_eq!(b.writes, 0);
    assert_eq!(
        j.saved.as_ref().unwrap().changes[0].state,
        ChangeState::NotApplied
    );
    j.cancel_at = None;
    engine::restore(&mut s, &mut b, &mut j).unwrap();
    assert_eq!(s.stage, Stage::Restored);
}
#[test]
fn cancellation_between_properties_stops_the_next_property_but_allows_undo() {
    let (mut s, mut b, mut j) = setup();
    s.plan.options.cpu_priority = true;
    b.cancel_after_write = true;
    assert!(engine::apply(&mut s, &mut b, &mut j).is_err());
    assert_eq!((b.gpu, b.cpu, b.writes), (1, 0x20, 1));
    engine::restore(&mut s, &mut b, &mut j).unwrap();
    assert_eq!((b.gpu, b.cpu), (2, 0x20));
}
#[test]
fn cancellation_after_close_intent_never_calls_close() {
    let (mut s, mut b, mut j) = setup();
    s.plan.actions[0].action = ActionKind::Close;
    j.cancel_at = Some(2);
    assert!(engine::apply(&mut s, &mut b, &mut j).is_err());
    assert_eq!(b.closes, 0);
}
#[test]
fn setting_drift_between_snapshot_and_write_is_preserved() {
    let (mut s, mut b, mut j) = setup();
    b.drift_after_read = true;
    assert!(engine::apply(&mut s, &mut b, &mut j).is_err());
    assert_eq!((b.writes, b.gpu), (0, 4));
    engine::restore(&mut s, &mut b, &mut j).unwrap();
    assert_eq!(b.gpu, 4);
    assert_eq!(s.stage, Stage::Restored);
    assert_eq!(s.changes[0].state, ChangeState::NotApplied);
}
#[test]
fn apply_cannot_replay_an_existing_session() {
    let (mut s, mut b, mut j) = setup();
    engine::apply(&mut s, &mut b, &mut j).unwrap();
    let writes = b.writes;
    assert!(engine::apply(&mut s, &mut b, &mut j).is_err());
    assert_eq!(b.writes, writes);
}
#[test]
fn close_pending_is_not_a_falsely_completed_recovery() {
    let (mut s, mut b, mut j) = setup();
    s.plan.actions[0].action = ActionKind::Close;
    engine::apply(&mut s, &mut b, &mut j).unwrap();
    engine::restore(&mut s, &mut b, &mut j).unwrap();
    assert_eq!(s.stage, Stage::RecoveryNeeded);
    assert_eq!(b.closes, 1);
}
#[test]
fn ownership_logon_and_file_identity_are_part_of_process_lifetime() {
    let a = identity(10);
    let mut b = a.clone();
    b.provenance.as_mut().unwrap().logon_id += 1;
    assert!(!policy::same_process(&a, &b));
    let mut b = a.clone();
    b.provenance.as_mut().unwrap().image_file_id = "replacement".into();
    assert!(!policy::same_process(&a, &b));
    let mut b = a.clone();
    b.provenance.as_mut().unwrap().owner_sid = "another-user".into();
    assert!(!policy::same_process(&a, &b));
}
#[test]
fn oversized_and_unknown_requests_are_rejected() {
    let mut s = session();
    s.plan.actions = (10..43)
        .map(|p| ApprovedAction {
            reopen: None,
            target: identity(p),
            action: ActionKind::Close,
        })
        .collect();
    assert!(policy::validate(&s.plan).is_err());
    let mut json = serde_json::to_value(session().plan).unwrap();
    json["arbitrary_command"] = "not permitted".into();
    assert!(serde_json::from_value::<Plan>(json).is_err());
    let mut s = session();
    s.plan.protected_paths = vec!["x".repeat(256 * 1024)];
    assert!(policy::validate(&s.plan).is_err());
}
#[test]
fn game_launch_does_not_silently_substitute_for_attach() {
    let mut s = session();
    s.plan.options.launch_game = true;
    assert!(policy::validate(&s.plan).is_err());
}
#[test]
fn device_and_accessibility_software_is_conservatively_protected() {
    for n in [
        "G-Helper.exe",
        "GHelper.exe",
        "NVDA.exe",
        "ASUSOptimization.exe",
    ] {
        assert!(policy::reserved_name(n));
    }
}
#[test]
fn multi_adapter_memory_does_not_become_an_unlabeled_total() {
    let a = GpuReading {
        adapter: "a".into(),
        dedicated_bytes: Some(100),
        ..Default::default()
    };
    let b = GpuReading {
        adapter: "b".into(),
        dedicated_bytes: Some(200),
        ..Default::default()
    };
    assert_eq!(gpu::dedicated_allocations(&[a, b]), None);
}
#[test]
fn invalid_gpu_values_are_unknown_not_clamped() {
    assert_eq!(gpu::valid_percent(120.0), None);
    assert_eq!(gpu::valid_percent(f64::INFINITY), None);
}
#[test]
fn request_round_trip_preserves_independent_approvals() {
    let p = session().plan;
    let out: Plan = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert!(out.consent && out.experimental_consent && !out.force_consent);
}
#[test]
fn database_rejects_duplicate_and_unfinished_sessions_independently_of_ui() {
    let mut db = Database::open(Path::new(":memory:")).unwrap();
    let mut s = session();
    db.create(&s).unwrap();
    assert!(db.create(&s).is_err());
    let mut other = s.clone();
    other.id = "other".into();
    assert!(db.create(&other).is_err());
    s.stage = Stage::Restored;
    db.save(&s).unwrap();
    db.create(&other).unwrap();
}
