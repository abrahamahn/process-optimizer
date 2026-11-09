//! Small native front door: On/Off first, app permissions behind Settings.
use super::{
    gpu, manual as native, process, runner,
    simple_recommendations::{
        build_choices, format_choice_detail, format_choice_row, format_process_report, Choice,
    },
    startup, ui_common, update, wide,
};
use crate::{
    manual::{self, Config, RuleAction},
    model::*,
    policy,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    mem::zeroed,
    ptr::{null, null_mut},
    sync::{Arc, Mutex},
    thread,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{HiDpi::*, WindowsAndMessaging::*},
};

// Winuser.h static-control centering style; absent from this binding namespace.
const SS_CENTER: i32 = 0x0001;
const TITLE: u16 = 1;
const SUBTITLE: u16 = 2;
const TOGGLE: u16 = 3;
const SUMMARY: u16 = 4;
const SETTINGS: u16 = 5;
const HELP: u16 = 6;
const APPS: u16 = 20;
const INTRO: u16 = 21;
const SCAN: u16 = 22;
const ALLOW_CLOSE: u16 = 23;
const REDUCE: u16 = 24;
const KEEP: u16 = 25;
const GPU_OPTION: u16 = 26;
const DETAIL: u16 = 27;
const REPORT: u16 = 28;
const ADVANCED: u16 = 29;
const ACK: u16 = 30;
const PERMANENT: u16 = 31;
const RESTORE_STARTUP: u16 = 32;
const COPY_LIST: u16 = 33;
const COPY_TEXT: u16 = 34;
const GAME_LABEL: u16 = 35;
const PERMANENT_LABEL: u16 = 36;
const UPDATE: u16 = 37;
const PANEL: [u16; 18] = [
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
    PERMANENT,
    RESTORE_STARTUP,
    COPY_LIST,
    COPY_TEXT,
    GAME_LABEL,
    PERMANENT_LABEL,
    UPDATE,
];
const ES_MULTILINE_STYLE: u32 = 0x0004;
const ES_AUTOVSCROLL_STYLE: u32 = 0x0040;
const ES_AUTOHSCROLL_STYLE: u32 = 0x0080;
const ES_READONLY_STYLE: u32 = 0x0800;

#[repr(C)]
struct AppDrawItem {
    _ctl_type: u32,
    ctl_id: u32,
    item_id: u32,
    _item_action: u32,
    item_state: u32,
    _hwnd_item: HWND,
    hdc: HDC,
    rc_item: RECT,
    _item_data: usize,
}
type Slot<T> = Arc<Mutex<Option<AppResult<T>>>>;

struct State {
    window: HWND,
    widgets: BTreeMap<u16, HWND>,
    font: HFONT,
    heading: HFONT,
    expanded: bool,
    copy_mode: bool,
    minimized: bool,
    smoke: bool,
    error: Option<String>,
    config: Config,
    snapshot: Snapshot,
    choices: Vec<Choice>,
    samples: Slot<Snapshot>,
    scanning: bool,
    prepared: Slot<(Plan, usize)>,
    preparing: bool,
    cancel_prepare: bool,
}
impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            if !self.font.is_null() {
                DeleteObject(self.font);
            }
            if !self.heading.is_null() {
                DeleteObject(self.heading);
            }
        }
    }
}
impl State {
    fn new(smoke: bool) -> AppResult<Self> {
        Ok(Self {
            window: null_mut(),
            widgets: BTreeMap::new(),
            font: null_mut(),
            heading: null_mut(),
            expanded: false,
            copy_mode: false,
            minimized: false,
            smoke,
            error: None,
            config: if smoke {
                Config::default()
            } else {
                runner::database()?.manual_settings()?
            },
            snapshot: Snapshot::default(),
            choices: vec![],
            samples: Arc::new(Mutex::new(None)),
            scanning: false,
            prepared: Arc::new(Mutex::new(None)),
            preparing: false,
            cancel_prepare: false,
        })
    }
    fn h(&self, id: u16) -> HWND {
        self.widgets.get(&id).copied().unwrap_or(null_mut())
    }
    fn text(&self, id: u16, text: &str) {
        ui_common::set_window_text_if_changed(self.h(id), text);
    }
    fn enable(&self, id: u16, value: bool) {
        unsafe {
            windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                self.h(id),
                i32::from(value),
            );
        }
    }
    fn check(&self, id: u16, checked: bool) {
        ui_common::set_check(self.h(id), checked);
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
        // A protected/essential app is already a mandatory Keep. Only leave this
        // control active if it can remove an older saved rule that is now blocked.
        self.enable(KEEP, idle && (!c.protected || configured_rule.is_some()));
        self.enable(REDUCE, can_game_change);
        self.enable(ALLOW_CLOSE, can_game_change);
        self.enable(
            PERMANENT,
            idle && !c.protected && c.id.is_some() && c.startup_enabled && !c.startup_disabled,
        );
        self.enable(
            RESTORE_STARTUP,
            idle && (c.startup_enabled || c.startup_disabled),
        );
        self.text(DETAIL, &format_choice_detail(c, rule));
    }
    fn draw_app_item(&self, item: &AppDrawItem) -> bool {
        if item.ctl_id != APPS as u32 || item.item_id == u32::MAX {
            return false;
        }
        unsafe {
            let selected = item.item_state & 0x0001u32 != 0;
            let background = if selected {
                GetSysColorBrush(COLOR_HIGHLIGHT)
            } else {
                GetSysColorBrush(COLOR_WINDOW)
            };
            FillRect(item.hdc, &item.rc_item, background);
            let essential = self
                .choices
                .get(item.item_id as usize)
                .is_some_and(|c| c.essential);
            SetTextColor(
                item.hdc,
                if essential {
                    0x000000d0
                } else if selected {
                    GetSysColor(COLOR_HIGHLIGHTTEXT)
                } else {
                    GetSysColor(COLOR_WINDOWTEXT)
                },
            );
            SetBkMode(item.hdc, 1);
            let length = SendMessageW(self.h(APPS), LB_GETTEXTLEN, item.item_id as usize, 0);
            if length >= 0 {
                let mut buffer = vec![0u16; length as usize + 1];
                SendMessageW(
                    self.h(APPS),
                    LB_GETTEXT,
                    item.item_id as usize,
                    buffer.as_mut_ptr() as LPARAM,
                );
                let mut rect = item.rc_item;
                rect.left += 6;
                rect.right -= 6;
                DrawTextW(
                    item.hdc,
                    buffer.as_mut_ptr(),
                    length as i32,
                    &mut rect,
                    DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | DT_END_ELLIPSIS,
                );
            }
            if item.item_state & 0x0010u32 != 0 {
                DrawFocusRect(item.hdc, &item.rc_item);
            }
        }
        true
    }
    fn control(&mut self, id: u16, class: &str, label: &str, style: u32) -> AppResult<()> {
        let h = ui_common::create_control(self.window, id, class, label, style)?;
        self.widgets.insert(id, h);
        Ok(())
    }
    fn init(&mut self) -> AppResult<()> {
        self.control(TITLE, "STATIC", "Game Mode is OFF", SS_CENTER as u32)?;
        self.control(
            SUBTITLE,
            "STATIC",
            "Your PC, ready for play.",
            SS_CENTER as u32,
        )?;
        self.control(
            TOGGLE,
            "BUTTON",
            "Turn ON",
            WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
        )?;
        self.control(
            SUMMARY,
            "STATIC",
            "Choose background apps once in Settings.",
            SS_CENTER as u32,
        )?;
        self.control(SETTINGS, "BUTTON", "Settings", WS_TABSTOP)?;
        self.control(
            HELP,
            "STATIC",
            "No game selection. Turn OFF when you finish.",
            SS_CENTER as u32,
        )?;
        self.control(
            INTRO,
            "STATIC",
            "APP CONTROL\r\nEach app has two independent choices: Game Mode behavior and Windows startup.",
            0,
        )?;
        self.control(SCAN, "BUTTON", "Refresh apps", WS_TABSTOP)?;
        self.control(
            APPS,
            "LISTBOX",
            "",
            WS_TABSTOP
                | WS_VSCROLL
                | WS_HSCROLL
                | LBS_NOTIFY as u32
                | LBS_NOINTEGRALHEIGHT as u32
                | LBS_OWNERDRAWFIXED as u32
                | LBS_HASSTRINGS as u32,
        )?;
        for (id, label) in [
            (KEEP, "Keep / exclude"),
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
                KEEP | RESTORE_STARTUP => WS_TABSTOP | WS_GROUP | BS_AUTORADIOBUTTON as u32,
                REDUCE | ALLOW_CLOSE | PERMANENT => WS_TABSTOP | BS_AUTORADIOBUTTON as u32,
                _ => WS_TABSTOP,
            };
            self.control(id, "BUTTON", label, style)?;
        }
        self.control(
            UPDATE,
            "BUTTON",
            &format!("Check for updates — v{}", env!("CARGO_PKG_VERSION")),
            WS_TABSTOP,
        )?;
        self.control(
            GPU_OPTION,
            "BUTTON",
            "Experimental GPU priority (Advanced only)",
            WS_TABSTOP | BS_AUTOCHECKBOX as u32,
        )?;
        self.control(
            GAME_LABEL,
            "STATIC",
            "GAME MODE — choose one state for the selected app",
            0,
        )?;
        self.control(
            PERMANENT_LABEL,
            "STATIC",
            "WINDOWS STARTUP — Keep or Disable for the selected app",
            0,
        )?;
        self.control(
            COPY_TEXT,
            "EDIT",
            "",
            WS_TABSTOP
                | WS_VSCROLL
                | WS_HSCROLL
                | ES_MULTILINE_STYLE
                | ES_AUTOVSCROLL_STYLE
                | ES_AUTOHSCROLL_STYLE
                | ES_READONLY_STYLE,
        )?;
        self.control(DETAIL,"STATIC","Select an app to see its current state and recommendation. Keep removes a Game Mode rule. Turn OFF restores priority changes already applied by the current Game Mode session.",0)?;
        self.fonts();
        self.visibility();
        self.layout();
        if !self.smoke {
            unsafe {
                SetTimer(self.window, 1, 1000, None);
            }
            self.poll();
        }
        Ok(())
    }
    fn fonts(&mut self) {
        let dpi = unsafe { GetDpiForWindow(self.window) }.max(96);
        let (old, old_heading) = (self.font, self.heading);
        self.font = ui_common::create_dpi_font("Segoe UI", 15, 400, dpi);
        self.heading = ui_common::create_dpi_font("Segoe UI", 28, 600, dpi);
        for (id, h) in &self.widgets {
            unsafe {
                SendMessageW(
                    *h,
                    WM_SETFONT,
                    if *id == TITLE {
                        self.heading as usize
                    } else {
                        self.font as usize
                    },
                    1,
                );
            }
        }
        unsafe {
            let item_height = ((24 * dpi / 96).max(20)) as LPARAM;
            SendMessageW(self.h(APPS), LB_SETITEMHEIGHT, 0, item_height);
            if !old.is_null() {
                DeleteObject(old);
            }
            if !old_heading.is_null() {
                DeleteObject(old_heading);
            }
        }
    }
    fn visibility(&self) {
        for id in PANEL {
            unsafe {
                ShowWindow(self.h(id), SW_HIDE);
            }
        }
        for id in [TITLE, SUBTITLE, TOGGLE, SUMMARY, HELP] {
            unsafe {
                ShowWindow(self.h(id), if self.expanded { SW_HIDE } else { SW_SHOW });
            }
        }
        if self.expanded {
            unsafe {
                ShowWindow(self.h(INTRO), SW_SHOW);
            }
            if self.copy_mode {
                unsafe {
                    ShowWindow(self.h(COPY_TEXT), SW_SHOW);
                    ShowWindow(self.h(COPY_LIST), SW_SHOW);
                }
            } else {
                for id in [
                    SCAN,
                    APPS,
                    ALLOW_CLOSE,
                    REDUCE,
                    KEEP,
                    DETAIL,
                    REPORT,
                    ADVANCED,
                    ACK,
                    UPDATE,
                    PERMANENT,
                    RESTORE_STARTUP,
                    COPY_LIST,
                    GAME_LABEL,
                    PERMANENT_LABEL,
                ] {
                    unsafe {
                        ShowWindow(self.h(id), SW_SHOW);
                    }
                }
            }
        }
        self.text(SETTINGS, if self.expanded { "Done" } else { "Settings" });
        self.text(
            COPY_LIST,
            if self.copy_mode {
                "Back to app settings"
            } else {
                "Copy / paste process list"
            },
        );
    }
    fn layout(&self) {
        let mut r: RECT = unsafe { zeroed() };
        unsafe {
            GetClientRect(self.window, &mut r);
        }
        let scale = unsafe { GetDpiForWindow(self.window) }.max(96) as f64 / 96.0;
        let w = (r.right as f64 / scale) as i32;
        let inner = w - 48;
        let put = |id, x: i32, y: i32, width: i32, height: i32| unsafe {
            MoveWindow(
                self.h(id),
                (x as f64 * scale) as i32,
                (y as f64 * scale) as i32,
                (width as f64 * scale) as i32,
                (height as f64 * scale) as i32,
                1,
            );
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
                for (i, id) in [KEEP, REDUCE, ALLOW_CLOSE].iter().enumerate() {
                    put(*id, 24 + i as i32 * (bw + 8), 408, bw, 48);
                }
                put(PERMANENT_LABEL, 24, 470, inner, 26);
                for (i, id) in [RESTORE_STARTUP, PERMANENT, COPY_LIST].iter().enumerate() {
                    put(*id, 24 + i as i32 * (bw + 8), 500, bw, 48);
                }
                put(DETAIL, 24, 562, inner, 54);
                let utility_w = (inner - 24) / 4;
                for (i, id) in [REPORT, ADVANCED, ACK, UPDATE].iter().enumerate() {
                    put(*id, 24 + i as i32 * (utility_w + 8), 630, utility_w, 42);
                }
                put(SETTINGS, (w - 124) / 2, 698, 124, 32);
            }
        }
    }
    fn panel(&mut self, expanded: bool) {
        self.expanded = expanded;
        self.visibility();
        let dpi = unsafe { GetDpiForWindow(self.window) }.max(96);
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: ((if expanded { 760 } else { 600 }) * dpi / 96) as i32,
            bottom: ((if expanded { 752 } else { 336 }) * dpi / 96) as i32,
        };
        unsafe {
            AdjustWindowRectEx(
                &mut rect,
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                0,
                WS_EX_CONTROLPARENT,
            );
            SetWindowPos(
                self.window,
                null_mut(),
                0,
                0,
                rect.right - rect.left,
                rect.bottom - rect.top,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        self.layout();
        if expanded && !self.copy_mode && !self.smoke {
            self.scan();
        }
    }
    fn scan(&mut self) {
        if self.scanning {
            return;
        }
        self.scanning = true;
        self.text(INTRO, "Reading background apps and GPU activity...");
        let slot = self.samples.clone();
        thread::spawn(move || {
            let result = gpu::snapshot();
            if let Ok(mut s) = slot.lock() {
                *s = Some(result);
            }
        });
    }
    fn populate(&mut self) {
        let selected_path = self
            .selected_index()
            .and_then(|i| self.choices.get(i))
            .map(|c| c.path.clone());
        let startup_entries = startup::entries().unwrap_or_default();
        self.choices = build_choices(&self.snapshot.processes, &self.config, &startup_entries);
        unsafe {
            SendMessageW(self.h(APPS), WM_SETREDRAW, 0, 0);
            SendMessageW(self.h(APPS), LB_RESETCONTENT, 0, 0);
        }
        let now = native::now();
        for c in &self.choices {
            let line = format_choice_row(c, &self.config, now);
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
            SendMessageW(self.h(APPS), LB_SETHORIZONTALEXTENT, 1200, 0);
            SendMessageW(self.h(APPS), WM_SETREDRAW, 1, 0);
            InvalidateRect(self.h(APPS), null(), 1);
            if !self.choices.is_empty() {
                let selected = selected_path
                    .as_ref()
                    .and_then(|path| self.choices.iter().position(|c| &c.path == path))
                    .unwrap_or(0);
                SendMessageW(self.h(APPS), LB_SETCURSEL, selected, 0);
            }
        }
        self.text(
            INTRO,
            "APP CONTROL\r\nSelect an app. Choose its Game Mode state and Windows startup state independently.",
        );
    }
    fn selected(&self) -> AppResult<&Choice> {
        let i = unsafe { SendMessageW(self.h(APPS), LB_GETCURSEL, 0, 0) };
        if i < 0 {
            return Err("Select a background app in Settings first.".into());
        }
        self.choices
            .get(i as usize)
            .ok_or("Selection changed; refresh the list.".into())
    }
    fn permit(&mut self, action: Option<RuleAction>) -> AppResult<()> {
        if self.preparing || runner::database()?.active()?.is_some() {
            return Err("Turn Game Mode OFF before changing app permissions.".into());
        }
        let choice = self.selected()?;
        let path = choice.path.clone();
        let rule = if let Some(action) = action {
            if choice.protected {
                return Err("This application is protected.".into());
            }
            let id = choice
                .id
                .as_ref()
                .ok_or("Open the app and refresh Settings before approving it.")?;
            let rows = process::enumerate().map_err(|e| e.message)?;
            let current = rows
                .iter()
                .find(|r| policy::same_process(&r.identity, id))
                .ok_or("App changed; refresh and review it again.")?;
            let settings = runner::database()?.settings()?;
            if current.protected_reason.is_some()
                || settings
                    .protected_paths
                    .iter()
                    .any(|p| policy::normalized_path(p) == path)
            {
                return Err("This app is protected. Its permission was not changed.".into());
            }
            let effect = match action {
                RuleAction::Close => "request normal close (never force termination)",
                RuleAction::ReduceLoad => "lower supported CPU, energy and memory priorities",
            };
            let question=format!("Remember permission for {}?\n\n{}\n\nEach time YOU press Turn ON over the next 30 days, the app may {} for the current processes using this unchanged executable.\n\nApprove only optional background apps — not games, voice, accessibility or device tools. New processes are not chased. Remove permission with Keep.\n\nClosing can lose application state; OFF restores settings, not closed apps or unsaved work. No automatic reopening is approved here.",choice.name,path,effect);
            if !confirm(self.window, &question) {
                return Ok(());
            }
            Some(manual::approve(
                id,
                native::stamp(id).map_err(|e| e.message)?,
                action,
                native::now(),
            )?)
        } else {
            None
        };
        // Refresh before the write; never erase permissions from a stale local copy.
        let db = runner::database()?;
        let mut config = db.manual_settings()?;
        config.rules.retain(|r| r.app.path != path);
        if let Some(rule) = rule {
            config.rules.push(rule);
        }
        db.save_manual_settings(&config)?;
        self.config = config;
        self.populate();
        Ok(())
    }
    fn disable_startup(&mut self) -> AppResult<()> {
        if self.preparing || runner::database()?.active()?.is_some() {
            return Err("Turn Game Mode OFF before changing Windows startup.".into());
        }
        let choice = self.selected()?;
        if choice.protected {
            return Err("This application is protected.".into());
        }
        let id = choice
            .id
            .as_ref()
            .ok_or("Open the app, Refresh apps, then review it before disabling startup.")?;
        let entries = startup::matching(&choice.path).map_err(|e| e.message)?;
        if entries.is_empty() {
            return Err("No supported current-user Windows startup entry matches this app. It may be a service, scheduled task, machine-wide entry, or not auto-started. Nothing was changed.".into());
        }
        let names = entries
            .iter()
            .map(|e| e.value_name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if !confirm(self.window, &format!(
            "Disable {} from Windows startup?\n\nStartup entry: {}\n\nThis is persistent across reboots, but reversible with Restore Windows startup. It does NOT uninstall the app, disable services, or close the app right now.",
            choice.name, names)) {
            return Ok(());
        }
        let db = runner::database()?;
        let mut config = db.manual_settings()?;
        let owner = id
            .provenance
            .as_ref()
            .ok_or("Unknown application owner")?
            .owner_sid
            .clone();
        for entry in &entries {
            if config
                .startup_disabled
                .iter()
                .any(|b| b.value_name.eq_ignore_ascii_case(&entry.value_name))
            {
                return Err(
                    "That startup entry is already backed up. Restore or review it first.".into(),
                );
            }
            let mut backup = manual::startup_backup(
                id,
                entry.value_name.clone(),
                entry.command.clone(),
                entry.kind.clone(),
                native::now(),
            )?;
            backup.owner_sid = owner.clone();
            config.startup_disabled.push(backup);
        }
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

    fn keep_startup(&mut self) -> AppResult<()> {
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
        if self.preparing || runner::database()?.active()?.is_some() {
            return Err("Turn Game Mode OFF before restoring Windows startup.".into());
        }
        let path = self.selected()?.path.clone();
        let db = runner::database()?;
        let mut config = db.manual_settings()?;
        let backups: Vec<_> = config
            .startup_disabled
            .iter()
            .filter(|b| b.app.path == path)
            .cloned()
            .collect();
        if backups.is_empty() {
            return Err("No startup backup exists for this app.".into());
        }
        if !confirm(self.window, "Restore this app's original Windows startup entry?\n\nA conflicting entry with the same name will never be overwritten.") {
            return Ok(());
        }
        for backup in &backups {
            startup::restore(&backup.value_name, &backup.command, &backup.kind)
                .map_err(|e| e.message)?;
        }
        config.startup_disabled.retain(|b| b.app.path != path);
        db.save_manual_settings(&config)?;
        self.config = config;
        self.populate();
        info(self.window, "Original Windows startup entry restored.");
        Ok(())
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
        self.text(COPY_TEXT, &format_process_report(&self.snapshot.processes));
        self.text(INTRO, "COPY PROCESS LIST\r\nDrag to select text, or press Ctrl+A then Ctrl+C. Paste it directly into ChatGPT.");
        self.copy_mode = true;
        self.visibility();
        self.layout();
        unsafe {
            SetFocus(self.h(COPY_TEXT));
        }
        Ok(())
    }

    fn toggle(&mut self) -> AppResult<()> {
        if self.preparing {
            self.cancel_prepare = true;
            self.text(TOGGLE, "Cancelling...");
            self.enable(TOGGLE, false);
            return Ok(());
        }
        let db = runner::database()?;
        if let Some(s) = db.active()? {
            db.request_stop(&s.id)?;
            // Worker locking makes duplicate recovery attempts harmless. Do not clear the record.
            if !s
                .worker
                .as_ref()
                .is_some_and(|id| matches!(process::alive(id), Ok(true)))
            {
                runner::spawn_worker("--recover", None)?;
            }
            self.text(TITLE, "Turning Game Mode OFF");
            self.text(SUMMARY, "Restoring recorded settings...");
            return Ok(());
        }
        self.config = db.manual_settings()?;
        if !self.config.rules.iter().any(|r| r.active(native::now())) {
            self.panel(true);
            self.text(
                SUMMARY,
                "One-time setup: approve optional background apps below.",
            );
            return Ok(());
        }
        let experimental = false;
        self.preparing = true;
        self.cancel_prepare = false;
        self.text(TITLE, "Preparing Game Mode");
        self.text(TOGGLE, "Cancel");
        self.text(
            SUMMARY,
            "Checking your saved app permissions. No game selection needed.",
        );
        let slot = self.prepared.clone();
        thread::spawn(move || {
            let result = native::build_plan(experimental);
            if let Ok(mut p) = slot.lock() {
                *p = Some(result);
            }
        });
        Ok(())
    }
    fn report(&self) -> AppResult<()> {
        if let Some(s) = runner::database()?.latest()? {
            info(self.window, &crate::report::render(&s));
        } else {
            info(self.window,"No session has run yet. First approve optional background apps in Settings, then Turn ON.");
        }
        Ok(())
    }
    fn poll(&mut self) {
        if self.smoke || self.minimized {
            return;
        }
        let sample = self.samples.lock().ok().and_then(|mut v| v.take());
        if let Some(result) = sample {
            self.scanning = false;
            match result {
                Ok(s) => {
                    self.snapshot = s;
                    self.populate();
                }
                Err(e) => self.text(INTRO, &format!("App scan unavailable: {e}")),
            }
        }
        let prepared = self.prepared.lock().ok().and_then(|mut v| v.take());
        if let Some(result) = prepared {
            self.preparing = false;
            if !self.cancel_prepare {
                let start = (|| -> AppResult<()> {
                    let (plan, excluded) = result?;
                    let mut db = runner::database()?;
                    let mut session = Session::new(runner::fresh_id(), plan);
                    session.note(format!("Manual On/Off mode. {excluded} stale/foreground target(s) excluded. Only the finite approved snapshot is considered; new apps are not re-closed."));
                    db.create(&session)?;
                    if let Err(e) = runner::spawn_worker("--session", Some(&session.id)) {
                        db.request_stop(&session.id)?;
                        return Err(e);
                    }
                    Ok(())
                })();
                if let Err(e) = start {
                    info(self.window, &e);
                }
            }
            self.cancel_prepare = false;
        }
        if self.preparing {
            return;
        }
        let result = (|| -> AppResult<()> {
            let db = runner::database()?;
            let active = db.active()?;
            let idle = active.is_none();
            self.enable(ADVANCED, idle);
            self.enable(UPDATE, idle);
            self.sync_selected_controls(idle);
            self.enable(SCAN, !self.scanning);
            self.enable(TOGGLE, true);
            self.enable(
                ACK,
                active
                    .as_ref()
                    .is_some_and(|s| s.stage == Stage::RecoveryNeeded),
            );
            if let Some(s) = active {
                let alive = s
                    .worker
                    .as_ref()
                    .is_some_and(|w| matches!(process::alive(w), Ok(true)));
                if s.stage == Stage::RecoveryNeeded || (!alive && s.worker.is_some()) {
                    self.text(TITLE, "Recovery needs attention");
                    self.text(TOGGLE, "Restore settings");
                    self.text(
                        SUMMARY,
                        "Some outcomes need review. Settings > Session details.",
                    );
                } else if s.stage == Stage::Restoring || db.stop_requested(&s.id)? {
                    self.text(TITLE, "Turning Game Mode OFF");
                    self.text(TOGGLE, "Restore settings");
                    self.text(
                        SUMMARY,
                        "Restoring original settings. Closed apps are not reopened here.",
                    );
                } else if s.stage == Stage::Active {
                    self.text(TITLE, "Game Mode is ON");
                    self.text(TOGGLE, "Turn OFF");
                    let closes = s
                        .closed
                        .iter()
                        .filter(|c| {
                            matches!(
                                c.state,
                                CloseState::ClosedGracefully | CloseState::Terminated
                            )
                        })
                        .count();
                    self.text(SUMMARY,&if s.plan.actions.is_empty(){"No approved apps matched. Nothing was changed.".into()}else{format!("{closes} process(es) closed; {} setting(s) recorded.\r\nYou can start or switch games normally.",s.changes.len())});
                } else {
                    self.text(TITLE, "Starting Game Mode");
                    self.text(TOGGLE, "Turn OFF");
                    self.text(SUMMARY, "Applying approved background-app actions...");
                }
                self.text(
                    HELP,
                    "Closing this window leaves the session on. Reopen to turn OFF.",
                );
            } else {
                self.text(TITLE, "Game Mode is OFF");
                self.text(TOGGLE, "Turn ON");
                self.config = db.manual_settings()?;
                let n = self
                    .config
                    .rules
                    .iter()
                    .filter(|r| r.active(native::now()))
                    .count();
                let persistent = self.config.startup_disabled.len();
                self.text(
                    SUMMARY,
                    &if n == 0 && persistent == 0 {
                        "Settings: choose Game Mode-only or Permanent cleanup.".into()
                    } else {
                        format!(
                            "{n} Game Mode rule(s) ready; {persistent} startup item(s) disabled."
                        )
                    },
                );
                self.text(HELP, "No game selection. Turn OFF when you finish.");
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.text(TITLE, "Recovery data needs attention");
            self.text(SUMMARY, &e);
            self.enable(TOGGLE, false);
        }
    }
    fn command(&mut self, id: u16) -> AppResult<()> {
        match id {
            TOGGLE => self.toggle()?,
            SETTINGS => self.panel(!self.expanded),
            SCAN => self.scan(),
            ALLOW_CLOSE => self.permit(Some(RuleAction::Close))?,
            REDUCE => self.permit(Some(RuleAction::ReduceLoad))?,
            KEEP => self.permit(None)?,
            PERMANENT => self.disable_startup()?,
            RESTORE_STARTUP => self.keep_startup()?,
            COPY_LIST => self.copy_toggle()?,
            REPORT => self.report()?,
            ADVANCED => runner::spawn_worker("--advanced", None)?,
            UPDATE => {
                if update::interactive(self.window)? {
                    unsafe {
                        DestroyWindow(self.window);
                    }
                }
            }
            ACK => {
                self.report()?;
                if confirm(self.window,"Acknowledge unresolved recovery WITHOUT claiming restoration?\n\nThis keeps current settings and the recovery record. It does not undo a close or restore unsaved work."){runner::spawn_worker("--acknowledge",None)?;}
            }
            _ => {}
        }
        Ok(())
    }
}
fn confirm(window: HWND, text: &str) -> bool {
    ui_common::confirm_dialog(window, "Game Mode permission", text)
}
fn info(window: HWND, text: &str) {
    ui_common::info_dialog(window, "Process Optimizer", text);
}

unsafe extern "system" fn proc(window: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if msg == WM_CTLCOLORSTATIC {
        let dc = w as HDC;
        SetTextColor(dc, GetSysColor(COLOR_WINDOWTEXT));
        SetBkColor(dc, GetSysColor(COLOR_WINDOW));
        return GetSysColorBrush(COLOR_WINDOW) as LRESULT;
    }
    if msg == WM_NCCREATE {
        let create = &*(l as *const CREATESTRUCTW);
        SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
    }
    if msg == WM_TIMER && w == 2 {
        DestroyWindow(window);
        return 0;
    }
    if msg == WM_DESTROY {
        KillTimer(window, 1);
        PostQuitMessage(0);
        return 0;
    }
    let raw = GetWindowLongPtrW(window, GWLP_USERDATA) as *const RefCell<State>;
    if !raw.is_null() {
        if let Ok(mut s) = (*raw).try_borrow_mut() {
            match msg {
                WM_CREATE => {
                    s.window = window;
                    if let Err(e) = s.init() {
                        s.error = Some(e);
                        return -1;
                    }
                    return 0;
                }
                WM_DRAWITEM => {
                    let item = &*(l as *const AppDrawItem);
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
                    let result = s.command(id);
                    if matches!(
                        id,
                        KEEP | REDUCE | ALLOW_CLOSE | RESTORE_STARTUP | PERMANENT
                    ) {
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
                }
                WM_SIZE => {
                    s.minimized = w == SIZE_MINIMIZED as usize;
                    if !s.minimized {
                        s.layout();
                    }
                    return 0;
                }
                WM_TIMER => {
                    s.poll();
                    return 0;
                }
                WM_DPICHANGED => {
                    let r = &*(l as *const RECT);
                    SetWindowPos(
                        window,
                        null_mut(),
                        r.left,
                        r.top,
                        r.right - r.left,
                        r.bottom - r.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    s.fonts();
                    s.layout();
                    return 0;
                }
                _ => {}
            }
        }
    }
    DefWindowProcW(window, msg, w, l)
}
fn run_inner(smoke: bool, preview: bool) -> AppResult<()> {
    let state = Box::new(RefCell::new(State::new(smoke)?));
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(null());
        let name = wide("ProcessOptimizerSimpleGameMode");
        let mut class: WNDCLASSW = zeroed();
        class.hInstance = instance;
        class.lpszClassName = name.as_ptr();
        class.lpfnWndProc = Some(proc);
        class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        class.hbrBackground = (COLOR_WINDOW + 1) as HBRUSH;
        if RegisterClassW(&class) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let dpi = GetDpiForSystem().max(96);
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let mut r = RECT {
            left: 0,
            top: 0,
            right: (600 * dpi / 96) as i32,
            bottom: (336 * dpi / 96) as i32,
        };
        AdjustWindowRectEx(&mut r, style, 0, WS_EX_CONTROLPARENT);
        let window = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            name.as_ptr(),
            wide("Game Mode — Process Optimizer").as_ptr(),
            style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            r.right - r.left,
            r.bottom - r.top,
            null_mut(),
            null_mut(),
            instance,
            (&*state as *const RefCell<State>).cast(),
        );
        if window.is_null() {
            return Err(state
                .borrow_mut()
                .error
                .take()
                .unwrap_or_else(|| std::io::Error::last_os_error().to_string()));
        }
        if smoke && !preview {
            // Only six primary controls, two actionable. No game-path field or browse button.
            let mut s = state.borrow_mut();
            if s.widgets.len() != 24 {
                return Err("Incomplete simple UI.".into());
            }
            if GetWindowLongW(s.h(APPS), GWL_STYLE) as u32 & LBS_OWNERDRAWFIXED as u32 == 0 {
                return Err("App list lost essential-process color support.".into());
            }
            for id in PANEL {
                if GetWindowLongW(s.h(id), GWL_STYLE) as u32 & WS_VISIBLE != 0 {
                    return Err("Settings leaked into the default screen.".into());
                }
            }
            s.panel(true);
            s.panel(false);
            drop(s);
            DestroyWindow(window);
            return Ok(());
        }
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        if preview {
            SetTimer(window, 2, 7000, None);
        }
        ui_common::run_dialog_message_loop(window)
    }
}
pub fn run() -> AppResult<()> {
    if process::is_elevated().map_err(|e| e.message)? {
        return Err("Run normally, not as administrator.".into());
    }
    run_inner(false, false)
}
pub fn smoke() -> AppResult<()> {
    run_inner(true, false)
}

pub fn preview() -> AppResult<()> {
    run_inner(true, true)
}
