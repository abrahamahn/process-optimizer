"""Expected target exits during a finite reviewed app-group closure are not session cancellation."""
from pathlib import Path

def edit(path, old, new):
    p=Path(path);s=p.read_text(encoding='utf-8')
    assert s.count(old)==1, f'Source drift: {path}'
    p.write_text(s.replace(old,new),encoding='utf-8',newline='\n')

edit('src/engine.rs', '''        b.authorize(&action.target).map_err(|e| {
            format!(
                "Application stopped before further mutations: {}",
                e.message
            )
        })?;''', '''        if let Err(e) = b.authorize(&action.target) {
            if e.kind != FaultKind::Gone {
                return Err(format!("Application stopped before further mutations: {}", e.message));
            }
            // A previously closed app may have ended its own helpers. Do not
            // mistake their exact-lifetime exit for cancellation of the game.
            if matches!(action.action, ActionKind::Close | ActionKind::ForceClose) {
                s.closed.push(CloseRecord {
                    target: action.target.clone(),
                    force_allowed: action.action == ActionKind::ForceClose,
                    state: CloseState::AlreadyGone,
                    detail: "Original target exited before its action. No close was sent and no replacement was touched.".into(),
                });
            }
            s.note(format!("Skipped exited original target PID {}: {}", action.target.pid, e.message));
            j.save(s)?;
            continue;
        }''')
p=Path('tests/product_workflows.rs')
p.write_text(p.read_text(encoding='utf-8')+'''
struct GroupClosure {
    called: Vec<u32>,
    gate: FaultKind,
}
impl Backend for GroupClosure {
    fn authorize(&mut self, id: &Identity) -> NativeResult<()> {
        if !self.called.is_empty() && id.pid == 11 {
            Err(Fault::new(self.gate, "fixture target exit or session cancellation"))
        } else { Ok(()) }
    }
    fn read(&mut self, _: &Identity, _: Property) -> NativeResult<Value> { panic!("close-only fixture") }
    fn write(&mut self, _: &Identity, _: &Value) -> NativeResult<()> { panic!("close-only fixture") }
    fn close(&mut self, target: &Identity, _: bool, _: u32) -> NativeResult<CloseState> {
        self.called.push(target.pid);
        Ok(CloseState::ClosedGracefully)
    }
}
fn group_session() -> Session {
    let mut s = session();
    for pid in [11, 12] {
        s.plan.actions.push(ApprovedAction { target: id(pid, "background"), action: ActionKind::Close, reopen: None });
    }
    s
}
#[test]
fn expected_helper_exit_keeps_the_game_session_active_and_continues_reviewed_actions() {
    let mut s = group_session();
    let mut backend = GroupClosure { called: vec![], gate: FaultKind::Gone };
    engine::apply(&mut s, &mut backend, &mut Store::default()).unwrap();
    assert_eq!(backend.called, vec![10, 12]);
    assert_eq!(s.stage, Stage::Active);
    assert_eq!(s.closed[1].state, CloseState::AlreadyGone);
    assert!(s.reopened.is_empty());
}
#[test]
fn session_cancellation_is_not_treated_as_a_harmless_helper_exit() {
    let mut s = group_session();
    let mut backend = GroupClosure { called: vec![], gate: FaultKind::Denied };
    assert!(engine::apply(&mut s, &mut backend, &mut Store::default()).is_err());
    assert_eq!(backend.called, vec![10]);
}
''',encoding='utf-8',newline='\n')
edit('specs/03-session-recovery-and-architecture.md', 'S-02: Cancel during application is checked between bounded actions.', 'S-02: Cancel during application is checked between bounded actions. An exact target that exits after whole-plan preflight is skipped as AlreadyGone without a native call; expected helper exits do not cancel the game session. Authorization revocation or game cancellation still stops new actions. An error after a native close call remains uncertain, never a harmless-exit inference.')
print('Reviewed finite group closure now distinguishes target exit from session cancellation.')
