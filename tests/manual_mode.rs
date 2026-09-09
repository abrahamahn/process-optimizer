//! Manual mode is an explicit lifecycle, never a synthetic game process.
use process_optimizer::{
    applications, integrity,
    journal::{Database, Journal},
    manual::{self, Config, RuleAction},
    model::*,
    policy,
};
use std::path::Path;
fn id(pid: u32) -> Identity {
    Identity {
        pid,
        created: 1,
        path: format!(r"C:\Fixture\app{pid}.exe"),
        session_id: 1,
        provenance: Some(Provenance {
            owner_sid: "test-user".into(),
            logon_id: 1,
            image_file_id: format!("file{pid}"),
        }),
    }
}
fn stamp(target: &Identity) -> ReopenApproval {
    ReopenApproval {
        image_file_id: target.provenance.as_ref().unwrap().image_file_id.clone(),
        file_size: 10,
        modified: 1,
    }
}
fn row(target: Identity) -> ProcessRow {
    ProcessRow {
        name: "test-app.exe".into(),
        identity: target,
        parent_pid: 0,
        protected_reason: None,
        gpu: vec![],
    }
}
fn config() -> Config {
    let i = id(10);
    Config {
        schema: 1,
        rules: vec![manual::approve(&i, stamp(&i), RuleAction::Close, 100).unwrap()],
    }
}
fn plan() -> Plan {
    manual::plan(&config(), &[row(id(10))], &id(99), &[], 101, false).unwrap()
}
#[test]
fn manual_on_has_no_game_path_or_game_process() {
    let p = plan();
    assert!(p.manual_mode);
    assert!(p.game.is_none());
    assert!(p.game_path.is_empty());
    assert!(policy::validate(&p).is_ok());
}
#[test]
fn legacy_attach_without_game_still_fails() {
    let mut p = plan();
    p.manual_mode = false;
    assert!(policy::validate(&p).is_err());
}
#[test]
fn manual_flag_cannot_hide_a_game_identity() {
    let mut p = plan();
    p.game = Some(id(99));
    assert!(policy::validate(&p).is_err());
}
#[test]
fn no_first_run_permission_means_no_cleanup() {
    assert!(manual::plan(&Config::default(), &[row(id(10))], &id(99), &[], 101, false).is_err());
}
#[test]
fn unapproved_apps_are_kept() {
    let p = manual::plan(
        &config(),
        &[row(id(10)), row(id(11))],
        &id(99),
        &[],
        101,
        false,
    )
    .unwrap();
    assert_eq!(p.actions.len(), 1);
    assert_eq!(p.actions[0].target.pid, 10);
}
#[test]
fn rules_do_not_match_a_replacement_file() {
    let mut target = id(10);
    target.provenance.as_mut().unwrap().image_file_id = "new-file".into();
    let p = manual::plan(&config(), &[row(target)], &id(99), &[], 101, false).unwrap();
    assert!(p.actions.is_empty());
}
#[test]
fn exact_owner_and_current_session_are_required() {
    for change in 0..3 {
        let mut target = id(10);
        match change {
            0 => target.session_id = 2,
            1 => target.provenance.as_mut().unwrap().owner_sid = "other".into(),
            _ => target.provenance.as_mut().unwrap().logon_id = 2,
        };
        assert!(
            manual::plan(&config(), &[row(target)], &id(99), &[], 101, false)
                .unwrap()
                .actions
                .is_empty()
        );
    }
}
#[test]
fn every_new_on_resolves_a_new_finite_lifetime() {
    let mut target = id(10);
    target.pid = 45;
    target.created = 200;
    let p = manual::plan(&config(), &[row(target)], &id(99), &[], 101, false).unwrap();
    assert_eq!(p.actions[0].target.pid, 45);
    assert_eq!(p.actions[0].target.created, 200);
}
#[test]
fn protection_wins_over_remembered_close_permission() {
    let p = manual::plan(
        &config(),
        &[row(id(10))],
        &id(99),
        &[id(10).path],
        101,
        false,
    )
    .unwrap();
    assert!(p.actions.is_empty());
}
#[test]
fn one_protected_group_member_keeps_the_entire_group() {
    let a = row(id(10));
    let mut b = a.clone();
    b.identity.pid = 11;
    b.protected_reason = Some("required".into());
    assert!(manual::plan(&config(), &[a, b], &id(99), &[], 101, false)
        .unwrap()
        .actions
        .is_empty());
}
#[test]
fn expired_or_clock_rolled_back_approval_cannot_start() {
    let c = config();
    for now in [99, 100 + manual::APPROVAL_SECONDS] {
        assert!(manual::plan(&c, &[row(id(10))], &id(99), &[], now, false).is_err());
    }
}
#[test]
fn file_timestamp_and_size_are_checked_by_match() {
    let c = config();
    let i = id(10);
    let mut s = stamp(&i);
    assert!(manual::matching_rule(&c, &i, &s, 101).is_some());
    s.modified += 1;
    assert!(manual::matching_rule(&c, &i, &s, 101).is_none());
}
#[test]
fn no_force_or_reopen_can_be_smuggled_into_manual_plan() {
    let mut p = plan();
    p.actions[0].action = ActionKind::ForceClose;
    p.force_consent = true;
    assert!(policy::validate(&p).is_err());
    let mut p = plan();
    p.actions[0].reopen = Some(stamp(&id(10)));
    assert!(policy::validate(&p).is_err());
}
#[test]
fn empty_matching_snapshot_is_honest_noop_not_wildcard() {
    let p = manual::plan(&config(), &[], &id(99), &[], 101, false).unwrap();
    assert!(p.actions.is_empty());
    assert!(policy::validate(&p).is_ok());
}
#[test]
fn group_overflow_is_rejected_not_truncated() {
    let rows = (10..43)
        .map(|pid| {
            let mut r = row(id(10));
            r.identity.pid = pid;
            r
        })
        .collect::<Vec<_>>();
    assert!(manual::plan(&config(), &rows, &id(99), &[], 101, false).is_err());
}
#[test]
fn gpu_is_only_enabled_for_opted_in_reduction() {
    let mut c = config();
    c.rules[0].action = RuleAction::ReduceLoad;
    let p = manual::plan(&c, &[row(id(10))], &id(99), &[], 101, false).unwrap();
    assert!(!p.options.gpu_priority && !p.experimental_consent);
    let p = manual::plan(&c, &[row(id(10))], &id(99), &[], 101, true).unwrap();
    assert!(p.options.gpu_priority && p.experimental_consent);
}
#[test]
fn invalid_and_duplicate_rules_fail_closed() {
    let mut c = config();
    c.rules.push(c.rules[0].clone());
    assert!(manual::validate(&c).is_err());
    let mut c = config();
    c.rules[0].app = applications::app_key(&id(11)).unwrap();
    assert!(manual::validate(&c).is_err());
}
#[test]
fn manual_session_and_permissions_survive_store_reopen() {
    let mut db = Database::open(Path::new(":memory:")).unwrap();
    db.save_manual_settings(&config()).unwrap();
    assert_eq!(db.manual_settings().unwrap().rules.len(), 1);
    let mut s = Session::new("manual".into(), plan());
    db.create(&s).unwrap();
    s.stage = Stage::Active;
    db.save(&s).unwrap();
    assert!(db.active().unwrap().unwrap().plan.manual_mode);
    db.request_stop(&s.id).unwrap();
    s.stage = Stage::Restored;
    db.save(&s).unwrap();
    assert!(db.active().unwrap().is_none());
    assert_eq!(db.manual_settings().unwrap().rules.len(), 1);
}
#[test]
fn legacy_record_cannot_inherit_manual_mode() {
    let mut s = Session::new("manual".into(), plan());
    s.schema = 3;
    assert!(integrity::validate_record(&s).is_err());
    s.schema = 2;
    assert!(integrity::validate_record(&s).is_err());
}
#[test]
fn older_schema_three_records_remain_recoverable() {
    let mut s = Session::new("old".into(), plan());
    s.schema = 3;
    s.plan.manual_mode = false;
    s.plan.game_path = id(99).path.clone();
    s.plan.game = Some(id(99));
    let mut v = serde_json::to_value(&s).unwrap();
    v["plan"].as_object_mut().unwrap().remove("manual_mode");
    let old: Session = serde_json::from_value(v).unwrap();
    assert!(!old.plan.manual_mode);
    assert!(integrity::validate_record(&old).is_ok());
}
