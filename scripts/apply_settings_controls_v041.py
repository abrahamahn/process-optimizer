from pathlib import Path
import re

path = Path('src/windows/simple_ui.rs')
text = path.read_text(encoding='utf-8')


def replace_once(old: str, new: str, label: str) -> None:
    global text
    if old not in text:
        raise RuntimeError(f'missing patch anchor: {label}')
    text = text.replace(old, new, 1)

replace_once(
    'const ES_READONLY_STYLE: u32 = 0x0800;\ntype Slot<T> = Arc<Mutex<Option<AppResult<T>>>>;',
    'const ES_READONLY_STYLE: u32 = 0x0800;\nconst LBS_OWNERDRAWFIXED_STYLE: u32 = 0x0010;\ntype Slot<T> = Arc<Mutex<Option<AppResult<T>>>>;',
    'ownerdraw constant',
)

replace_once(
    '            WS_TABSTOP | WS_VSCROLL | WS_HSCROLL | LBS_NOTIFY as u32 | LBS_NOINTEGRALHEIGHT as u32,',
    '            WS_TABSTOP | WS_VSCROLL | WS_HSCROLL | LBS_NOTIFY as u32 | LBS_NOINTEGRALHEIGHT as u32 | LBS_OWNERDRAWFIXED_STYLE,',
    'apps ownerdraw style',
)

replace_once(
    'struct Choice {\n    path: String,\n    name: String,\n    id: Option<Identity>,\n    protected: bool,\n    gpu: Option<f64>,\n    startup_enabled: bool,\n    startup_disabled: bool,\n}\n',
    '''struct Choice {
    path: String,
    name: String,
    id: Option<Identity>,
    protected: bool,
    protected_reason: Option<String>,
    gpu: Option<f64>,
    startup_enabled: bool,
    startup_disabled: bool,
}

fn recommendation(choice: &Choice) -> &'static str {
    if choice.protected {
        return "KEEP";
    }
    let n = choice.name.to_ascii_lowercase();
    if n.contains("apogee")
        || n.contains("antelopeaudio")
        || n.contains("nvbroadcast")
        || n == "msedgewebview2.exe"
        || n == "postgres.exe"
        || n == "pg_ctl.exe"
        || n.starts_with("asus")
    {
        "KEEP"
    } else if matches!(
        n.as_str(),
        "applephotostreams.exe" | "apsdaemon.exe" | "mdnsresponder.exe" | "everything.exe"
    ) && choice.startup_enabled
    {
        "DISABLE STARTUP"
    } else if n.contains("nvidia overlay")
        || matches!(
            n.as_str(),
            "rtss.exe" | "rtsshooksloader64.exe" | "msiafterburner.exe" | "radeonsoftware.exe"
        )
    {
        "CLOSE DURING GAME MODE"
    } else if matches!(
        n.as_str(),
        "chrome.exe"
            | "msedge.exe"
            | "firefox.exe"
            | "brave.exe"
            | "windowsterminal.exe"
            | "powershell.exe"
            | "cncmd.exe"
            | "code.exe"
    ) || choice.gpu.unwrap_or(0.0) >= 2.0
    {
        "LOWER DURING GAME MODE"
    } else {
        "KEEP"
    }
}
''',
    'choice model',
)

text = text.replace(
    '                protected: false,\n                gpu: None,',
    '                protected: false,\n                protected_reason: None,\n                gpu: None,',
)
if text.count('protected_reason: None') < 3:
    raise RuntimeError('not all choice constructors were patched')

replace_once(
    '            entry.protected |= row.protected_reason.is_some();\n            if let Some(v) = busiest_engine(&row.gpu) {',
    '''            entry.protected |= row.protected_reason.is_some();
            if entry.protected_reason.is_none() {
                entry.protected_reason = row.protected_reason.clone();
            }
            if let Some(v) = busiest_engine(&row.gpu) {''',
    'protection reason',
)

pattern = re.compile(r'''        self\.choices = choices\n            \.into_values\(\)\n            \.filter\(\|c\| \{.*?\n            \}\)\n            \.collect\(\);\n        self\.choices\.sort_by_key\(\|c\| c\.name\.to_lowercase\(\)\);''', re.S)
text, count = pattern.subn(
    '''        self.choices = choices.into_values().collect();
        self.choices
            .sort_by_key(|c| (!c.protected, c.name.to_lowercase()));''',
    text,
    count=1,
)
if count != 1:
    raise RuntimeError('protected-choice visibility block not found')

pattern = re.compile(r'''        for c in &self\.choices \{\n            let status = self.*?\n            \}\n        unsafe \{\n            SendMessageW\(self\.h\(APPS\), LB_SETHORIZONTALEXTENT, 1100, 0\);''', re.S)
replacement = '''        for c in &self.choices {
            let game = self
                .config
                .rules
                .iter()
                .find(|r| r.app.path == c.path)
                .map(|r| {
                    if !r.active(native::now()) {
                        "EXPIRED"
                    } else {
                        match r.action {
                            RuleAction::Close => "CLOSE",
                            RuleAction::ReduceLoad => "LOWER",
                        }
                    }
                })
                .unwrap_or("KEEP");
            let (category, hint) = describe(&c.name);
            let value = c.gpu.map(|v| format!(" | GPU {v:.1}%")).unwrap_or_default();
            let startup = if c.startup_disabled {
                "DISABLED"
            } else if c.startup_enabled {
                "ON"
            } else {
                "N/A"
            };
            let safety = if c.protected {
                "ESSENTIAL — DO NOT CHANGE"
            } else {
                "OPTIONAL"
            };
            let line = format!(
                "[{safety}] {} — {} | Current: Game {game}, Startup {startup} | Recommended: {} | {}{}{}",
                c.name,
                category,
                recommendation(c),
                hint,
                if c.id.is_none() { " | not running" } else { "" },
                value
            );
            unsafe {
                SendMessageW(
                    self.h(APPS),
                    LB_ADDSTRING,
                    0,
                    wide(&line).as_ptr() as LPARAM,
                );
            }
        }
        unsafe {
            SendMessageW(self.h(APPS), LB_SETHORIZONTALEXTENT, 1450, 0);'''
text, count = pattern.subn(replacement, text, count=1)
if count != 1:
    raise RuntimeError('list rendering block not found')

replace_once(
    '''        self.text(
            INTRO,
            "CHOOSE WHAT TO CLEAN\\r\\nSelect an app, then choose Game Mode only or Permanent cleanup below.",
        );
    }
    fn selected(&self) -> AppResult<&Choice> {''',
    '''        if !self.choices.is_empty() {
            unsafe {
                SendMessageW(self.h(APPS), LB_SETCURSEL, 0, 0);
            }
        }
        self.text(
            INTRO,
            "CHOOSE WHAT TO CLEAN\\r\\nRed ESSENTIAL rows are protected. Select an OPTIONAL app to choose Keep, Lower, Close, or startup cleanup.",
        );
        let idle = runner::database()
            .and_then(|db| db.active())
            .map(|active| active.is_none())
            .unwrap_or(false);
        self.refresh_selected(idle);
    }
    fn selected(&self) -> AppResult<&Choice> {''',
    'populate selection',
)

insert_anchor = '    fn permit(&mut self, action: Option<RuleAction>) -> AppResult<()> {'
if insert_anchor not in text:
    raise RuntimeError('permit anchor missing')
methods = r'''    fn refresh_selected(&self, idle: bool) {
        let index = unsafe { SendMessageW(self.h(APPS), LB_GETCURSEL, 0, 0) };
        let Some(choice) = (index >= 0)
            .then(|| self.choices.get(index as usize))
            .flatten()
        else {
            for id in [ALLOW_CLOSE, REDUCE, KEEP, PERMANENT, RESTORE_STARTUP] {
                self.enable(id, false);
            }
            self.text(DETAIL, "Select an app. Red ESSENTIAL rows are informational only and cannot be changed.");
            return;
        };
        let rule = self.config.rules.iter().find(|r| r.app.path == choice.path);
        let has_rule = rule.is_some();
        let current_game = match rule.map(|r| &r.action) {
            Some(RuleAction::Close) => "Close",
            Some(RuleAction::ReduceLoad) => "Lower",
            None => "Keep",
        };
        let current_startup = if choice.startup_disabled {
            "Disabled by Process Optimizer"
        } else if choice.startup_enabled {
            "Starts with Windows"
        } else {
            "No supported startup entry"
        };
        self.text(
            ALLOW_CLOSE,
            if matches!(rule.map(|r| &r.action), Some(RuleAction::Close)) {
                "Close during Game Mode  [CURRENT]"
            } else {
                "Close during Game Mode"
            },
        );
        self.text(
            REDUCE,
            if matches!(rule.map(|r| &r.action), Some(RuleAction::ReduceLoad)) {
                "Lower during Game Mode  [CURRENT]"
            } else {
                "Lower during Game Mode"
            },
        );
        self.text(
            KEEP,
            if !has_rule {
                "Keep  [CURRENT]"
            } else {
                "Keep / undo Game Mode rule"
            },
        );
        self.text(
            PERMANENT,
            if choice.startup_disabled {
                "Windows startup disabled  [CURRENT]"
            } else {
                "Disable Windows startup"
            },
        );
        self.text(RESTORE_STARTUP, "Restore Windows startup");

        self.enable(ALLOW_CLOSE, idle && !choice.protected && choice.id.is_some());
        self.enable(REDUCE, idle && !choice.protected && choice.id.is_some());
        // Removing a stale/unsafe rule is always allowed while idle, even if the app is now protected.
        self.enable(KEEP, idle && has_rule);
        self.enable(
            PERMANENT,
            idle && !choice.protected && choice.startup_enabled && !choice.startup_disabled,
        );
        // Restoration is a safety operation and remains available for a previously disabled item.
        self.enable(RESTORE_STARTUP, idle && choice.startup_disabled);

        let (category, hint) = describe(&choice.name);
        let safety = if choice.protected {
            format!(
                "SAFETY: ESSENTIAL — DO NOT CHANGE{}",
                choice
                    .protected_reason
                    .as_deref()
                    .map(|r| format!(" ({r})"))
                    .unwrap_or_default()
            )
        } else {
            "SAFETY: Optional — user-controlled".into()
        };
        self.text(
            DETAIL,
            &format!(
                "{} | Current: Game Mode = {}; Windows startup = {}. Recommended: {}. {} — {}",
                safety,
                current_game,
                current_startup,
                recommendation(choice),
                category,
                hint
            ),
        );
    }

    fn draw_app_item(&self, item: &DRAWITEMSTRUCT) -> LRESULT {
        if item.CtlID != APPS as u32 || item.itemID == u32::MAX {
            return 0;
        }
        let index = item.itemID as usize;
        let selected = item.itemState & ODS_SELECTED as u32 != 0;
        unsafe {
            FillRect(
                item.hDC,
                &item.rcItem,
                GetSysColorBrush(if selected { COLOR_HIGHLIGHT } else { COLOR_WINDOW }),
            );
            SetBkMode(item.hDC, TRANSPARENT as i32);
            let protected = self.choices.get(index).is_some_and(|c| c.protected);
            let color = if protected {
                0x000000D8 // COLORREF: strong red, intentionally retained even when selected.
            } else if selected {
                GetSysColor(COLOR_HIGHLIGHTTEXT)
            } else {
                GetSysColor(COLOR_WINDOWTEXT)
            };
            SetTextColor(item.hDC, color);
            let len = SendMessageW(item.hwndItem, LB_GETTEXTLEN, item.itemID as usize, 0);
            if len >= 0 {
                let mut buffer = vec![0u16; len as usize + 1];
                SendMessageW(
                    item.hwndItem,
                    LB_GETTEXT,
                    item.itemID as usize,
                    buffer.as_mut_ptr() as LPARAM,
                );
                let mut rect = item.rcItem;
                rect.left += 6;
                rect.right -= 6;
                DrawTextW(
                    item.hDC,
                    buffer.as_mut_ptr(),
                    len as i32,
                    &mut rect,
                    DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | DT_END_ELLIPSIS,
                );
            }
            if item.itemState & ODS_FOCUS as u32 != 0 {
                DrawFocusRect(item.hDC, &item.rcItem);
            }
        }
        1
    }

'''
text = text.replace(insert_anchor, methods + insert_anchor, 1)

replace_once(
    '''            for id in [
                ALLOW_CLOSE,
                REDUCE,
                KEEP,
                PERMANENT,
                RESTORE_STARTUP,
                ADVANCED,
            ] {
                self.enable(id, idle);
            }
            self.enable(SCAN, !self.scanning);''',
    '''            self.enable(ADVANCED, idle);
            self.refresh_selected(idle);
            self.enable(SCAN, !self.scanning);''',
    'poll control state',
)

replace_once(
    '            SCAN => self.scan(),\n            ALLOW_CLOSE => self.permit(Some(RuleAction::Close))?,',
    '            SCAN => self.scan(),\n            APPS => {\n                let idle = runner::database()?.active()?.is_none();\n                self.refresh_selected(idle);\n            }\n            ALLOW_CLOSE => self.permit(Some(RuleAction::Close))?,',
    'apps selection command',
)

replace_once(
    '''                WM_COMMAND => {
                    if let Err(e) = s.command((w & 0xffff) as u16) {
                        info(window, &e);
                    }
                    return 0;
                }
                WM_SIZE => {''',
    '''                WM_COMMAND => {
                    if let Err(e) = s.command((w & 0xffff) as u16) {
                        info(window, &e);
                    }
                    return 0;
                }
                WM_DRAWITEM => {
                    let item = &*(l as *const DRAWITEMSTRUCT);
                    if item.CtlID == APPS as u32 {
                        return s.draw_app_item(item);
                    }
                }
                WM_SIZE => {''',
    'ownerdraw event',
)

replace_once(
    '''        unsafe {
            if !old.is_null() {
                DeleteObject(old);
            }
            if !old_heading.is_null() {
                DeleteObject(old_heading);
            }
        }
    }
    fn visibility(&self) {''',
    '''        unsafe {
            SendMessageW(
                self.h(APPS),
                LB_SETITEMHEIGHT,
                0,
                (28 * dpi / 96) as LPARAM,
            );
            if !old.is_null() {
                DeleteObject(old);
            }
            if !old_heading.is_null() {
                DeleteObject(old_heading);
            }
        }
    }
    fn visibility(&self) {''',
    'list row height',
)

# Preserve the explicit safety semantics in the intro/copy wording.
text = text.replace(
    '"GAME MODE ONLY — temporary actions when you press Turn ON"',
    '"GAME MODE ONLY — choose Keep / Lower / Close per optional app"',
    1,
)
text = text.replace(
    '"PERMANENT CLEANUP — stop auto-starting with Windows (reversible)"',
    '"WINDOWS STARTUP — Disable / Restore per supported optional app (reversible)"',
    1,
)

path.write_text(text, encoding='utf-8', newline='\n')
print('settings controls v0.4.1 patch applied')
