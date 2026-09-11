from pathlib import Path

path = Path('src/windows/simple_ui.rs')
text = path.read_text(encoding='utf-8')

def one(old: str, new: str, label: str):
    global text
    if text.count(old) != 1:
        raise RuntimeError(f'{label}: expected one exact anchor, got {text.count(old)}')
    text = text.replace(old, new, 1)

one(
    'use super::{gpu, manual as native, process, runner, startup, wide};',
    'use super::{gpu, manual as native, process, runner, startup, update, wide};',
    'Windows update module import',
)
one(
    'const PERMANENT_LABEL: u16 = 36;\nconst PANEL: [u16; 17] = [',
    'const PERMANENT_LABEL: u16 = 36;\nconst UPDATE: u16 = 37;\nconst PANEL: [u16; 18] = [',
    'update control id',
)
one(
    '    PERMANENT_LABEL,\n];',
    '    PERMANENT_LABEL,\n    UPDATE,\n];',
    'panel membership',
)
one(
    '''        ] {
            self.control(id, "BUTTON", label, WS_TABSTOP)?;
        }
        self.control(
            GPU_OPTION,''',
    '''        ] {
            self.control(id, "BUTTON", label, WS_TABSTOP)?;
        }
        self.control(
            UPDATE,
            "BUTTON",
            &format!("Check for updates — v{}", env!("CARGO_PKG_VERSION")),
            WS_TABSTOP,
        )?;
        self.control(
            GPU_OPTION,''',
    'update button creation',
)
one(
    '''                    ACK,
                    PERMANENT,''',
    '''                    ACK,
                    UPDATE,
                    PERMANENT,''',
    'settings update visibility',
)
one(
    '''                for (i, id) in [REPORT, ADVANCED, ACK].iter().enumerate() {
                    put(*id, 24 + i as i32 * (bw + 8), 630, bw, 42);
                }''',
    '''                let utility_w = (inner - 24) / 4;
                for (i, id) in [REPORT, ADVANCED, ACK, UPDATE].iter().enumerate() {
                    put(*id, 24 + i as i32 * (utility_w + 8), 630, utility_w, 42);
                }''',
    'update button layout',
)
one(
    '''            self.enable(ADVANCED, idle);
            self.refresh_selected(idle);''',
    '''            self.enable(ADVANCED, idle);
            self.enable(UPDATE, idle);
            self.refresh_selected(idle);''',
    'update enabled state',
)
one(
    '''            ADVANCED => runner::spawn_worker("--advanced", None)?,
            ACK => {''',
    '''            ADVANCED => runner::spawn_worker("--advanced", None)?,
            UPDATE => {
                if update::interactive(self.window)? {
                    unsafe { DestroyWindow(self.window); }
                }
            }
            ACK => {''',
    'update command',
)
one(
    '            if s.widgets.len() != 23 {',
    '            if s.widgets.len() != 24 {',
    'simple UI smoke control count',
)
path.write_text(text, encoding='utf-8', newline='\n')
print('Applied v0.5.0 in-app update button to Settings.')
