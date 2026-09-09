from pathlib import Path

p = Path('src/windows/simple_ui.rs')
text = p.read_text(encoding='utf-8')


def once(old: str, new: str, label: str) -> None:
    global text
    if old not in text:
        raise RuntimeError(f'{label}: expected source block not found')
    text = text.replace(old, new, 1)

# Preserve the user's selected application when a state change repopulates the list.
once(
'''    fn populate(&mut self) {
        let startup_entries = startup::entries().unwrap_or_default();
''',
'''    fn populate(&mut self) {
        let selected_path = self
            .selected_index()
            .and_then(|i| self.choices.get(i))
            .map(|c| c.path.clone());
        let startup_entries = startup::entries().unwrap_or_default();
''',
'capture selection',
)

# Keep the list scannable. Put explanatory hints in the selected-app detail instead.
once(
'''            let (category, hint) = if c.essential {
                ("Windows / core", "Required; do not change")
            } else if c.protected {
                ("Protected app", "Kept outside optimizer actions")
            } else {
                describe(&c.name)
            };
''',
'''            let category = if c.essential {
                "Windows / core"
            } else if c.protected {
                "Protected app"
            } else {
                describe(&c.name).0
            };
''',
'short list category',
)
once(
'''            let line = format!(
                "{} | {} — {} | Game: {} | Startup: {} | RECOMMENDED: {}{} | {}{}",
                safety,
                c.name,
                category,
                game,
                startup,
                recommendation(c),
                if c.id.is_none() { " | not running" } else { "" },
                hint,
                value
            );
''',
'''            let line = format!(
                "{} | {} — {} | Game: {} | Startup: {} | RECOMMENDED: {}{}{}",
                safety,
                c.name,
                category,
                game,
                startup,
                recommendation(c),
                if c.id.is_none() { " | Not running" } else { "" },
                value
            );
''',
'short list row',
)
once(
'''            SendMessageW(self.h(APPS), LB_SETHORIZONTALEXTENT, 1500, 0);
            SendMessageW(self.h(APPS), WM_SETREDRAW, 1, 0);
            InvalidateRect(self.h(APPS), null(), 1);
            if !self.choices.is_empty() {
                SendMessageW(self.h(APPS), LB_SETCURSEL, 0, 0);
            }
''',
'''            SendMessageW(self.h(APPS), LB_SETHORIZONTALEXTENT, 1200, 0);
            SendMessageW(self.h(APPS), WM_SETREDRAW, 1, 0);
            InvalidateRect(self.h(APPS), null(), 1);
            if !self.choices.is_empty() {
                let selected = selected_path
                    .as_ref()
                    .and_then(|path| self.choices.iter().position(|c| &c.path == path))
                    .unwrap_or(0);
                SendMessageW(self.h(APPS), LB_SETCURSEL, selected, 0);
            }
''',
'restore selection',
)

# Show category guidance in the detail area, where it does not make every list row noisy.
once(
'''        let safety = if c.essential {
            "ESSENTIAL — KEEP. This Windows/core process is never an optimization target."
        } else if c.protected {
            "PROTECTED — KEEP. Process Optimizer will not change this app."
        } else {
            "Optional app."
        };
        self.text(
            DETAIL,
            &format!(
                "{} Current: Game Mode = {}; Windows startup = {}. Recommended: {}. Turn OFF restores any priority changes already applied.",
                safety,
                game,
                startup,
                recommendation(c)
            ),
        );
''',
'''        let (category, guidance) = if c.essential {
            ("Windows / core", "Required; do not change")
        } else if c.protected {
            ("Protected app", "Kept outside optimizer actions")
        } else {
            describe(&c.name)
        };
        let safety = if c.essential {
            "ESSENTIAL — KEEP. This Windows/core process is never an optimization target."
        } else if c.protected {
            "PROTECTED — KEEP. Process Optimizer will not change this app."
        } else {
            "Optional app."
        };
        self.text(
            DETAIL,
            &format!(
                "{} {}: {}. Current: Game Mode = {}; Startup = {}. Recommended: {}. Turn OFF restores priority changes already applied.",
                safety,
                category,
                guidance,
                game,
                startup,
                recommendation(c)
            ),
        );
''',
'detail guidance',
)

# Auto-radio buttons change visually before the confirmation callback. Re-sync after every
# selector command so Cancel/validation errors immediately return to the persisted state.
once(
'''                    if let Err(e) = s.command(id) {
                        info(window, &e);
                    }
                    return 0;
''',
'''                    let result = s.command(id);
                    if matches!(id, KEEP | REDUCE | ALLOW_CLOSE | RESTORE_STARTUP | PERMANENT) {
                        let idle = runner::database()
                            .and_then(|db| db.active())
                            .map(|active| active.is_none())
                            .unwrap_or(false);
                        s.sync_selected_controls(idle);
                    }
                    if let Err(e) = result {
                        info(window, &e);
                    }
                    return 0;
''',
'resync radio state',
)

p.write_text(text, encoding='utf-8', newline='\n')
print('Applied final app-control UX polish.')
