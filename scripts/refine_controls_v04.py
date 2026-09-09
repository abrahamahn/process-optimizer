from pathlib import Path
import re

p = Path('src/windows/simple_ui.rs')
text = p.read_text(encoding='utf-8')


def once(old: str, new: str, label: str) -> None:
    global text
    if old not in text:
        raise RuntimeError(f'{label}: expected source block not found')
    text = text.replace(old, new, 1)


once(
'''    } else {
        ("Optional app", "Review before changing")
    }
}

struct Choice {
    path: String,
    name: String,
    id: Option<Identity>,
    protected: bool,
    gpu: Option<f64>,
    startup_enabled: bool,
    startup_disabled: bool,
}
''',
'''    } else {
        ("Optional app", "Review before changing")
    }
}

fn essential_process(name: &str, reason: Option<&str>) -> bool {
    let n = name.to_ascii_lowercase();
    if matches!(
        n.as_str(),
        "system"
            | "registry"
            | "secure system"
            | "memory compression"
            | "smss.exe"
            | "csrss.exe"
            | "wininit.exe"
            | "services.exe"
            | "lsass.exe"
            | "winlogon.exe"
            | "svchost.exe"
            | "dwm.exe"
            | "audiodg.exe"
            | "fontdrvhost.exe"
            | "sihost.exe"
            | "ctfmon.exe"
            | "msmpeng.exe"
            | "mssense.exe"
            | "securityhealthservice.exe"
    ) {
        return true;
    }
    reason.is_some_and(|r| {
        let r = r.to_ascii_lowercase();
        r.contains("critical windows process") || r.contains("windows component: protected")
    })
}

fn recommendation(c: &Choice) -> &'static str {
    if c.essential {
        return "KEEP — REQUIRED";
    }
    if c.protected {
        return "KEEP — PROTECTED";
    }
    let n = c.name.to_ascii_lowercase();
    if n.contains("apogee")
        || n.contains("antelopeaudio")
        || n.starts_with("asus")
        || n.contains("nvbroadcast")
        || n == "msedgewebview2.exe"
        || n == "postgres.exe"
        || n == "pg_ctl.exe"
    {
        return "KEEP / REVIEW";
    }
    if c.startup_enabled
        && (matches!(
            n.as_str(),
            "applephotostreams.exe"
                | "apsdaemon.exe"
                | "mdnsresponder.exe"
                | "everything.exe"
                | "onedrive.exe"
                | "dropbox.exe"
                | "googledrivefs.exe"
        ) || n.contains("updater"))
    {
        return "DISABLE STARTUP IF UNUSED";
    }
    if matches!(
        n.as_str(),
        "chrome.exe"
            | "msedge.exe"
            | "firefox.exe"
            | "brave.exe"
            | "windowsterminal.exe"
            | "powershell.exe"
            | "cncmd.exe"
            | "code.exe"
            | "rtss.exe"
            | "rtsshooksloader64.exe"
            | "msiafterburner.exe"
            | "radeonsoftware.exe"
            | "everything.exe"
            | "onedrive.exe"
            | "dropbox.exe"
            | "googledrivefs.exe"
    ) || n.contains("nvidia overlay")
        || c.gpu.is_some_and(|v| v >= 1.0)
    {
        return "LOWER PRIORITY";
    }
    "KEEP / REVIEW"
}

struct Choice {
    path: String,
    name: String,
    id: Option<Identity>,
    protected: bool,
    essential: bool,
    gpu: Option<f64>,
    startup_enabled: bool,
    startup_disabled: bool,
}
''',
'classification helpers',
)

once(
'''            WS_TABSTOP | WS_VSCROLL | WS_HSCROLL | LBS_NOTIFY as u32 | LBS_NOINTEGRALHEIGHT as u32,
''',
'''            WS_TABSTOP
                | WS_VSCROLL
                | WS_HSCROLL
                | LBS_NOTIFY as u32
                | LBS_NOINTEGRALHEIGHT as u32
                | LBS_OWNERDRAWFIXED as u32
                | LBS_HASSTRINGS as u32,
''',
'owner-draw list style',
)

old_buttons = '''        for (id, label) in [
            (ALLOW_CLOSE, "Close during Game Mode"),
            (REDUCE, "Reduce during Game Mode"),
            (KEEP, "Keep / remove game rule"),
            (PERMANENT, "Disable Windows startup"),
            (RESTORE_STARTUP, "Restore Windows startup"),
            (COPY_LIST, "Copy / paste process list"),
            (REPORT, "Session details"),
            (ADVANCED, "Advanced tools"),
            (ACK, "Recovery options"),
        ] {
            self.control(id, "BUTTON", label, WS_TABSTOP)?;
        }
'''
new_buttons = '''        for (id, label) in [
            (KEEP, "Keep"),
            (REDUCE, "Lower priority"),
            (ALLOW_CLOSE, "Close"),
            (RESTORE_STARTUP, "Keep startup / restore"),
            (PERMANENT, "Disable startup"),
            (COPY_LIST, "Copy / paste process list"),
            (REPORT, "Session details"),
            (ADVANCED, "Advanced tools"),
            (ACK, "Recovery options"),
        ] {
            let style = match id {
                KEEP | RESTORE_STARTUP => {
                    WS_TABSTOP | WS_GROUP | BS_AUTORADIOBUTTON as u32
                }
                REDUCE | ALLOW_CLOSE | PERMANENT => WS_TABSTOP | BS_AUTORADIOBUTTON as u32,
                _ => WS_TABSTOP,
            };
            self.control(id, "BUTTON", label, style)?;
        }
'''
once(old_buttons, new_buttons, 'state selector buttons')

once(
'''            "GAME MODE ONLY — temporary actions when you press Turn ON",
''',
'''            "GAME MODE — choose one state for the selected app",
''',
'game label',
)
once(
'''            "PERMANENT CLEANUP — stop auto-starting with Windows (reversible)",
''',
'''            "WINDOWS STARTUP — Keep or Disable for the selected app",
''',
'startup label',
)
once(
'''        self.control(DETAIL,"STATIC","Game Mode only affects approved apps when you press ON. Permanent cleanup currently disables supported current-user startup entries and can be restored here. It does not uninstall apps or disable system services.",0)?;
''',
'''        self.control(DETAIL,"STATIC","Select an app to see its current state and recommendation. Keep removes a Game Mode rule. Turn OFF restores priority changes already applied by the current Game Mode session.",0)?;
''',
'detail intro',
)
once(
'''            "CHOOSE WHAT TO CLEAN\\r\\nGame Mode only = temporary. Permanent cleanup = stop supported auto-start entries.",
''',
'''            "APP CONTROL\\r\\nEach app has two independent choices: Game Mode behavior and Windows startup.",
''',
'intro copy',
)

# Put radio selectors in natural left-to-right state order.
once(
'''                for (i, id) in [ALLOW_CLOSE, REDUCE, KEEP].iter().enumerate() {
''',
'''                for (i, id) in [KEEP, REDUCE, ALLOW_CLOSE].iter().enumerate() {
''',
'game selector layout',
)
once(
'''                for (i, id) in [PERMANENT, RESTORE_STARTUP, COPY_LIST].iter().enumerate() {
''',
'''                for (i, id) in [RESTORE_STARTUP, PERMANENT, COPY_LIST].iter().enumerate() {
''',
'startup selector layout',
)

# Fixed owner-draw rows need enough height for the UI font.
once(
'''        unsafe {
            if !old.is_null() {
''',
'''        unsafe {
            let item_height = ((24 * dpi / 96).max(20)) as LPARAM;
            SendMessageW(self.h(APPS), LB_SETITEMHEIGHT, 0, item_height);
            if !old.is_null() {
''',
'list row height',
)

# Add state helpers and red owner drawing before control creation.
once(
'''    fn control(&mut self, id: u16, class: &str, label: &str, style: u32) -> AppResult<()> {
''',
'''    fn check(&self, id: u16, checked: bool) {
        unsafe {
            SendMessageW(
                self.h(id),
                BM_SETCHECK,
                if checked {
                    BST_CHECKED as usize
                } else {
                    BST_UNCHECKED as usize
                },
                0,
            );
        }
    }
    fn selected_index(&self) -> Option<usize> {
        let i = unsafe { SendMessageW(self.h(APPS), LB_GETCURSEL, 0, 0) };
        if i < 0 || self.choices.get(i as usize).is_none() {
            None
        } else {
            Some(i as usize)
        }
    }
    fn sync_selected_controls(&self, idle: bool) {
        let Some(i) = self.selected_index() else {
            for id in [KEEP, REDUCE, ALLOW_CLOSE, RESTORE_STARTUP, PERMANENT] {
                self.enable(id, false);
                self.check(id, false);
            }
            self.text(
                DETAIL,
                "Select an app. Game Mode and Windows startup are controlled independently.",
            );
            return;
        };
        let c = &self.choices[i];
        let rule = self.config.rules.iter().find(|r| r.app.path == c.path);
        let rule = rule.filter(|r| r.active(native::now()));
        self.check(KEEP, rule.is_none());
        self.check(
            REDUCE,
            rule.is_some_and(|r| r.action == RuleAction::ReduceLoad),
        );
        self.check(
            ALLOW_CLOSE,
            rule.is_some_and(|r| r.action == RuleAction::Close),
        );
        self.check(RESTORE_STARTUP, !c.startup_disabled);
        self.check(PERMANENT, c.startup_disabled);

        let can_game_change = idle && !c.protected && c.id.is_some();
        self.enable(KEEP, idle);
        self.enable(REDUCE, can_game_change);
        self.enable(ALLOW_CLOSE, can_game_change);
        self.enable(
            PERMANENT,
            idle
                && !c.protected
                && c.id.is_some()
                && c.startup_enabled
                && !c.startup_disabled,
        );
        self.enable(
            RESTORE_STARTUP,
            idle && (c.startup_enabled || c.startup_disabled),
        );

        let game = match rule.map(|r| &r.action) {
            Some(RuleAction::Close) => "Close",
            Some(RuleAction::ReduceLoad) => "Lower priority",
            None => "Keep",
        };
        let startup = if c.startup_disabled {
            "Disabled"
        } else if c.startup_enabled {
            "Keep / starts with Windows"
        } else {
            "No supported startup entry"
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
                "{} Current: Game Mode = {}; Windows startup = {}. Recommended: {}. Turn OFF restores any priority changes already applied.",
                safety,
                game,
                startup,
                recommendation(c)
            ),
        );
    }
    fn draw_app_item(&self, item: &DRAWITEMSTRUCT) -> bool {
        if item.CtlID != APPS as u32 || item.itemID == u32::MAX {
            return false;
        }
        unsafe {
            let selected = item.itemState & ODS_SELECTED as u32 != 0;
            let background = if selected {
                GetSysColorBrush(COLOR_HIGHLIGHT)
            } else {
                GetSysColorBrush(COLOR_WINDOW)
            };
            FillRect(item.hDC, &item.rcItem, background);
            let essential = self
                .choices
                .get(item.itemID as usize)
                .is_some_and(|c| c.essential);
            SetTextColor(
                item.hDC,
                if essential {
                    0x000000d0
                } else if selected {
                    GetSysColor(COLOR_HIGHLIGHTTEXT)
                } else {
                    GetSysColor(COLOR_WINDOWTEXT)
                },
            );
            SetBkMode(item.hDC, 1);
            let length = SendMessageW(
                self.h(APPS),
                LB_GETTEXTLEN,
                item.itemID as usize,
                0,
            );
            if length >= 0 {
                let mut buffer = vec![0u16; length as usize + 1];
                SendMessageW(
                    self.h(APPS),
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
                    length as i32,
                    &mut rect,
                    (DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | DT_END_ELLIPSIS) as u32,
                );
            }
            if item.itemState & ODS_FOCUS as u32 != 0 {
                DrawFocusRect(item.hDC, &item.rcItem);
            }
        }
        true
    }
    fn control(&mut self, id: u16, class: &str, label: &str, style: u32) -> AppResult<()> {
''',
'selection and owner-draw helpers',
)

old_populate = '''        for row in &self.snapshot.processes {
            if !policy::complete_identity(&row.identity) {
                continue;
            }
            let path = policy::normalized_path(&row.identity.path);
            let entry = choices.entry(path.clone()).or_insert_with(|| Choice {
                path,
                name: row.name.clone(),
                id: Some(row.identity.clone()),
                protected: false,
                gpu: None,
                startup_enabled: false,
                startup_disabled: false,
            });
            entry.protected |= row.protected_reason.is_some();
            if let Some(v) = busiest_engine(&row.gpu) {
                entry.gpu = Some(entry.gpu.unwrap_or(0.0).max(v));
            }
        }
'''
new_populate = '''        for row in &self.snapshot.processes {
            let complete = policy::complete_identity(&row.identity);
            let essential = essential_process(&row.name, row.protected_reason.as_deref());
            if !complete && !essential {
                continue;
            }
            let path = if complete {
                policy::normalized_path(&row.identity.path)
            } else {
                format!("protected://{}:{}", row.name.to_ascii_lowercase(), row.identity.pid)
            };
            let entry = choices.entry(path.clone()).or_insert_with(|| Choice {
                path,
                name: row.name.clone(),
                id: if complete {
                    Some(row.identity.clone())
                } else {
                    None
                },
                protected: false,
                essential,
                gpu: None,
                startup_enabled: false,
                startup_disabled: false,
            });
            entry.protected |= row.protected_reason.is_some() || essential;
            entry.essential |= essential;
            if let Some(v) = busiest_engine(&row.gpu) {
                entry.gpu = Some(entry.gpu.unwrap_or(0.0).max(v));
            }
        }
'''
once(old_populate, new_populate, 'populate protected rows')

# Add essential=false to the two config-only Choice initializers.
needle = '''                    id: None,
                    protected: false,
                    gpu: None,
'''
count = text.count(needle)
if count != 2:
    raise RuntimeError(f'config-only Choice initializers: expected 2, got {count}')
text = text.replace(
    needle,
    '''                    id: None,
                    protected: false,
                    essential: false,
                    gpu: None,
''',
)

old_filter = '''        self.choices = choices
            .into_values()
            .filter(|c| {
                !c.protected
                    || self.config.rules.iter().any(|r| r.app.path == c.path)
                    || self
                        .config
                        .startup_disabled
                        .iter()
                        .any(|b| b.app.path == c.path)
            })
            .collect();
        self.choices.sort_by_key(|c| c.name.to_lowercase());
'''
new_filter = '''        self.choices = choices.into_values().collect();
        self.choices
            .sort_by_key(|c| (!c.essential, c.name.to_lowercase()));
'''
once(old_filter, new_filter, 'keep protected rows visible')

render_pattern = re.compile(
    r'''        for c in &self\.choices \{.*?        \}\n        unsafe \{\n            SendMessageW\(self\.h\(APPS\), LB_SETHORIZONTALEXTENT, 1100, 0\);''',
    re.S,
)
new_render = '''        for c in &self.choices {
            let game = self
                .config
                .rules
                .iter()
                .find(|r| r.app.path == c.path)
                .map(|r| {
                    if !r.active(native::now()) {
                        "Keep (approval expired)"
                    } else {
                        match r.action {
                            RuleAction::Close => "Close",
                            RuleAction::ReduceLoad => "Lower priority",
                        }
                    }
                })
                .unwrap_or("Keep");
            let (category, hint) = if c.essential {
                ("Windows / core", "Required; do not change")
            } else if c.protected {
                ("Protected app", "Kept outside optimizer actions")
            } else {
                describe(&c.name)
            };
            let value = c.gpu.map(|v| format!(" | GPU {v:.1}%")).unwrap_or_default();
            let startup = if c.startup_disabled {
                "Disabled"
            } else if c.startup_enabled {
                "Keep"
            } else {
                "N/A"
            };
            let safety = if c.essential {
                "ESSENTIAL — KEEP"
            } else if c.protected {
                "PROTECTED — KEEP"
            } else {
                "OPTIONAL"
            };
            let line = format!(
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
            SendMessageW(self.h(APPS), LB_SETHORIZONTALEXTENT, 1500, 0);'''
text, count = render_pattern.subn(new_render, text, count=1)
if count != 1:
    raise RuntimeError(f'app row renderer: expected 1 replacement, got {count}')

once(
'''            InvalidateRect(self.h(APPS), null(), 1);
        }
        self.text(
''',
'''            InvalidateRect(self.h(APPS), null(), 1);
            if !self.choices.is_empty() {
                SendMessageW(self.h(APPS), LB_SETCURSEL, 0, 0);
            }
        }
        self.text(
''',
'default app selection',
)
once(
'''            "CHOOSE WHAT TO CLEAN\\r\\nSelect an app, then choose Game Mode only or Permanent cleanup below.",
''',
'''            "APP CONTROL\\r\\nSelect an app. Choose its Game Mode state and Windows startup state independently.",
''',
'populated intro',
)

# Keep startup is the explicit opposite of Disable startup; if already disabled it restores.
once(
'''    fn restore_startup(&mut self) -> AppResult<()> {
''',
'''    fn keep_startup(&mut self) -> AppResult<()> {
        let choice = self.selected()?;
        if choice.startup_disabled {
            return self.restore_startup();
        }
        if choice.startup_enabled {
            info(
                self.window,
                "Windows startup is kept for this app. Process Optimizer will not change it unless you explicitly choose Disable startup later.",
            );
        } else {
            info(
                self.window,
                "No supported current-user Windows startup entry exists for this app, so there is nothing persistent to disable. Game Mode choices remain independent.",
            );
        }
        Ok(())
    }

    fn restore_startup(&mut self) -> AppResult<()> {
''',
'keep startup action',
)

# Selection buttons are synchronized from persisted state; only Advanced is globally gated here.
old_enable = '''            for id in [
                ALLOW_CLOSE,
                REDUCE,
                KEEP,
                PERMANENT,
                RESTORE_STARTUP,
                ADVANCED,
            ] {
                self.enable(id, idle);
            }
            self.enable(SCAN, !self.scanning);
'''
new_enable = '''            self.enable(ADVANCED, idle);
            self.sync_selected_controls(idle);
            self.enable(SCAN, !self.scanning);
'''
once(old_enable, new_enable, 'state-aware button enablement')

once(
'''            RESTORE_STARTUP => self.restore_startup()?,
''',
'''            RESTORE_STARTUP => self.keep_startup()?,
''',
'keep startup command',
)

# Owner draw and list selection notification.
old_command = '''                WM_COMMAND => {
                    if let Err(e) = s.command((w & 0xffff) as u16) {
                        info(window, &e);
                    }
                    return 0;
                }
'''
new_command = '''                WM_DRAWITEM => {
                    let item = &*(l as *const DRAWITEMSTRUCT);
                    if s.draw_app_item(item) {
                        return 1;
                    }
                }
                WM_COMMAND => {
                    let id = (w & 0xffff) as u16;
                    if id == APPS {
                        let idle = runner::database()
                            .and_then(|db| db.active())
                            .map(|active| active.is_none())
                            .unwrap_or(false);
                        s.sync_selected_controls(idle);
                        return 0;
                    }
                    if let Err(e) = s.command(id) {
                        info(window, &e);
                    }
                    return 0;
                }
'''
once(old_command, new_command, 'owner draw event handling')

# Smoke gate verifies the list remains native owner-drawn and the settings/default split stays intact.
once(
'''            if s.widgets.len() != 23 {
                return Err("Incomplete simple UI.".into());
            }
''',
'''            if s.widgets.len() != 23 {
                return Err("Incomplete simple UI.".into());
            }
            if GetWindowLongW(s.h(APPS), GWL_STYLE) as u32 & LBS_OWNERDRAWFIXED as u32 == 0 {
                return Err("App list lost essential-process color support.".into());
            }
''',
'owner draw smoke gate',
)

# Pure recommendation tests compile and run on the Windows CI target.
once(
'''pub fn run() -> AppResult<()> {
''',
'''#[cfg(test)]
mod recommendation_tests {
    use super::*;

    fn choice(name: &str, protected: bool, essential: bool, startup: bool, gpu: Option<f64>) -> Choice {
        Choice {
            path: format!(r"c:\\apps\\{name}"),
            name: name.into(),
            id: None,
            protected,
            essential,
            gpu,
            startup_enabled: startup,
            startup_disabled: false,
        }
    }

    #[test]
    fn essential_and_protected_are_always_keep_recommendations() {
        assert_eq!(
            recommendation(&choice("svchost.exe", true, true, false, Some(20.0))),
            "KEEP — REQUIRED"
        );
        assert_eq!(
            recommendation(&choice("steam.exe", true, false, false, Some(20.0))),
            "KEEP — PROTECTED"
        );
    }

    #[test]
    fn browser_and_gpu_active_optional_apps_prefer_lower_priority() {
        assert_eq!(
            recommendation(&choice("chrome.exe", false, false, false, None)),
            "LOWER PRIORITY"
        );
        assert_eq!(
            recommendation(&choice("optional.exe", false, false, false, Some(2.0))),
            "LOWER PRIORITY"
        );
    }

    #[test]
    fn unused_sync_style_startup_apps_get_persistent_cleanup_hint() {
        assert_eq!(
            recommendation(&choice("Everything.exe", false, false, true, None)),
            "DISABLE STARTUP IF UNUSED"
        );
    }
}

pub fn run() -> AppResult<()> {
''',
'recommendation tests',
)

p.write_text(text, encoding='utf-8', newline='\n')
print('Refined stateful app controls, essential labels, and recommendations.')
