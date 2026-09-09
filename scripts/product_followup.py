"""Exact-match corrections from native validation and final product review."""
from pathlib import Path

def edit(path, old, new):
    p = Path(path)
    s = p.read_text(encoding='utf-8')
    assert s.count(old) == 1, f'Unexpected source while correcting {path}: {old[:80]}'
    p.write_text(s.replace(old, new), encoding='utf-8', newline='\n')

edit('src/windows/reopen.rs', 'offset < 64 || offset > 1024 * 1024', '!(64..=1024 * 1024).contains(&offset)')
edit('src/engine.rs', '        || !s.closed.is_empty()\n', '        || !s.closed.is_empty()\n        || !s.reopened.is_empty()\n')
edit('src/profiles.rs', 'RecipeTarget { app, action: recipe.clone() }', 'RecipeTarget { app: app.clone(), action: recipe.clone() }')
edit('src/profiles.rs', 'previous.action != recipe', 'previous.action != recipe || previous.app != app')
edit('src/profiles.rs', 'Choose one recipe action per executable before saving.', 'A profile cannot combine different file identities or actions at one executable path.')
edit('src/integrity.rs', '    if s.stage == Stage::Restored\n', '''    if s.stage == Stage::Restored && s.plan.actions.iter().any(|a|
        a.reopen.is_some()
        && s.closed.iter().any(|c| policy::same_process(&c.target, &a.target) && c.state == CloseState::ClosedGracefully)
        && !s.reopened.iter().any(|r| policy::same_process(&r.target, &a.target))) {
        return Err("A completed session is missing an approved reopening outcome.".into());
    }
    if s.stage == Stage::Restored
''')
p = Path('tests/product_workflows.rs')
p.write_text(p.read_text(encoding='utf-8') + '''
#[test]
fn apply_rejects_existing_reopen_evidence_before_saving_anything() {
    let mut s = session();
    s.reopened.push(ReopenRecord { target: id(10, "background"), state: ReopenState::Skipped, child: None, detail: "prior evidence".into() });
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
    s.reopened.push(ReopenRecord { target: id(10, "background"), state: ReopenState::Deferred, child: None, detail: "desktop unavailable".into() });
    assert!(integrity::validate_record(&s).is_ok());
}
''', encoding='utf-8', newline='\n')

p = Path('README.md')
s = p.read_text(encoding='utf-8')
a = s.index('4. Inspect the exact plan')
b = s.index('6. **Restore now**', a)
s = s[:a] + '''4. Before Start, optionally expand a selection with **Select same app**, save/load a game profile, or separately approve **Reopen after close...** for one main GUI process. These controls only edit the unsent preview.
5. Inspect the exact plan and choose **START GAME SESSION**. Lower-priority actions require an explicitly selected policy. GPU scheduling prompts for additional experimental consent; force termination has its own target-specific confirmation.
''' + s[b:]
s = s.replace('Changed executables and protected groups are omitted or rejected with an explanation.', 'Replaced executable file identities and protected groups are omitted or rejected with an explanation. Profiles bind file IDs, not cryptographic content hashes; in-place updates still require your fresh review.')
p.write_text(s, encoding='utf-8', newline='\n')
edit('specs/01-product.md', 'An updated game requires renewed review/save; missing, changed or protected target groups are reported instead of silently selected.', 'A changed game file ID requires renewed review/save; missing, replaced or protected target groups are reported instead of silently selected. A file ID is not a content hash: same-file in-place updates may retain the recipe match, never the prior consent.')
print('Applied static-check correction and final workflow-integrity review.')
