//! Synthetic identities and in-memory stores only; no user applications.
use process_optimizer::{
    applications::{self, app_key},
    engine::{self, Backend},
    integrity,
    journal::{Database, Journal},
    model::*,
    policy, profiles, report,
};
use std::path::Path;

fn id(pid: u32, name: &str) -> Identity {
    Identity {
        pid,
        created: u64::from(pid) + 100,
        path: format!(r"C:\Apps\{name}.exe"),
        session_id: 1,
        provenance: Some(Provenance {
            owner_sid: "fixture-user".into(),
            logon_id: 100,
            image_file_id: format!("file-{name}"),
        }),
    }
}
fn row(id: Identity) -> ProcessRow {
    ProcessRow {
        identity: id,
        parent_pid: 0,
        name: "fixture.exe".into(),
        protected_reason: None,
        gpu: vec![],
    }
}
fn action() -> ApprovedAction {
    ApprovedAction {
        target: id(10, "background"),
        action: ActionKind::Close,
        reopen: None,
    }
}
fn stamp() -> ReopenApproval {
    ReopenApproval {
        image_file_id: "file-background".into(),
        file_size: 4096,
        modified: 100,
    }
}
fn session() -> Session {
    Session::new(
        "fixture-session".into(),
        Plan {
            manual_mode: false,
            game_path: id(99, "game").path.clone(),
            game: Some(id(99, "game")),
            actions: vec![action()],
            protected_paths: vec![],
            options: Options::default(),
            consent: true,
            force_consent: false,
            experimental_consent: false,
        },
    )
}
fn profile() -> profiles::GameProfile {
    profiles::capture(&id(99, "game"), &[action()], &Options::default()).unwrap()
}
fn closed_session(state: CloseState) -> Session {
    let mut s = session();
    s.stage = Stage::Active;
    s.plan.actions[0].reopen = Some(stamp());
    s.closed.push(CloseRecord {
        target: id(10, "background"),
        force_allowed: false,
        state,
        detail: "fixture closure".into(),
    });
    s
}
#[derive(Default)]
struct Store {
    saves: usize,
    fail: Option<usize>,
    last: Option<Session>,
}
impl Journal for Store {
    fn save(&mut self, s: &Session) -> AppResult<()> {
        self.saves += 1;
        if self.fail == Some(self.saves) {
            return Err("injected disk fault".into());
        }
        integrity::validate_record(s)?;
        self.last = Some(s.clone());
        Ok(())
    }
}
#[derive(Default)]
struct Native {
    calls: usize,
    outcome: Option<ReopenState>,
    unexpected_reads: usize,
}
impl Backend for Native {
    fn authorize(&mut self, _: &Identity) -> NativeResult<()> {
        Ok(())
    }
    fn read(&mut self, _: &Identity, _: Property) -> NativeResult<Value> {
        self.unexpected_reads += 1;
        Err(Fault::new(FaultKind::Other, "unexpected native read"))
    }
    fn write(&mut self, _: &Identity, _: &Value) -> NativeResult<()> {
        panic!("no policy writes in these close-only tests")
    }
    fn close(&mut self, _: &Identity, _: bool, _: u32) -> NativeResult<CloseState> {
        panic!("recovery must never close")
    }
    fn reopen(&mut self, id: &Identity, _: &ReopenApproval) -> NativeResult<ReopenRecord> {
        self.calls += 1;
        let state = self.outcome.clone().unwrap_or(ReopenState::Started);
        let child = if state == ReopenState::Started {
            let mut next = id.clone();
            next.pid += 1000;
            next.created += 1000;
            Some(next)
        } else {
            None
        };
        Ok(ReopenRecord {
            target: id.clone(),
            state,
            child,
            detail: "fixture result".into(),
        })
    }
}
#[test]
fn executable_group_is_finite_and_lifetime_scoped() {
    let a = id(10, "background");
    let b = id(11, "background");
    let mut other = id(12, "background");
    other.path = r"C:\Different\background.exe".into();
    let rows = vec![
        row(a.clone()),
        row(b.clone()),
        row(other),
        row(id(13, "helper")),
    ];
    assert_eq!(applications::group(&rows, &a).unwrap(), vec![a, b]);
}
#[test]
fn group_excludes_other_owners_sessions_logons_and_versions() {
    let a = id(10, "background");
    let mut rows = vec![row(a.clone())];
    for n in 0..4 {
        let mut b = id(20 + n, "background");
        match n {
            0 => b.session_id = 2,
            1 => b.provenance.as_mut().unwrap().owner_sid = "other".into(),
            2 => b.provenance.as_mut().unwrap().logon_id += 1,
            _ => b.provenance.as_mut().unwrap().image_file_id = "new-file".into(),
        };
        rows.push(row(b));
    }
    assert_eq!(applications::group(&rows, &a).unwrap().len(), 1);
}
#[test]
fn group_rejects_stale_selection_and_overflow() {
    let a = id(10, "background");
    let rows = (10..43)
        .map(|pid| row(id(pid, "background")))
        .collect::<Vec<_>>();
    assert!(applications::group(&rows, &a).is_err());
    let mut old = a;
    old.created += 1;
    assert!(applications::group(&rows, &old).is_err());
}
#[test]
fn profile_never_retains_reopen_or_experimental_approval() {
    let mut a = action();
    a.reopen = Some(stamp());
    let options = Options {
        gpu_priority: true,
        ..Options::default()
    };
    let p = profiles::capture(&id(99, "game"), &[a], &options).unwrap();
    assert!(!p.options.gpu_priority);
    let body = serde_json::to_string(&p).unwrap();
    assert!(
        !body.contains("consent")
            && !body.contains("reopen")
            && !body.contains("owner_sid")
            && !body.contains("created")
    );
    let preview = profiles::resolve(
        &p,
        &id(99, "game"),
        &[row(id(99, "game")), row(id(20, "background"))],
        &[],
    )
    .unwrap();
    assert_eq!(preview.actions[0].target.pid, 20);
    assert!(preview.actions[0].reopen.is_none());
}
#[test]
fn profile_rejects_force_and_mixed_group_actions() {
    let mut a = action();
    a.action = ActionKind::ForceClose;
    assert!(profiles::capture(&id(99, "game"), &[a], &Options::default()).is_err());
    let mut b = action();
    b.target.pid += 1;
    b.action = ActionKind::LowerPriorities;
    assert!(profiles::capture(
        &id(99, "game"),
        &[action(), b],
        &Options {
            cpu_priority: true,
            ..Options::default()
        }
    )
    .is_err());
}
#[test]
fn profile_load_expands_current_group_but_has_no_future_rule() {
    let p = profile();
    let mut rows = vec![
        row(id(99, "game")),
        row(id(20, "background")),
        row(id(21, "background")),
    ];
    let preview = profiles::resolve(&p, &id(99, "game"), &rows, &[]).unwrap();
    rows.push(row(id(22, "background")));
    assert_eq!(preview.actions.len(), 2);
    assert!(preview.actions.iter().all(|a| a.target.pid != 22));
}
#[test]
fn profile_game_update_is_not_silently_trusted() {
    let p = profile();
    let mut game = id(99, "game");
    game.provenance.as_mut().unwrap().image_file_id = "updated".into();
    assert!(profiles::resolve(&p, &game, &[row(game.clone())], &[]).is_err());
}
#[test]
fn profile_updated_or_absent_app_is_not_selected() {
    let p = profile();
    let mut app = id(10, "background");
    app.provenance.as_mut().unwrap().image_file_id = "updated".into();
    let preview =
        profiles::resolve(&p, &id(99, "game"), &[row(id(99, "game")), row(app)], &[]).unwrap();
    assert!(preview.actions.is_empty());
    assert!(preview.notices.iter().any(|n| n.contains("unverifiable")));
}
#[test]
fn profile_protection_applies_to_the_whole_matching_group() {
    let p = profile();
    let mut blocked = row(id(21, "background"));
    blocked.protected_reason = Some("fixture protection".into());
    let preview = profiles::resolve(
        &p,
        &id(99, "game"),
        &[row(id(99, "game")), row(id(20, "background")), blocked],
        &[],
    )
    .unwrap();
    assert!(preview.actions.is_empty());
    let preview = profiles::resolve(
        &p,
        &id(99, "game"),
        &[row(id(99, "game")), row(id(20, "background"))],
        &[id(20, "background").path],
    )
    .unwrap();
    assert!(preview.actions.is_empty());
}
#[test]
fn profile_expansion_limit_never_returns_a_truncated_plan() {
    let p = profile();
    let mut rows = (10..43)
        .map(|pid| row(id(pid, "background")))
        .collect::<Vec<_>>();
    rows.push(row(id(99, "game")));
    assert!(profiles::resolve(&p, &id(99, "game"), &rows, &[]).is_err());
}
#[test]
fn profile_rejects_unknown_fields_invalid_policies_and_duplicate_apps() {
    let p = profile();
    let mut value = serde_json::to_value(&p).unwrap();
    value["command"] = "powershell".into();
    assert!(serde_json::from_value::<profiles::GameProfile>(value).is_err());
    let mut p = profile();
    p.options.gpu_priority = true;
    assert!(profiles::validate(&p).is_err());
    let mut p = profile();
    p.targets.push(p.targets[0].clone());
    assert!(profiles::validate(&p).is_err());
}
#[test]
fn profile_storage_round_trip_overwrite_delete_preserves_session() {
    let mut db = Database::open(Path::new(":memory:")).unwrap();
    db.create(&session()).unwrap();
    let mut p = profile();
    db.save_profile(&p).unwrap();
    assert_eq!(db.profile(&p.game.path).unwrap().unwrap().targets.len(), 1);
    p.targets.clear();
    db.save_profile(&p).unwrap();
    assert!(db
        .profile(&p.game.path)
        .unwrap()
        .unwrap()
        .targets
        .is_empty());
    db.delete_profile(&p.game.path).unwrap();
    assert!(db.profile(&p.game.path).unwrap().is_none());
    assert!(db.active().unwrap().is_some());
}
#[test]
fn profile_count_is_bounded_and_existing_recipe_can_be_replaced() {
    let mut db = Database::open(Path::new(":memory:")).unwrap();
    for n in 0..profiles::MAX_PROFILES {
        let mut p = profile();
        p.game = app_key(&id(99, &format!("game{n}"))).unwrap();
        db.save_profile(&p).unwrap();
    }
    assert!(db.save_profile(&profile()).is_err());
    let mut p = profile();
    p.game = app_key(&id(99, "game0")).unwrap();
    db.save_profile(&p).unwrap();
}
#[test]
fn verified_close_reopens_once_and_reports_a_new_lifetime() {
    let mut s = closed_session(CloseState::ClosedGracefully);
    let mut n = Native::default();
    let mut j = Store::default();
    engine::restore(&mut s, &mut n, &mut j).unwrap();
    assert_eq!(s.stage, Stage::Restored);
    assert_eq!(s.reopened[0].state, ReopenState::Started);
    assert_ne!(s.reopened[0].child.as_ref().unwrap().pid, 10);
    engine::restore(&mut s, &mut n, &mut j).unwrap();
    assert_eq!(n.calls, 1);
    assert_eq!(n.unexpected_reads, 0);
}
#[test]
fn no_reopen_permission_means_no_launch() {
    let mut s = closed_session(CloseState::ClosedGracefully);
    s.plan.actions[0].reopen = None;
    let mut n = Native::default();
    engine::restore(&mut s, &mut n, &mut Store::default()).unwrap();
    assert_eq!(n.calls, 0);
    assert!(s.reopened.is_empty());
}
#[test]
fn unverified_or_unsent_close_never_reopens() {
    for state in [
        CloseState::NotRequested,
        CloseState::KeptOpen,
        CloseState::AlreadyGone,
        CloseState::IntentRecorded,
        CloseState::RequestedPending,
    ] {
        let mut s = closed_session(state);
        let mut n = Native::default();
        engine::restore(&mut s, &mut n, &mut Store::default()).unwrap();
        assert_eq!(n.calls, 0);
        assert_eq!(s.reopened[0].state, ReopenState::Skipped);
    }
}
#[test]
fn failed_reopen_intent_save_never_launches() {
    let mut s = closed_session(CloseState::ClosedGracefully);
    let mut n = Native::default();
    let mut j = Store {
        fail: Some(2),
        ..Default::default()
    };
    assert!(engine::restore(&mut s, &mut n, &mut j).is_err());
    assert_eq!(n.calls, 0);
}
#[test]
fn interruption_after_launch_does_not_launch_a_duplicate_on_recovery() {
    let mut s = closed_session(CloseState::ClosedGracefully);
    let mut n = Native::default();
    let mut j = Store {
        fail: Some(3),
        ..Default::default()
    };
    assert!(engine::restore(&mut s, &mut n, &mut j).is_err());
    assert_eq!(n.calls, 1);
    let mut durable = j.last.clone().unwrap();
    assert_eq!(durable.reopened[0].state, ReopenState::IntentRecorded);
    j.fail = None;
    engine::restore(&mut durable, &mut n, &mut j).unwrap();
    assert_eq!(n.calls, 1);
    assert_eq!(durable.stage, Stage::RecoveryNeeded);
    assert_eq!(durable.reopened[0].state, ReopenState::Indeterminate);
    engine::restore(&mut durable, &mut n, &mut j).unwrap();
    assert_eq!(n.calls, 1);
}
#[test]
fn locked_existing_or_changed_apps_have_explicit_terminal_outcomes() {
    for state in [
        ReopenState::Deferred,
        ReopenState::AlreadyRunning,
        ReopenState::Skipped,
    ] {
        let mut s = closed_session(CloseState::ClosedGracefully);
        let mut n = Native {
            outcome: Some(state.clone()),
            ..Default::default()
        };
        let mut j = Store::default();
        engine::restore(&mut s, &mut n, &mut j).unwrap();
        assert_eq!(s.reopened[0].state, state);
        engine::restore(&mut s, &mut n, &mut j).unwrap();
        assert_eq!(n.calls, 1);
    }
}
#[test]
fn uncertain_reopen_cannot_claim_full_restoration() {
    let mut s = closed_session(CloseState::ClosedGracefully);
    let mut n = Native {
        outcome: Some(ReopenState::Indeterminate),
        ..Default::default()
    };
    engine::restore(&mut s, &mut n, &mut Store::default()).unwrap();
    assert_eq!(s.stage, Stage::RecoveryNeeded);
    s.stage = Stage::Restored;
    assert!(integrity::validate_record(&s).is_err());
}
#[test]
fn reopen_requires_matching_stamp_graceful_action_and_unique_app() {
    let mut s = closed_session(CloseState::ClosedGracefully);
    s.plan.actions[0].reopen.as_mut().unwrap().image_file_id = "other".into();
    assert!(policy::validate(&s.plan).is_err());
    let mut s = closed_session(CloseState::ClosedGracefully);
    s.plan.actions[0].action = ActionKind::ForceClose;
    s.plan.force_consent = true;
    assert!(policy::validate(&s.plan).is_err());
    let mut s = closed_session(CloseState::ClosedGracefully);
    let mut duplicate = s.plan.actions[0].clone();
    duplicate.target.pid += 1;
    s.plan.actions.push(duplicate);
    assert!(policy::validate(&s.plan).is_err());
}
#[test]
fn legacy_v2_is_recoverable_but_cannot_acquire_reopen_permission() {
    let mut s = session();
    s.schema = 2;
    s.stage = Stage::Active;
    let mut json = serde_json::to_value(&s).unwrap();
    json.as_object_mut().unwrap().remove("reopened");
    json["plan"]["actions"][0]
        .as_object_mut()
        .unwrap()
        .remove("reopen");
    let mut old: Session = serde_json::from_value(json).unwrap();
    assert!(integrity::validate_record(&old).is_ok());
    engine::restore(&mut old, &mut Native::default(), &mut Store::default()).unwrap();
    assert_eq!(old.schema, 2);
    assert_eq!(old.stage, Stage::Restored);
    s.plan.actions[0].reopen = Some(stamp());
    assert!(integrity::validate_record(&s).is_err());
    let mut fresh = session();
    fresh.schema = 2;
    assert!(Database::open(Path::new(":memory:"))
        .unwrap()
        .create(&fresh)
        .is_err());
}
#[test]
fn reopen_records_must_be_in_plan_and_based_on_verified_closure() {
    let mut s = closed_session(CloseState::KeptOpen);
    s.reopened.push(ReopenRecord {
        target: id(10, "background"),
        state: ReopenState::IntentRecorded,
        child: None,
        detail: "bad".into(),
    });
    assert!(integrity::validate_record(&s).is_err());
    s.closed[0].state = CloseState::ClosedGracefully;
    s.reopened[0].target = id(30, "unapproved");
    let mut n = Native::default();
    assert!(engine::restore(&mut s, &mut n, &mut Store::default()).is_err());
    assert_eq!(n.calls, 0);
}
#[test]
fn human_report_separates_settings_closure_and_reopening() {
    let mut s = closed_session(CloseState::ClosedGracefully);
    let mut n = Native::default();
    engine::restore(&mut s, &mut n, &mut Store::default()).unwrap();
    let text = report::render(&s);
    for label in [
        "SETTINGS",
        "CLOSE REQUESTS",
        "OPTIONAL APP REOPENING",
        "not measured",
        "unsaved work",
    ] {
        assert!(text.contains(label));
    }
}

#[test]
fn apply_rejects_existing_reopen_evidence_before_saving_anything() {
    let mut s = session();
    s.reopened.push(ReopenRecord {
        target: id(10, "background"),
        state: ReopenState::Skipped,
        child: None,
        detail: "prior evidence".into(),
    });
    let mut store = Store::default();
    assert!(engine::apply(&mut s, &mut Native::default(), &mut store).is_err());
    assert_eq!(store.saves, 0);
}
#[test]
fn profile_cannot_coalesce_different_file_identities_at_one_path() {
    let mut other = action();
    other.target.pid += 1;
    other.target.provenance.as_mut().unwrap().image_file_id = "replacement-file".into();
    assert!(profiles::capture(&id(99, "game"), &[action(), other], &Options::default()).is_err());
}
#[test]
fn completed_session_requires_an_outcome_for_verified_approved_reopening() {
    let mut s = closed_session(CloseState::ClosedGracefully);
    s.stage = Stage::Restored;
    assert!(integrity::validate_record(&s).is_err());
    s.reopened.push(ReopenRecord {
        target: id(10, "background"),
        state: ReopenState::Deferred,
        child: None,
        detail: "desktop unavailable".into(),
    });
    assert!(integrity::validate_record(&s).is_ok());
}

struct GroupClosure {
    called: Vec<u32>,
    gate: FaultKind,
}
impl Backend for GroupClosure {
    fn authorize(&mut self, id: &Identity) -> NativeResult<()> {
        if !self.called.is_empty() && id.pid == 11 {
            Err(Fault::new(
                self.gate,
                "fixture target exit or session cancellation",
            ))
        } else {
            Ok(())
        }
    }
    fn read(&mut self, _: &Identity, _: Property) -> NativeResult<Value> {
        panic!("close-only fixture")
    }
    fn write(&mut self, _: &Identity, _: &Value) -> NativeResult<()> {
        panic!("close-only fixture")
    }
    fn close(&mut self, target: &Identity, _: bool, _: u32) -> NativeResult<CloseState> {
        self.called.push(target.pid);
        Ok(CloseState::ClosedGracefully)
    }
}
fn group_session() -> Session {
    let mut s = session();
    for pid in [11, 12] {
        s.plan.actions.push(ApprovedAction {
            target: id(pid, "background"),
            action: ActionKind::Close,
            reopen: None,
        });
    }
    s
}
#[test]
fn expected_helper_exit_keeps_the_game_session_active_and_continues_reviewed_actions() {
    let mut s = group_session();
    let mut backend = GroupClosure {
        called: vec![],
        gate: FaultKind::Gone,
    };
    engine::apply(&mut s, &mut backend, &mut Store::default()).unwrap();
    assert_eq!(backend.called, vec![10, 12]);
    assert_eq!(s.stage, Stage::Active);
    assert_eq!(s.closed[1].state, CloseState::AlreadyGone);
    assert!(s.reopened.is_empty());
}
#[test]
fn session_cancellation_is_not_treated_as_a_harmless_helper_exit() {
    let mut s = group_session();
    let mut backend = GroupClosure {
        called: vec![],
        gate: FaultKind::Denied,
    };
    assert!(engine::apply(&mut s, &mut backend, &mut Store::default()).is_err());
    assert_eq!(backend.called, vec![10]);
}
