from pathlib import Path

p = Path('src/windows/simple_ui.rs')
text = p.read_text(encoding='utf-8')


def once(old: str, new: str, label: str) -> None:
    global text
    if old not in text:
        raise RuntimeError(f'{label}: expected source block not found')
    text = text.replace(old, new, 1)

once(
'''            (KEEP, "Keep"),
''',
'''            (KEEP, "Keep / exclude"),
''',
'game keep label',
)

once(
'''        let c = &self.choices[i];
        let rule = self.config.rules.iter().find(|r| r.app.path == c.path);
        let rule = rule.filter(|r| r.active(native::now()));
''',
'''        let c = &self.choices[i];
        let configured_rule = self.config.rules.iter().find(|r| r.app.path == c.path);
        let rule = configured_rule.filter(|r| r.active(native::now()));
        self.text(
            RESTORE_STARTUP,
            if c.startup_disabled {
                "Restore startup"
            } else {
                "Keep startup"
            },
        );
''',
'configured rule and dynamic startup label',
)

once(
'''        self.enable(KEEP, idle);
''',
'''        // A protected/essential app is already a mandatory Keep. Only leave this
        // control active if it can remove an older saved rule that is now blocked.
        self.enable(KEEP, idle && (!c.protected || configured_rule.is_some()));
''',
'protected keep lock',
)

p.write_text(text, encoding='utf-8', newline='\n')
print('Final selector clarity patch applied.')
