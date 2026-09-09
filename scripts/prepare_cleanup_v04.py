"""Temporary exact-match preparation for the v0.4 cleanup UI.
The script edits source/docs only on the validation branch and is removed before main.
"""
from pathlib import Path


def edit(path: str, old: str, new: str, count: int = 1) -> None:
    p = Path(path)
    text = p.read_text(encoding="utf-8")
    actual = text.count(old)
    if actual != count:
        raise RuntimeError(f"{path}: expected {count} matches, found {actual}: {old[:100]!r}")
    p.write_text(text.replace(old, new, count), encoding="utf-8", newline="\n")

# Upgrade old v0.3 manual settings in memory; persistent cleanup is a new independent field.
edit(
    "src/journal.rs",
    """        let config = match body {
            Some(body) => {
                if body.len() > 256 * 1024 {
                    return Err(\"Game Mode settings are too large.\".into());
                }
                serde_json::from_str(&body)
                    .map_err(|e| format!(\"Invalid Game Mode settings: {e}\"))?
            }
            None => crate::manual::Config::default(),
        };
        crate::manual::validate(&config)?;
        Ok(config)
""",
    """        let mut config: crate::manual::Config = match body {
            Some(body) => {
                if body.len() > 256 * 1024 {
                    return Err(\"Game Mode settings are too large.\".into());
                }
                serde_json::from_str(&body)
                    .map_err(|e| format!(\"Invalid Game Mode settings: {e}\"))?
            }
            None => crate::manual::Config::default(),
        };
        if config.schema == 1 {
            config.schema = crate::manual::CONFIG_SCHEMA;
        }
        crate::manual::validate(&config)?;
        Ok(config)
""",
)
edit(
    "src/journal.rs",
    """    pub fn save_manual_settings(&self, config: &crate::manual::Config) -> AppResult<()> {
        crate::manual::validate(config)?;
        let body = serde_json::to_string(config).map_err(|e| e.to_string())?;
""",
    """    pub fn save_manual_settings(&self, config: &crate::manual::Config) -> AppResult<()> {
        let mut normalized = config.clone();
        normalized.schema = crate::manual::CONFIG_SCHEMA;
        crate::manual::validate(&normalized)?;
        let body = serde_json::to_string(&normalized).map_err(|e| e.to_string())?;
""",
)

# Friendlier Settings, copyable process report, and distinct temporary/persistent actions.
edit(
    "src/windows/simple_ui.rs",
    "use super::{gpu, manual as native, process, runner, wide};",
    "use super::{gpu, manual as native, process, runner, startup, wide};",
)
edit(
    "src/windows/simple_ui.rs",
    "    gpu::busiest_engine,",
    "    gpu::{busiest_engine, dedicated_allocations},",
)
edit(
    "src/windows/simple_ui.rs",
    """const ACK: u16 = 30;
const PANEL: [u16; 11] = [
    APPS,
    INTRO,
    SCAN,
    ALLOW_CLOSE,
    REDUCE,
    KEEP,
    GPU_OPTION,
    DETAIL,
    REPORT,
    ADVANCED,
    ACK,
];
""",
    """const ACK: u16 = 30;
const PERMANENT: u16 = 31;
const RESTORE_STARTUP: u16 = 32;
const COPY_LIST: u16 = 33;
const COPY_TEXT: u16 = 34;
const GAME_LABEL: u16 = 35;
const PERMANENT_LABEL: u16 = 36;
const PANEL: [u16; 17] = [
    APPS, INTRO, SCAN, ALLOW_CLOSE, REDUCE, KEEP, GPU_OPTION, DETAIL, REPORT, ADVANCED, ACK,
    PERMANENT, RESTORE_STARTUP, COPY_LIST, COPY_TEXT, GAME_LABEL, PERMANENT_LABEL,
];
const ES_MULTILINE_STYLE: u32 = 0x0004;
const ES_AUTOVSCROLL_STYLE: u32 = 0x0040;
const ES_AUTOHSCROLL_STYLE: u32 = 0x0080;
const ES_READONLY_STYLE: u32 = 0x0800;
""",
)
edit(
    "src/windows/simple_ui.rs",
    "type Slot<T> = Arc<Mutex<Option<AppResult<T>>>>;\n\nstruct Choice {",
    """type Slot<T> = Arc<Mutex<Option<AppResult<T>>>>;

fn describe(name: &str) -> (&'static str, &'static str) {
    let n = name.to_ascii_lowercase();
    if matches!(n.as_str(), "chrome.exe" | "msedge.exe" | "firefox.exe" | "brave.exe") {
        ("Web browser", "Usually a Game Mode-only choice")
    } else if n.contains("nvidia overlay") || matches!(n.as_str(), "rtss.exe" | "rtsshooksloader64.exe" | "msiafterburner.exe" | "radeonsoftware.exe") {
        ("Overlay / monitoring", "Close for gaming only if you do not use its overlay or capture")
    } else if matches!(n.as_str(), "windowsterminal.exe" | "powershell.exe" | "cncmd.exe" | "code.exe") {
        ("Developer tool", "Usually a Game Mode-only choice")
    } else if matches!(n.as_str(), "applephotostreams.exe" | "apsdaemon.exe" | "mdnsresponder.exe") {
        ("Apple sync / discovery", "Permanent startup cleanup can make sense if unused")
    } else if n.contains("apogee") || n.contains("antelopeaudio") {
        ("Audio hardware software", "Keep if you use that audio device")
    } else if n.starts_with("asus") {
        ("ASUS utility", "Review carefully; device hotkeys or power features may depend on it")
    } else if n.contains("nvbroadcast") {
        ("NVIDIA Broadcast", "Keep if you use microphone or camera effects")
    } else if n == "everything.exe" {
        ("File search utility", "Game Mode or startup cleanup if you do not need it")
    } else if n == "msedgewebview2.exe" {
        ("App web component", "Keep unless you understand which parent app owns it")
    } else if n == "postgres.exe" || n == "pg_ctl.exe" {
        ("Developer database", "Use a proper database stop; do not force-close it")
    } else {
        ("Optional app", "Review before changing")
    }
}

struct Choice {""",
)
edit(
    "src/windows/simple_ui.rs",
    """    protected: bool,
    gpu: Option<f64>,
}
""",
    """    protected: bool,
    gpu: Option<f64>,
    startup_enabled: bool,
    startup_disabled: bool,
}
""",
)
edit(
    "src/windows/simple_ui.rs",
    """    expanded: bool,
    minimized: bool,
""",
    """    expanded: bool,
    copy_mode: bool,
    minimized: bool,
""",
)
edit(
    "src/windows/simple_ui.rs",
    """            expanded: false,
            minimized: false,
""",
    """            expanded: false,
            copy_mode: false,
            minimized: false,
""",
)
edit(
    "src/windows/simple_ui.rs",
    """                if class == \"LISTBOX\" {
                    WS_EX_CLIENTEDGE
                } else {
""",
    """                if class == \"LISTBOX\" || class == \"EDIT\" {
                    WS_EX_CLIENTEDGE
                } else {
""",
)
edit(
    "src/windows/simple_ui.rs",
    """            \"BACKGROUND APPS\\r\\nOnly apps you approve are changed. No game files to choose.\",
""",
    """            \"CHOOSE WHAT TO CLEAN\\r\\nGame Mode only = temporary. Permanent cleanup = stop supported auto-start entries.\",
""",
)
edit(
    "src/windows/simple_ui.rs",
    """        for (id, label) in [
            (ALLOW_CLOSE, \"Allow normal close\"),
            (REDUCE, \"Reduce background load\"),
            (KEEP, \"Keep / remove approval\"),
            (REPORT, \"Session details\"),
            (ADVANCED, \"Advanced tools\"),
            (ACK, \"Recovery options\"),
        ] {
""",
    """        for (id, label) in [
            (ALLOW_CLOSE, \"Close during Game Mode\"),
            (REDUCE, \"Reduce during Game Mode\"),
            (KEEP, \"Keep / remove game rule\"),
            (PERMANENT, \"Disable Windows startup\"),
            (RESTORE_STARTUP, \"Restore Windows startup\"),
            (COPY_LIST, \"Copy / paste process list\"),
            (REPORT, \"Session details\"),
            (ADVANCED, \"Advanced tools\"),
            (ACK, \"Recovery options\"),
        ] {
""",
)
edit(
    "src/windows/simple_ui.rs",
    """        self.control(
            GPU_OPTION,
            \"BUTTON\",
            \"Experimental GPU priority (this activation only)\",
            WS_TABSTOP | BS_AUTOCHECKBOX as u32,
        )?;
        self.control(DETAIL,\"STATIC\",\"Approvals expire after 30 days or a detected file change.\\r\\nOn runs cleanup once; new/reopened apps are not chased.\\r\\nOff restores settings, not closed apps or unsaved work.\",0)?;
""",
    """        self.control(
            GPU_OPTION,
            \"BUTTON\",
            \"Experimental GPU priority (Advanced only)\",
            WS_TABSTOP | BS_AUTOCHECKBOX as u32,
        )?;
        self.control(GAME_LABEL, \"STATIC\", \"GAME MODE ONLY — temporary actions when you press Turn ON\", 0)?;
        self.control(PERMANENT_LABEL, \"STATIC\", \"PERMANENT CLEANUP — stop auto-starting with Windows (reversible)\", 0)?;
        self.control(
            COPY_TEXT,
            \"EDIT\",
            \"\",
            WS_TABSTOP | WS_VSCROLL | WS_HSCROLL | ES_MULTILINE_STYLE | ES_AUTOVSCROLL_STYLE | ES_AUTOHSCROLL_STYLE | ES_READONLY_STYLE,
        )?;
        self.control(DETAIL,\"STATIC\",\"Game Mode only affects approved apps when you press ON. Permanent cleanup currently disables supported current-user startup entries and can be restored here. It does not uninstall apps or disable system services.\",0)?;
""",
)

# Replace visibility/layout as one cohesive block.
start = Path("src/windows/simple_ui.rs").read_text(encoding="utf-8")
a = start.index("    fn visibility(&self) {")
b = start.index("    fn panel(&mut self, expanded: bool) {", a)
replacement = r'''    fn visibility(&self) {
        for id in PANEL {
            unsafe { ShowWindow(self.h(id), SW_HIDE); }
        }
        for id in [TITLE, SUBTITLE, TOGGLE, SUMMARY, HELP] {
            unsafe { ShowWindow(self.h(id), if self.expanded { SW_HIDE } else { SW_SHOW }); }
        }
        if self.expanded {
            unsafe { ShowWindow(self.h(INTRO), SW_SHOW); }
            if self.copy_mode {
                unsafe {
                    ShowWindow(self.h(COPY_TEXT), SW_SHOW);
                    ShowWindow(self.h(COPY_LIST), SW_SHOW);
                }
            } else {
                for id in [SCAN, APPS, ALLOW_CLOSE, REDUCE, KEEP, DETAIL, REPORT, ADVANCED, ACK,
                    PERMANENT, RESTORE_STARTUP, COPY_LIST, GAME_LABEL, PERMANENT_LABEL] {
                    unsafe { ShowWindow(self.h(id), SW_SHOW); }
                }
            }
        }
        self.text(SETTINGS, if self.expanded { "Done" } else { "Settings" });
        self.text(COPY_LIST, if self.copy_mode { "Back to app settings" } else { "Copy / paste process list" });
    }
    fn layout(&self) {
        let mut r: RECT = unsafe { zeroed() };
        unsafe { GetClientRect(self.window, &mut r); }
        let scale = unsafe { GetDpiForWindow(self.window) }.max(96) as f64 / 96.0;
        let w = (r.right as f64 / scale) as i32;
        let inner = w - 48;
        let put = |id, x: i32, y: i32, width: i32, height: i32| unsafe {
            MoveWindow(self.h(id), (x as f64 * scale) as i32, (y as f64 * scale) as i32,
                (width as f64 * scale) as i32, (height as f64 * scale) as i32, 1);
        };
        put(TITLE, 24, 26, inner, 44);
        put(SUBTITLE, 24, 74, inner, 24);
        put(TOGGLE, 48, 112, w - 96, 64);
        put(SUMMARY, 24, 190, inner, 46);
        put(SETTINGS, (w - 124) / 2, 246, 124, 32);
        put(HELP, 24, 290, inner, 34);
        if self.expanded {
            put(INTRO, 24, 20, inner - 160, 54);
            if self.copy_mode {
                put(COPY_TEXT, 24, 88, inner, 548);
                put(COPY_LIST, (w - 220) / 2, 650, 220, 40);
                put(SETTINGS, (w - 124) / 2, 706, 124, 32);
            } else {
                put(SCAN, w - 164, 28, 140, 34);
                put(APPS, 24, 84, inner, 280);
                put(GAME_LABEL, 24, 378, inner, 26);
                let bw = (inner - 16) / 3;
                for (i, id) in [ALLOW_CLOSE, REDUCE, KEEP].iter().enumerate() {
                    put(*id, 24 + i as i32 * (bw + 8), 408, bw, 48);
                }
                put(PERMANENT_LABEL, 24, 470, inner, 26);
                for (i, id) in [PERMANENT, RESTORE_STARTUP, COPY_LIST].iter().enumerate() {
                    put(*id, 24 + i as i32 * (bw + 8), 500, bw, 48);
                }
                put(DETAIL, 24, 562, inner, 54);
                for (i, id) in [REPORT, ADVANCED, ACK].iter().enumerate() {
                    put(*id, 24 + i as i32 * (bw + 8), 630, bw, 42);
                }
                put(SETTINGS, (w - 124) / 2, 698, 124, 32);
            }
        }
    }
'''
Path("src/windows/simple_ui.rs").write_text(start[:a] + replacement + start[b:], encoding="utf-8", newline="\n")

edit(
    "src/windows/simple_ui.rs",
    """            right: (600 * dpi / 96) as i32,
            bottom: ((if expanded { 602 } else { 336 }) * dpi / 96) as i32,
""",
    """            right: ((if expanded { 760 } else { 600 }) * dpi / 96) as i32,
            bottom: ((if expanded { 752 } else { 336 }) * dpi / 96) as i32,
""",
)
edit(
    "src/windows/simple_ui.rs",
    """        if expanded && !self.smoke {
            self.scan();
        }
""",
    """        if expanded && !self.copy_mode && !self.smoke {
            self.scan();
        }
""",
)

# Populate startup state and use friendly descriptions.
edit(
    "src/windows/simple_ui.rs",
    """    fn populate(&mut self) {
        let mut choices = BTreeMap::new();
""",
    """    fn populate(&mut self) {
        let startup_entries = startup::entries().unwrap_or_default();
        let mut choices = BTreeMap::new();
""",
)
edit(
    "src/windows/simple_ui.rs",
    """                protected: false,
                gpu: None,
            });
""",
    """                protected: false,
                gpu: None,
                startup_enabled: false,
                startup_disabled: false,
            });
""",
    count=1,
)
edit(
    "src/windows/simple_ui.rs",
    """                    protected: false,
                    gpu: None,
                });
""",
    """                    protected: false,
                    gpu: None,
                    startup_enabled: false,
                    startup_disabled: false,
                });
""",
    count=1,
)
edit(
    "src/windows/simple_ui.rs",
    """        self.choices = choices
            .into_values()
            .filter(|c| !c.protected || self.config.rules.iter().any(|r| r.app.path == c.path))
            .collect();
""",
    """        for backup in &self.config.startup_disabled {
            choices.entry(backup.app.path.clone()).or_insert_with(|| Choice {
                path: backup.app.path.clone(),
                name: backup.app.path.rsplit('\\\\').next().unwrap_or("App").into(),
                id: None,
                protected: false,
                gpu: None,
                startup_enabled: false,
                startup_disabled: true,
            });
        }
        for c in choices.values_mut() {
            c.startup_enabled = startup_entries.iter().any(|e| e.executable.as_deref() == Some(c.path.as_str()));
            c.startup_disabled = self.config.startup_disabled.iter().any(|b| b.app.path == c.path);
        }
        self.choices = choices
            .into_values()
            .filter(|c| !c.protected
                || self.config.rules.iter().any(|r| r.app.path == c.path)
                || self.config.startup_disabled.iter().any(|b| b.app.path == c.path))
            .collect();
""",
)
edit(
    "src/windows/simple_ui.rs",
    """            let value = c
                .gpu
                .map(|v| format!(\"  | GPU peak {v:.1}%\"))
                .unwrap_or_default();
            let line = format!(
                \"{}  — {}{}{}\",
                c.name,
                status,
                if c.id.is_none() { \" (not running)\" } else { \"\" },
                value
            );
""",
    """            let (category, hint) = describe(&c.name);
            let value = c.gpu.map(|v| format!(\" | GPU {v:.1}%\")).unwrap_or_default();
            let startup = if c.startup_disabled {
                \" | Startup disabled\"
            } else if c.startup_enabled {
                \" | Starts with Windows\"
            } else {
                \"\"
            };
            let line = format!(
                \"{} — {} — {}{}{} | {}{}\",
                c.name,
                category,
                status,
                startup,
                if c.id.is_none() { \" | not running\" } else { \"\" },
                hint,
                value
            );
""",
)
edit(
    "src/windows/simple_ui.rs",
    """            \"BACKGROUND APPS\\r\\nChoose once. Unknown and unapproved apps are kept.\",
""",
    """            \"CHOOSE WHAT TO CLEAN\\r\\nSelect an app, then choose Game Mode only or Permanent cleanup below.\",
""",
)

# Add persistent startup actions and copyable report before toggle().
needle = "    fn toggle(&mut self) -> AppResult<()> {"
text = Path("src/windows/simple_ui.rs").read_text(encoding="utf-8")
assert text.count(needle) == 1
methods = r'''    fn disable_startup(&mut self) -> AppResult<()> {
        if self.preparing || runner::database()?.active()?.is_some() {
            return Err("Turn Game Mode OFF before changing Windows startup.".into());
        }
        let choice = self.selected()?;
        if choice.protected {
            return Err("This application is protected.".into());
        }
        let id = choice.id.as_ref().ok_or("Open the app, Refresh apps, then review it before disabling startup.")?;
        let entries = startup::matching(&choice.path).map_err(|e| e.message)?;
        if entries.is_empty() {
            return Err("No supported current-user Windows startup entry matches this app. It may be a service, scheduled task, machine-wide entry, or not auto-started. Nothing was changed.".into());
        }
        let names = entries.iter().map(|e| e.value_name.as_str()).collect::<Vec<_>>().join(", ");
        if !confirm(self.window, &format!(
            "Disable {} from Windows startup?\n\nStartup entry: {}\n\nThis is persistent across reboots, but reversible with Restore Windows startup. It does NOT uninstall the app, disable services, or close the app right now.",
            choice.name, names)) {
            return Ok(());
        }
        let db = runner::database()?;
        let mut config = db.manual_settings()?;
        let owner = id.provenance.as_ref().ok_or("Unknown application owner")?.owner_sid.clone();
        for entry in &entries {
            if config.startup_disabled.iter().any(|b| b.value_name.eq_ignore_ascii_case(&entry.value_name)) {
                return Err("That startup entry is already backed up. Restore or review it first.".into());
            }
            let mut backup = manual::startup_backup(id, entry.value_name.clone(), entry.command.clone(), entry.kind.clone(), native::now())?;
            backup.owner_sid = owner.clone();
            config.startup_disabled.push(backup);
        }
        // Backup is durable before deleting any startup value.
        db.save_manual_settings(&config)?;
        for entry in &entries {
            if let Err(e) = startup::disable(entry) {
                self.config = db.manual_settings()?;
                self.populate();
                return Err(format!("Startup cleanup stopped: {} The original value is backed up; use Restore Windows startup.", e.message));
            }
        }
        self.config = db.manual_settings()?;
        self.populate();
        info(self.window, "Windows startup disabled for the selected app. The app is not uninstalled or closed. Use Restore Windows startup to undo this later.");
        Ok(())
    }

    fn restore_startup(&mut self) -> AppResult<()> {
        if self.preparing || runner::database()?.active()?.is_some() {
            return Err("Turn Game Mode OFF before restoring Windows startup.".into());
        }
        let path = self.selected()?.path.clone();
        let db = runner::database()?;
        let mut config = db.manual_settings()?;
        let backups: Vec<_> = config.startup_disabled.iter().filter(|b| b.app.path == path).cloned().collect();
        if backups.is_empty() {
            return Err("No startup backup exists for this app.".into());
        }
        if !confirm(self.window, "Restore this app's original Windows startup entry?\n\nA conflicting entry with the same name will never be overwritten.") {
            return Ok(());
        }
        for backup in &backups {
            startup::restore(&backup.value_name, &backup.command, &backup.kind).map_err(|e| e.message)?;
        }
        config.startup_disabled.retain(|b| b.app.path != path);
        db.save_manual_settings(&config)?;
        self.config = config;
        self.populate();
        info(self.window, "Original Windows startup entry restored.");
        Ok(())
    }

    fn process_report(&self) -> String {
        let mut rows: Vec<_> = self.snapshot.processes.iter().collect();
        rows.sort_by_key(|r| (r.name.to_ascii_lowercase(), r.identity.pid));
        let mut text = String::from("Process Optimizer — copyable process report\r\nNo executable paths, command lines, documents, or account IDs are included.\r\n\r\nPID\tApplication\tGPU peak\tDedicated MiB\tStatus\r\n");
        for row in rows {
            let gpu = busiest_engine(&row.gpu).map(|v| format!("{v:.1}%")).unwrap_or_else(|| "-".into());
            let mem = dedicated_allocations(&row.gpu).map(|b| format!("{:.1}", b as f64 / 1024.0 / 1024.0)).unwrap_or_else(|| "-".into());
            let status = row.protected_reason.as_deref().unwrap_or("Optional / not approved");
            text.push_str(&format!("{}\t{}\t{}\t{}\t{}\r\n", row.identity.pid, row.name, gpu, mem, status));
        }
        text
    }

    fn copy_toggle(&mut self) -> AppResult<()> {
        if self.copy_mode {
            self.copy_mode = false;
            self.visibility();
            self.layout();
            return Ok(());
        }
        if self.snapshot.processes.is_empty() {
            return Err("Refresh apps first, then open the copyable process list.".into());
        }
        self.text(COPY_TEXT, &self.process_report());
        self.text(INTRO, "COPY PROCESS LIST\r\nDrag to select text, or press Ctrl+A then Ctrl+C. Paste it directly into ChatGPT.");
        self.copy_mode = true;
        self.visibility();
        self.layout();
        unsafe { SetFocus(self.h(COPY_TEXT)); }
        Ok(())
    }

'''
Path("src/windows/simple_ui.rs").write_text(text.replace(needle, methods + needle), encoding="utf-8", newline="\n")

# Simpler default: experimental GPU scheduling remains in Advanced only.
edit(
    "src/windows/simple_ui.rs",
    """        let experimental = unsafe { SendMessageW(self.h(GPU_OPTION), 0x00F0, 0, 0) } == 1;
        if experimental&&!confirm(self.window,\"Enable experimental GPU scheduling for this activation?\\n\\nIt is a scheduling preference, not GPU blocking. Driver support and gaming benefit remain unverified. Only approved Reduce load apps are eligible.\"){return Ok(());}
        unsafe {
            SendMessageW(self.h(GPU_OPTION), 0x00F1, 0, 0);
        }
""",
    """        let experimental = false;
""",
)
edit(
    "src/windows/simple_ui.rs",
    """            for id in [ALLOW_CLOSE, REDUCE, KEEP, GPU_OPTION, ADVANCED] {
                self.enable(id, idle);
            }
""",
    """            for id in [ALLOW_CLOSE, REDUCE, KEEP, PERMANENT, RESTORE_STARTUP, ADVANCED] {
                self.enable(id, idle);
            }
""",
)
edit(
    "src/windows/simple_ui.rs",
    """                let n = self
                    .config
                    .rules
                    .iter()
                    .filter(|r| r.active(native::now()))
                    .count();
                self.text(
                    SUMMARY,
                    &if n == 0 {
                        \"Choose background apps once in Settings.\".into()
                    } else {
                        format!(\"{n} background-app permission(s) ready.\")
                    },
                );
""",
    """                let n = self.config.rules.iter().filter(|r| r.active(native::now())).count();
                let persistent = self.config.startup_disabled.len();
                self.text(
                    SUMMARY,
                    &if n == 0 && persistent == 0 {
                        \"Settings: choose Game Mode-only or Permanent cleanup.\".into()
                    } else {
                        format!(\"{n} Game Mode rule(s) ready; {persistent} startup item(s) disabled.\")
                    },
                );
""",
)
edit(
    "src/windows/simple_ui.rs",
    """            KEEP => self.permit(None)?,
            REPORT => self.report()?,
""",
    """            KEEP => self.permit(None)?,
            PERMANENT => self.disable_startup()?,
            RESTORE_STARTUP => self.restore_startup()?,
            COPY_LIST => self.copy_toggle()?,
            REPORT => self.report()?,
""",
)
edit(
    "src/windows/simple_ui.rs",
    """            if s.widgets.len() != 17 {
""",
    """            if s.widgets.len() != 23 {
""",
)

# Behavioral spec: allow reversible persistent startup cleanup, not irreversible debloat.
edit(
    "specs/01-product.md",
    "P-03: Overclocking, undervolting, fan/firmware control, permanent debloating, security disabling, kernel patching, driver reset, game injection, anti-cheat bypass and a stripped boot shell are excluded.",
    "P-03: Overclocking, undervolting, fan/firmware control, irreversible debloating/service deletion, security disabling, kernel patching, driver reset, game injection, anti-cheat bypass and a stripped boot shell are excluded. Reversible, explicitly approved current-user startup cleanup is in scope when the original startup record is durably backed up before mutation.",
)
anchor = "M-06: A dead worker, unresolved operation or corrupt store must not be displayed as an ordinary successful On/Off transition."
text = Path("specs/01-product.md").read_text(encoding="utf-8")
assert text.count(anchor) == 1
insert = """M-07: Settings presents two plain-language choices: **Game Mode only** (temporary close/reduce actions applied only at user-initiated On) and **Permanent cleanup** (persistent, reversible startup cleanup). Permanent cleanup v0.4 only supports exact current-user `Run` startup values whose executable token matches the reviewed app. It backs up the original value before deletion and restores only when no conflicting value occupies that name. It never implies uninstalling the app, disabling Windows services/drivers/tasks, or closing the currently running process. Unsupported startup sources are reported without fallback.\n\nM-08: Settings exposes a copyable process report. The user can drag-select text or use Ctrl+A/Ctrl+C and paste the report into support/chat. The report may include PID, executable display name, bounded GPU/memory observation and protection status, but excludes executable paths, command lines, document/window content, SIDs and account identifiers.\n\n"
text = text.replace(anchor, insert + anchor)
Path("specs/01-product.md").write_text(text, encoding="utf-8", newline="\n")

# Policy owner: documented startup boundary.
text = Path("specs/02-gpu-and-process-policy.md").read_text(encoding="utf-8")
anchor = "## Unsupported features and future adapters"
assert text.count(anchor) == 1
policy = """## Persistent startup cleanup\n\nPersistent cleanup is not a gaming-session mutation. Version 0.4 may enumerate and edit only `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run` string values. An entry is eligible only when its first executable token can be conservatively resolved to the reviewed canonical executable; environment-variable/shell ambiguity is unsupported. Store the exact value name, command string and string kind before deletion. Re-read immediately before deletion; observable drift cancels the operation. Restoration never overwrites a different value using the same name.\n\nDo not use this feature to disable machine-wide startup, services, drivers, scheduled tasks, security software, device/audio/Bluetooth components, shell components or vendor utilities that cannot be attributed through the supported source. There is no process-kill fallback. Future service/task adapters require separate owner contracts, elevation design, dependency checks and tests.\n\n"""
Path("specs/02-gpu-and-process-policy.md").write_text(text.replace(anchor, policy + anchor), encoding="utf-8", newline="\n")

# Validation owner: add specific new cases.
text = Path("specs/04-validation-and-delivery.md").read_text(encoding="utf-8")
anchor = "| Empty current matches in manual mode | Honest no-change state, never fallback to wildcard cleanup |"
assert text.count(anchor) == 1
extra = anchor + "\n| Copyable process report | Drag/Ctrl+A/Ctrl+C-ready plain text; no paths, command lines, documents, SIDs or account identifiers |\n| Permanent cleanup unsupported source | Report unsupported; no service/task/kill fallback |\n| Permanent cleanup supported Run entry | Backup exact value before delete and verify absence |\n| Permanent cleanup entry changes after review | Abort deletion; retain existing value |\n| Restore startup name conflict | Preserve conflicting value; never overwrite it |"
Path("specs/04-validation-and-delivery.md").write_text(text.replace(anchor, extra), encoding="utf-8", newline="\n")

# README product language.
text = Path("README.md").read_text(encoding="utf-8")
anchor = "## Current implementation"
assert text.count(anchor) == 1
intro = """## New in 0.4.0 — two cleanup choices\n\nSettings now separates **Game Mode only** from **Permanent cleanup**. Game Mode only closes or reduces approved apps when you press ON. Permanent cleanup currently means one narrow, reversible operation: disable a matching current-user Windows `Run` startup entry after backing up its exact original value. It does not uninstall apps, disable services/drivers/tasks, or kill a running process. Unsupported sources are left alone.\n\nThe Settings list now explains common app types in plain language (browser, overlay, developer tool, sync utility, audio hardware software, ASUS utility, and so on). **Copy / paste process list** opens a read-only text view: drag-select text or press Ctrl+A/Ctrl+C and paste it directly into ChatGPT. The copied report deliberately omits executable paths, command lines, document/window content and account identifiers.\n\n"""
Path("README.md").write_text(text.replace(anchor, intro + anchor), encoding="utf-8", newline="\n")

# Deterministic model tests for persistent settings and schema migration shape.
path = Path("tests/manual_mode.rs")
text = path.read_text(encoding="utf-8")
text += r'''

#[test]
fn persistent_startup_backup_is_not_a_game_mode_rule() {
    let target = id(90, "startup-app");
    let backup = manual::startup_backup(
        &target,
        "ExampleStartup".into(),
        r#""C:\Fixture\startup-app.exe" --background"#.into(),
        manual::StartupValueKind::String,
        100,
    ).unwrap();
    let config = manual::Config { schema: manual::CONFIG_SCHEMA, rules: vec![], startup_disabled: vec![backup] };
    manual::validate(&config).unwrap();
    assert!(config.rules.is_empty());
}

#[test]
fn duplicate_startup_backup_names_fail_closed() {
    let target = id(91, "startup-app");
    let backup = manual::startup_backup(&target, "SameName".into(), r"C:\Fixture\startup-app.exe".into(), manual::StartupValueKind::String, 100).unwrap();
    let config = manual::Config { schema: manual::CONFIG_SCHEMA, rules: vec![], startup_disabled: vec![backup.clone(), backup] };
    assert!(manual::validate(&config).is_err());
}
'''
path.write_text(text, encoding="utf-8", newline="\n")

print("Prepared v0.4 two-mode cleanup UI, copyable diagnostics, and reversible startup cleanup.")
