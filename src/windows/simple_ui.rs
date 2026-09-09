//! Small native front door: On/Off first, app permissions behind Settings.
use super::{gpu, manual as native, process, runner, wide};
use crate::{
    gpu::busiest_engine,
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
type Slot<T> = Arc<Mutex<Option<AppResult<T>>>>;

struct Choice {
    path: String,
    name: String,
    id: Option<Identity>,
    protected: bool,
    gpu: Option<f64>,
}
struct State {
    window: HWND,
    widgets: BTreeMap<u16, HWND>,
    font: HFONT,
    heading: HFONT,
    expanded: bool,
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
        // Avoid repainting unchanged status text on every poll.
        unsafe {
            let n = GetWindowTextLengthW(self.h(id)).max(0) as usize;
            let mut old = vec![0u16; n + 1];
            GetWindowTextW(self.h(id), old.as_mut_ptr(), old.len() as i32);
            if String::from_utf16_lossy(&old[..n]) != text {
                SetWindowTextW(self.h(id), wide(text).as_ptr());
            }
        }
    }
    fn enable(&self, id: u16, value: bool) {
        unsafe {
            windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow(
                self.h(id),
                i32::from(value),
            );
        }
    }
    fn control(&mut self, id: u16, class: &str, label: &str, style: u32) -> AppResult<()> {
        let h = unsafe {
            CreateWindowExW(
                if class == "LISTBOX" {
                    WS_EX_CLIENTEDGE
                } else {
                    0
                },
                wide(class).as_ptr(),
                wide(label).as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                0,
                0,
                10,
                10,
                self.window,
                id as usize as HMENU,
                GetModuleHandleW(null()),
                null(),
            )
        };
        if h.is_null() {
            return Err(format!(
                "Could not create control {id}: {}",
                std::io::Error::last_os_error()
            ));
        }
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
            "BACKGROUND APPS\r\nOnly apps you approve are changed. No game files to choose.",
            0,
        )?;
        self.control(SCAN, "BUTTON", "Refresh apps", WS_TABSTOP)?;
        self.control(
            APPS,
            "LISTBOX",
            "",
            WS_TABSTOP | WS_VSCROLL | WS_HSCROLL | LBS_NOTIFY as u32 | LBS_NOINTEGRALHEIGHT as u32,
        )?;
        for (id, label) in [
            (ALLOW_CLOSE, "Allow normal close"),
            (REDUCE, "Reduce background load"),
            (KEEP, "Keep / remove approval"),
            (REPORT, "Session details"),
            (ADVANCED, "Advanced tools"),
            (ACK, "Recovery options"),
        ] {
            self.control(id, "BUTTON", label, WS_TABSTOP)?;
        }
        self.control(
            GPU_OPTION,
            "BUTTON",
            "Experimental GPU priority (this activation only)",
            WS_TABSTOP | BS_AUTOCHECKBOX as u32,
        )?;
        self.control(DETAIL,"STATIC","Approvals expire after 30 days or a detected file change.\r\nOn runs cleanup once; new/reopened apps are not chased.\r\nOff restores settings, not closed apps or unsaved work.",0)?;
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
        let create = |size: u32, weight: i32| unsafe {
            CreateFontW(
                -((size * dpi / 96) as i32),
                0,
                0,
                0,
                weight,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                0,
                wide("Segoe UI").as_ptr(),
            )
        };
        let (old, old_heading) = (self.font, self.heading);
        self.font = create(15, 400);
        self.heading = create(28, 600);
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
                ShowWindow(self.h(id), if self.expanded { SW_SHOW } else { SW_HIDE });
            }
        }
        for id in [TITLE, SUBTITLE, TOGGLE, SUMMARY, HELP] {
            unsafe {
                ShowWindow(self.h(id), if self.expanded { SW_HIDE } else { SW_SHOW });
            }
        }
        self.text(SETTINGS, if self.expanded { "Done" } else { "Settings" });
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
            put(INTRO, 24, 22, inner - 140, 50);
            put(SCAN, w - 160, 28, 136, 32);
            put(APPS, 24, 84, inner, 204);
            let bw = (inner - 16) / 3;
            for (i, id) in [ALLOW_CLOSE, REDUCE, KEEP].iter().enumerate() {
                put(*id, 24 + i as i32 * (bw + 8), 300, bw, 48);
            }
            put(GPU_OPTION, 24, 362, inner, 28);
            put(DETAIL, 24, 401, inner, 72);
            for (i, id) in [REPORT, ADVANCED, ACK].iter().enumerate() {
                put(*id, 24 + i as i32 * (bw + 8), 482, bw, 44);
            }
            put(SETTINGS, (w - 124) / 2, 548, 124, 32);
        }
    }
    fn panel(&mut self, expanded: bool) {
        self.expanded = expanded;
        self.visibility();
        let dpi = unsafe { GetDpiForWindow(self.window) }.max(96);
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: (600 * dpi / 96) as i32,
            bottom: ((if expanded { 602 } else { 336 }) * dpi / 96) as i32,
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
        if expanded && !self.smoke {
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
        let mut choices = BTreeMap::new();
        for row in &self.snapshot.processes {
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
            });
            entry.protected |= row.protected_reason.is_some();
            if let Some(v) = busiest_engine(&row.gpu) {
                entry.gpu = Some(entry.gpu.unwrap_or(0.0).max(v));
            }
        }
        for rule in &self.config.rules {
            choices
                .entry(rule.app.path.clone())
                .or_insert_with(|| Choice {
                    path: rule.app.path.clone(),
                    name: rule.app.path.rsplit('\\').next().unwrap_or("App").into(),
                    id: None,
                    protected: false,
                    gpu: None,
                });
        }
        self.choices = choices
            .into_values()
            .filter(|c| !c.protected || self.config.rules.iter().any(|r| r.app.path == c.path))
            .collect();
        self.choices.sort_by_key(|c| c.name.to_lowercase());
        unsafe {
            SendMessageW(self.h(APPS), WM_SETREDRAW, 0, 0);
            SendMessageW(self.h(APPS), LB_RESETCONTENT, 0, 0);
        }
        for c in &self.choices {
            let status = self
                .config
                .rules
                .iter()
                .find(|r| r.app.path == c.path)
                .map(|r| {
                    if !r.active(native::now()) {
                        "Approval expired"
                    } else {
                        match r.action {
                            RuleAction::Close => "Normal close",
                            RuleAction::ReduceLoad => "Reduce load",
                        }
                    }
                })
                .unwrap_or("Keep");
            let value = c
                .gpu
                .map(|v| format!("  | GPU peak {v:.1}%"))
                .unwrap_or_default();
            let line = format!(
                "{}  — {}{}{}",
                c.name,
                status,
                if c.id.is_none() { " (not running)" } else { "" },
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
            SendMessageW(self.h(APPS), LB_SETHORIZONTALEXTENT, 1100, 0);
            SendMessageW(self.h(APPS), WM_SETREDRAW, 1, 0);
            InvalidateRect(self.h(APPS), null(), 1);
        }
        self.text(
            INTRO,
            "BACKGROUND APPS\r\nChoose once. Unknown and unapproved apps are kept.",
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
        let experimental = unsafe { SendMessageW(self.h(GPU_OPTION), 0x00F0, 0, 0) } == 1;
        if experimental&&!confirm(self.window,"Enable experimental GPU scheduling for this activation?\n\nIt is a scheduling preference, not GPU blocking. Driver support and gaming benefit remain unverified. Only approved Reduce load apps are eligible."){return Ok(());}
        unsafe {
            SendMessageW(self.h(GPU_OPTION), 0x00F1, 0, 0);
        }
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
            for id in [ALLOW_CLOSE, REDUCE, KEEP, GPU_OPTION, ADVANCED] {
                self.enable(id, idle);
            }
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
                self.text(
                    SUMMARY,
                    &if n == 0 {
                        "Choose background apps once in Settings.".into()
                    } else {
                        format!("{n} background-app permission(s) ready.")
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
            REPORT => self.report()?,
            ADVANCED => runner::spawn_worker("--advanced", None)?,
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
    unsafe {
        MessageBoxW(
            window,
            wide(text).as_ptr(),
            wide("Game Mode permission").as_ptr(),
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
        ) == IDYES
    }
}
fn info(window: HWND, text: &str) {
    unsafe {
        MessageBoxW(
            window,
            wide(text).as_ptr(),
            wide("Process Optimizer").as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
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
                WM_COMMAND => {
                    if let Err(e) = s.command((w & 0xffff) as u16) {
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
            if s.widgets.len() != 17 {
                return Err("Incomplete simple UI.".into());
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
        let mut message: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result < 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if result == 0 {
                break;
            }
            if IsDialogMessageW(window, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    Ok(())
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
