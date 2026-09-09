use crate::{journal::Journal, model::*, policy};

pub trait Backend {
    /// Revalidate identity, ownership, critical status, game family and protected paths.
    fn authorize(&mut self, id: &Identity) -> NativeResult<()>;
    fn read(&mut self, id: &Identity, property: Property) -> NativeResult<Value>;
    fn write(&mut self, id: &Identity, value: &Value) -> NativeResult<()>;
    fn close(&mut self, id: &Identity, force: bool, timeout_ms: u32) -> NativeResult<CloseState>;
}

pub fn apply<B: Backend, J: Journal>(s: &mut Session, b: &mut B, j: &mut J) -> AppResult<()> {
    policy::validate(&s.plan)?;
    s.stage = Stage::Preparing;
    j.save(s)?;
    for action in s.plan.actions.clone() {
        if let Err(e) = b.authorize(&action.target) {
            s.note(format!("Skipped {}: {}", action.target.pid, e.message));
            j.save(s)?;
            continue;
        }
        match action.action {
            ActionKind::LowerPriorities => {
                let o = &s.plan.options;
                let properties = [(o.gpu_priority, Property::GpuPriority), (o.cpu_priority, Property::CpuPriority), (o.eco_qos, Property::EcoQos), (o.memory_priority, Property::MemoryPriority)];
                for (enabled, property) in properties {
                    if !enabled { continue; }
                    let before = match b.read(&action.target, property) {
                        Ok(v) => v,
                        Err(e) => { s.note(format!("{} {property:?} unavailable: {}", action.target.pid, e.message)); j.save(s)?; continue; }
                    };
                    let desired = before.background();
                    if desired == before { continue; }
                    s.changes.push(Change { target: action.target.clone(), before, applied: desired.clone(), state: ChangeState::Prepared, detail: "Write-ahead intent; setter may or may not have run.".into() });
                    // This commit MUST succeed before touching the target process.
                    j.save(s)?;
                    let idx = s.changes.len() - 1;
                    match b.write(&action.target, &desired) {
                        Ok(()) => match b.read(&action.target, property) {
                            Ok(actual) if actual == desired => { s.changes[idx].state = ChangeState::Applied; s.changes[idx].detail = "Applied and read back.".into(); }
                            Ok(_) => { s.changes[idx].detail = "Read-back differed; recovery must compare before writing.".into(); }
                            Err(e) => { s.changes[idx].detail = format!("Read-back failed: {}", e.message); }
                        },
                        Err(e) => { s.changes[idx].detail = format!("Setter failed or partially applied: {}", e.message); }
                    }
                    j.save(s)?;
                }
            }
            ActionKind::Close | ActionKind::ForceClose => {
                let force = action.action == ActionKind::ForceClose;
                s.closed.push(CloseRecord { target: action.target.clone(), force_allowed: force, state: CloseState::IntentRecorded, detail: "Close intent recorded. This is not an application-memory snapshot.".into() });
                j.save(s)?;
                let idx = s.closed.len() - 1;
                match b.close(&action.target, force, s.plan.options.close_timeout_ms) {
                    Ok(state) => { s.closed[idx].detail = format!("{state:?}. Reopening cannot restore unsaved work."); s.closed[idx].state = state; }
                    Err(e) => { s.closed[idx].state = CloseState::Failed; s.closed[idx].detail = e.message; }
                }
                j.save(s)?;
            }
        }
    }
    s.stage = Stage::Active;
    j.save(s)
}

/// Idempotent, reverse-order compare-and-restore; never replays destructive actions.
pub fn restore<B: Backend, J: Journal>(s: &mut Session, b: &mut B, j: &mut J) -> AppResult<()> {
    s.stage = Stage::Restoring;
    j.save(s)?;
    for idx in (0..s.changes.len()).rev() {
        if s.changes[idx].state.resolved() { continue; }
        let change = s.changes[idx].clone();
        match b.read(&change.target, change.before.property()) {
            Err(e) if e.kind == FaultKind::Gone => {
                s.changes[idx].state = ChangeState::ProcessGone;
                s.changes[idx].detail = "Original process no longer exists; no replacement PID was touched.".into();
            }
            Err(e) => { s.changes[idx].detail = format!("Recovery read failed: {}", e.message); }
            Ok(current) if current == change.before => {
                s.changes[idx].state = ChangeState::Restored;
                s.changes[idx].detail = "Already at original value; no write needed.".into();
            }
            Ok(current) if current == change.applied => {
                s.changes[idx].state = ChangeState::RestorePrepared;
                j.save(s)?;
                match b.write(&change.target, &change.before) {
                    Ok(()) => match b.read(&change.target, change.before.property()) {
                        Ok(actual) if actual == change.before => { s.changes[idx].state = ChangeState::Restored; s.changes[idx].detail = "Original value restored and verified.".into(); }
                        Ok(_) => { s.changes[idx].state = ChangeState::Conflict; s.changes[idx].detail = "Value changed during restoration; not overwritten again.".into(); }
                        Err(e) if e.kind == FaultKind::Gone => { s.changes[idx].state = ChangeState::ProcessGone; s.changes[idx].detail = e.message; }
                        Err(e) => { s.changes[idx].detail = e.message; }
                    },
                    Err(e) => { s.changes[idx].detail = format!("Recovery write failed: {}", e.message); }
                }
            }
            Ok(_) => {
                s.changes[idx].state = ChangeState::Conflict;
                s.changes[idx].detail = "An external change was detected. Kept current value; review required.".into();
            }
        }
        j.save(s)?;
    }
    s.stage = if s.changes.iter().all(|c| c.state.resolved()) { Stage::Restored } else { Stage::RecoveryNeeded };
    if s.closed.iter().any(|c| c.state == CloseState::IntentRecorded) {
        s.note("A close outcome is unknown after interruption. No close or reopen was replayed.");
    }
    j.save(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)] struct MemoryJournal { writes: usize, fail_at: Option<usize>, saved: Option<Session> }
    impl Journal for MemoryJournal {
        fn save(&mut self, s: &Session) -> AppResult<()> { self.writes += 1; if self.fail_at == Some(self.writes) { return Err("disk failure".into()); } self.saved = Some(s.clone()); Ok(()) }
    }
    struct Fake { value: Value, writes: usize, close_calls: usize, denied: bool, gone: bool, write_error_after_effect: bool }
    impl Backend for Fake {
        fn authorize(&mut self, _: &Identity) -> NativeResult<()> { if self.denied { Err(Fault::new(FaultKind::Denied, "protected")) } else { Ok(()) } }
        fn read(&mut self, _: &Identity, _: Property) -> NativeResult<Value> { if self.gone { Err(Fault::new(FaultKind::Gone, "gone")) } else { Ok(self.value.clone()) } }
        fn write(&mut self, _: &Identity, v: &Value) -> NativeResult<()> { self.writes += 1; self.value = v.clone(); if self.write_error_after_effect { Err(Fault::new(FaultKind::Other, "ambiguous")) } else { Ok(()) } }
        fn close(&mut self, _: &Identity, force: bool, _: u32) -> NativeResult<CloseState> { self.close_calls += 1; Ok(if force { CloseState::Terminated } else { CloseState::KeptOpen }) }
    }
    fn setup(action: ActionKind) -> (Session, Fake, MemoryJournal) {
        let id = Identity { pid: 20, created: 100, path: r"C:\Apps\background.exe".into(), session_id: 1 };
        let p = Plan { game_path: r"C:\Game\game.exe".into(), game: None, actions: vec![ApprovedAction { target: id, action }], protected_paths: vec![], options: Options::default(), consent: true, force_consent: action == ActionKind::ForceClose };
        (Session::new("test".into(), p), Fake { value: Value::GpuPriority(2), writes: 0, close_calls: 0, denied: false, gone: false, write_error_after_effect: false }, MemoryJournal::default())
    }
    #[test] fn apply_and_restore_original() { let (mut s, mut b, mut j) = setup(ActionKind::LowerPriorities); apply(&mut s, &mut b, &mut j).unwrap(); assert_eq!(b.value, Value::GpuPriority(1)); restore(&mut s, &mut b, &mut j).unwrap(); assert_eq!(b.value, Value::GpuPriority(2)); assert_eq!(s.stage, Stage::Restored); }
    #[test] fn prepared_intent_is_durable_before_mutation() { let (mut s, mut b, mut j) = setup(ActionKind::LowerPriorities); j.fail_at = Some(2); assert!(apply(&mut s, &mut b, &mut j).is_err()); assert_eq!(b.writes, 0); }
    #[test] fn external_change_is_not_overwritten() { let (mut s, mut b, mut j) = setup(ActionKind::LowerPriorities); apply(&mut s, &mut b, &mut j).unwrap(); b.value = Value::GpuPriority(3); restore(&mut s, &mut b, &mut j).unwrap(); assert_eq!(b.value, Value::GpuPriority(3)); assert_eq!(s.stage, Stage::RecoveryNeeded); }
    #[test] fn crashed_after_setter_can_recover_prepared_record() { let (mut s, mut b, mut j) = setup(ActionKind::LowerPriorities); j.fail_at = Some(3); assert!(apply(&mut s, &mut b, &mut j).is_err()); let mut durable = j.saved.clone().unwrap(); assert_eq!(durable.changes[0].state, ChangeState::Prepared); j.fail_at = None; restore(&mut durable, &mut b, &mut j).unwrap(); assert_eq!(b.value, Value::GpuPriority(2)); }
    #[test] fn restore_is_idempotent() { let (mut s, mut b, mut j) = setup(ActionKind::LowerPriorities); apply(&mut s, &mut b, &mut j).unwrap(); restore(&mut s, &mut b, &mut j).unwrap(); let count = b.writes; restore(&mut s, &mut b, &mut j).unwrap(); assert_eq!(b.writes, count); }
    #[test] fn pid_reuse_or_exit_does_not_receive_restore() { let (mut s, mut b, mut j) = setup(ActionKind::LowerPriorities); apply(&mut s, &mut b, &mut j).unwrap(); b.gone = true; restore(&mut s, &mut b, &mut j).unwrap(); assert_eq!(b.writes, 1); assert_eq!(s.changes[0].state, ChangeState::ProcessGone); }
    #[test] fn protected_targets_are_skipped() { let (mut s, mut b, mut j) = setup(ActionKind::ForceClose); b.denied = true; apply(&mut s, &mut b, &mut j).unwrap(); assert_eq!(b.close_calls, 0); }
    #[test] fn graceful_close_never_escalates() { let (mut s, mut b, mut j) = setup(ActionKind::Close); apply(&mut s, &mut b, &mut j).unwrap(); assert_eq!(s.closed[0].state, CloseState::KeptOpen); assert!(!s.closed[0].force_allowed); }
    #[test] fn approved_force_path_is_distinct() { let (mut s, mut b, mut j) = setup(ActionKind::ForceClose); apply(&mut s, &mut b, &mut j).unwrap(); assert_eq!(s.closed[0].state, CloseState::Terminated); }
    #[test] fn recovery_never_replays_a_close() { let (mut s, mut b, mut j) = setup(ActionKind::ForceClose); apply(&mut s, &mut b, &mut j).unwrap(); restore(&mut s, &mut b, &mut j).unwrap(); assert_eq!(b.close_calls, 1); }
    #[test] fn ambiguous_set_error_keeps_recovery_intent() { let (mut s, mut b, mut j) = setup(ActionKind::LowerPriorities); b.write_error_after_effect = true; apply(&mut s, &mut b, &mut j).unwrap(); assert_eq!(s.changes[0].state, ChangeState::Prepared); b.write_error_after_effect = false; restore(&mut s, &mut b, &mut j).unwrap(); assert_eq!(b.value, Value::GpuPriority(2)); }
    #[test] fn close_is_not_attempted_after_journal_failure() { let (mut s, mut b, mut j) = setup(ActionKind::Close); j.fail_at = Some(2); assert!(apply(&mut s, &mut b, &mut j).is_err()); assert_eq!(b.close_calls, 0); }
    #[test] fn already_low_priority_is_untouched() { let (mut s, mut b, mut j) = setup(ActionKind::LowerPriorities); b.value = Value::GpuPriority(0); apply(&mut s, &mut b, &mut j).unwrap(); assert_eq!(b.writes, 0); assert!(s.changes.is_empty()); }
}
