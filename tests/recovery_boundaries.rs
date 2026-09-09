//! Deliberate mutation/journal interruption tests. All effects are in memory.
use process_optimizer::{
    engine::{self, Backend},
    integrity,
    journal::{Database, Journal},
    model::*,
};
use std::{cell::Cell, path::Path, rc::Rc};

fn id(pid: u32) -> Identity {
    Identity {
        pid,
        created: 100,
        path: format!(r"C:\Fixture\{pid}.exe"),
        session_id: 1,
        provenance: Some(Provenance {
            owner_sid: "test-user".into(),
            logon_id: 10,
            image_file_id: format!("file-{pid}"),
        }),
    }
}
fn session(action: ActionKind) -> Session {
    let game = id(99);
    Session::new(
        "recovery-boundaries".into(),
        Plan {
            manual_mode: false,
            game_path: game.path.clone(),
            game: Some(game),
            actions: vec![ApprovedAction {
                reopen: None,
                target: id(20),
                action,
            }],
            protected_paths: vec![],
            options: Options {
                gpu_priority: action == ActionKind::LowerPriorities,
                ..Options::default()
            },
            consent: true,
            force_consent: action == ActionKind::ForceClose,
            experimental_consent: true,
        },
    )
}
struct BackendFixture {
    current: Value,
    writes: usize,
    closes: usize,
    reads: usize,
    cancel: Rc<Cell<bool>>,
    drift_read: Option<(usize, Value)>,
    error_after_write: bool,
    close_error: bool,
}
impl Backend for BackendFixture {
    fn authorize(&mut self, _: &Identity) -> NativeResult<()> {
        if self.cancel.get() {
            Err(Fault::new(FaultKind::Denied, "cancelled"))
        } else {
            Ok(())
        }
    }
    fn read(&mut self, _: &Identity, _: Property) -> NativeResult<Value> {
        self.reads += 1;
        if let Some((index, value)) = &self.drift_read {
            if *index == self.reads {
                self.current = value.clone();
            }
        }
        Ok(self.current.clone())
    }
    fn write(&mut self, _: &Identity, value: &Value) -> NativeResult<()> {
        self.writes += 1;
        self.current = value.clone();
        if self.error_after_write {
            Err(Fault::new(
                FaultKind::Other,
                "effect occurred but result failed",
            ))
        } else {
            Ok(())
        }
    }
    fn close(&mut self, _: &Identity, force: bool, _: u32) -> NativeResult<CloseState> {
        self.closes += 1;
        if self.close_error {
            Err(Fault::new(FaultKind::Other, "unconfirmed close"))
        } else {
            Ok(if force {
                CloseState::Terminated
            } else {
                CloseState::KeptOpen
            })
        }
    }
}
struct Store {
    calls: usize,
    fail: Option<usize>,
    cancel_at: Option<usize>,
    cancel: Rc<Cell<bool>>,
    saved: Option<Session>,
}
impl Journal for Store {
    fn save(&mut self, s: &Session) -> AppResult<()> {
        self.calls += 1;
        if self.fail == Some(self.calls) {
            return Err("injected journal failure".into());
        }
        self.saved = Some(s.clone());
        if self.cancel_at == Some(self.calls) {
            self.cancel.set(true);
        }
        Ok(())
    }
}
fn setup() -> (BackendFixture, Store) {
    let cancel = Rc::new(Cell::new(false));
    (
        BackendFixture {
            current: Value::GpuPriority(2),
            writes: 0,
            closes: 0,
            reads: 0,
            cancel: cancel.clone(),
            drift_read: None,
            error_after_write: false,
            close_error: false,
        },
        Store {
            calls: 0,
            fail: None,
            cancel_at: None,
            cancel,
            saved: None,
        },
    )
}

#[test]
fn never_applied_intent_does_not_take_ownership_of_a_later_matching_value() {
    let mut s = session(ActionKind::LowerPriorities);
    let (mut b, mut j) = setup();
    j.cancel_at = Some(2);
    assert!(engine::apply(&mut s, &mut b, &mut j).is_err());
    assert_eq!(b.writes, 0);
    assert_eq!(s.changes[0].state, ChangeState::NotApplied);
    // Another application independently selected exactly the value we had intended.
    b.current = Value::GpuPriority(1);
    engine::restore(&mut s, &mut b, &mut j).unwrap();
    assert_eq!(b.writes, 0);
    assert_eq!(b.current, Value::GpuPriority(1));
}
#[test]
fn drift_to_the_desired_value_before_setter_is_still_not_our_change() {
    let mut s = session(ActionKind::LowerPriorities);
    let (mut b, mut j) = setup();
    b.drift_read = Some((2, Value::GpuPriority(1)));
    assert!(engine::apply(&mut s, &mut b, &mut j).is_err());
    engine::restore(&mut s, &mut b, &mut j).unwrap();
    assert_eq!((b.writes, b.current), (0, Value::GpuPriority(1)));
}
#[test]
fn cancelled_unsent_close_is_not_an_unconfirmed_shutdown() {
    let mut s = session(ActionKind::Close);
    let (mut b, mut j) = setup();
    j.cancel_at = Some(2);
    assert!(engine::apply(&mut s, &mut b, &mut j).is_err());
    assert_eq!(s.closed[0].state, CloseState::NotRequested);
    engine::restore(&mut s, &mut b, &mut j).unwrap();
    assert_eq!((b.closes, s.stage), (0, Stage::Restored));
}
#[test]
fn every_apply_persistence_boundary_recovers_only_durable_intent() {
    for boundary in 1..=4 {
        let mut s = session(ActionKind::LowerPriorities);
        let (mut b, mut j) = setup();
        j.fail = Some(boundary);
        assert!(engine::apply(&mut s, &mut b, &mut j).is_err());
        j.fail = None;
        if let Some(mut durable) = j.saved.clone() {
            engine::restore(&mut durable, &mut b, &mut j).unwrap();
        }
        assert_eq!(b.current, Value::GpuPriority(2), "boundary {boundary}");
    }
}
#[test]
fn every_restore_persistence_boundary_avoids_a_duplicate_native_write() {
    for boundary in 1..=4 {
        let mut s = session(ActionKind::LowerPriorities);
        let (mut b, mut j) = setup();
        engine::apply(&mut s, &mut b, &mut j).unwrap();
        j.calls = 0;
        j.fail = Some(boundary);
        assert!(engine::restore(&mut s, &mut b, &mut j).is_err());
        j.fail = None;
        let mut durable = j.saved.clone().unwrap();
        engine::restore(&mut durable, &mut b, &mut j).unwrap();
        assert_eq!(b.current, Value::GpuPriority(2));
        assert_eq!(b.writes, 2, "boundary {boundary}");
    }
}
#[test]
fn competing_writer_after_restore_intent_is_not_overwritten() {
    let mut s = session(ActionKind::LowerPriorities);
    let (mut b, mut j) = setup();
    engine::apply(&mut s, &mut b, &mut j).unwrap();
    b.drift_read = Some((b.reads + 2, Value::GpuPriority(3)));
    engine::restore(&mut s, &mut b, &mut j).unwrap();
    assert_eq!(b.writes, 1);
    assert_eq!(b.current, Value::GpuPriority(3));
    assert_eq!(s.changes[0].state, ChangeState::Conflict);
}
#[test]
fn once_conflicted_matching_our_old_value_does_not_restore_ownership() {
    let mut s = session(ActionKind::LowerPriorities);
    let (mut b, mut j) = setup();
    engine::apply(&mut s, &mut b, &mut j).unwrap();
    b.current = Value::GpuPriority(3);
    engine::restore(&mut s, &mut b, &mut j).unwrap();
    b.current = Value::GpuPriority(1);
    engine::restore(&mut s, &mut b, &mut j).unwrap();
    assert_eq!(b.writes, 1);
    assert_eq!(s.stage, Stage::RecoveryNeeded);
    b.current = Value::GpuPriority(2);
    engine::restore(&mut s, &mut b, &mut j).unwrap();
    assert_eq!(s.stage, Stage::Restored);
    assert_eq!(b.writes, 1);
}
#[test]
fn ambiguous_setter_stops_later_actions_and_keeps_original_for_recovery() {
    let mut s = session(ActionKind::LowerPriorities);
    let (mut b, mut j) = setup();
    s.plan.actions.push(ApprovedAction {
        reopen: None,
        target: id(21),
        action: ActionKind::Close,
    });
    b.error_after_write = true;
    assert!(engine::apply(&mut s, &mut b, &mut j).is_err());
    assert_eq!(b.closes, 0);
    b.error_after_write = false;
    engine::restore(&mut s, &mut b, &mut j).unwrap();
    assert_eq!(b.current, Value::GpuPriority(2));
}
#[test]
fn failed_close_is_not_reported_as_completed_or_replayed() {
    let mut s = session(ActionKind::ForceClose);
    let (mut b, mut j) = setup();
    b.close_error = true;
    assert!(engine::apply(&mut s, &mut b, &mut j).is_err());
    engine::restore(&mut s, &mut b, &mut j).unwrap();
    assert_eq!(b.closes, 1);
    assert_eq!(s.stage, Stage::RecoveryNeeded);
}
#[test]
fn invalid_getter_value_does_not_reach_native_setter() {
    for value in [
        Value::GpuPriority(-1),
        Value::CpuPriority(123),
        Value::MemoryPriority(0),
        Value::EcoQos {
            control: 128,
            state: 128,
        },
    ] {
        let mut s = session(ActionKind::LowerPriorities);
        let (mut b, mut j) = setup();
        b.current = value;
        assert!(engine::apply(&mut s, &mut b, &mut j).is_err());
        assert_eq!(b.writes, 0);
    }
}
#[test]
fn corrupt_out_of_plan_recovery_never_reads_or_writes_native_targets() {
    let mut s = session(ActionKind::LowerPriorities);
    s.stage = Stage::Active;
    s.changes.push(Change {
        target: id(21),
        before: Value::GpuPriority(2),
        applied: Value::GpuPriority(1),
        state: ChangeState::Applied,
        detail: "invalid".into(),
    });
    let (mut b, mut j) = setup();
    assert!(engine::restore(&mut s, &mut b, &mut j).is_err());
    assert_eq!((b.reads, b.writes, j.calls), (0, 0, 0));
}
#[test]
fn stored_originals_and_approvals_cannot_be_rewritten() {
    let mut s = session(ActionKind::LowerPriorities);
    let (mut b, _) = setup();
    let mut db = Database::open(Path::new(":memory:")).unwrap();
    db.create(&s).unwrap();
    engine::apply(&mut s, &mut b, &mut db).unwrap();
    let old = s.clone();
    s.changes[0].before = Value::GpuPriority(3);
    assert!(db.save(&s).is_err());
    s = old.clone();
    s.plan.force_consent = true;
    assert!(db.save(&s).is_err());
    s = old.clone();
    s.changes.clear();
    assert!(db.save(&s).is_err());
    assert_eq!(
        db.active().unwrap().unwrap().changes[0].before,
        old.changes[0].before
    );
}
#[test]
fn duplicate_and_unapproved_properties_fail_record_validation() {
    let mut s = session(ActionKind::LowerPriorities);
    s.stage = Stage::Active;
    let c = Change {
        target: id(20),
        before: Value::GpuPriority(2),
        applied: Value::GpuPriority(1),
        state: ChangeState::Applied,
        detail: "fixture".into(),
    };
    s.changes = vec![c.clone(), c];
    assert!(integrity::validate_record(&s).is_err());
    s.changes.pop();
    s.plan.options.gpu_priority = false;
    s.plan.options.cpu_priority = true;
    assert!(integrity::validate_record(&s).is_err());
}
#[test]
fn missing_game_or_provenance_cannot_become_a_new_plan() {
    let mut s = session(ActionKind::Close);
    s.plan.game = None;
    let mut db = Database::open(Path::new(":memory:")).unwrap();
    assert!(db.create(&s).is_err());
    s = session(ActionKind::Close);
    s.plan.actions[0].target.provenance = None;
    assert!(db.create(&s).is_err());
}
#[test]
fn session_with_unresolved_settings_cannot_claim_completion() {
    let mut s = session(ActionKind::LowerPriorities);
    let (mut b, mut j) = setup();
    engine::apply(&mut s, &mut b, &mut j).unwrap();
    s.stage = Stage::Restored;
    assert!(integrity::validate_record(&s).is_err());
}
