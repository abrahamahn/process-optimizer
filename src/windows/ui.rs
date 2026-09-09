use super::{gpu, process, runner, wide};
use crate::{
    gpu::{busiest_engine, dedicated_allocations},
    journal::Journal,
    model::*,
    policy,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    sync::{Arc, Mutex},
    thread,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{Controls::Dialogs::*, HiDpi::*, WindowsAndMessaging::*},
};

const GAME: u16 = 10;
const BROWSE: u16 = 11;
const USE_GAME: u16 = 12;
const REFRESH: u16 = 13;
const LIST: u16 = 20;
const GPU_STATUS: u16 = 21;
const DETAILS: u16 = 22;
const LOWER: u16 = 30;
const CLOSE: u16 = 31;
const FORCE: u16 = 32;
const PROTECT: u16 = 33;
const KEEP: u16 = 34;
const OPT_GPU: u16 = 40;
const OPT_CPU: u16 = 41;
const OPT_ECO: u16 = 42;
const OPT_MEMORY: u16 = 43;
const OPT_LAUNCH: u16 = 44;
const START: u16 = 50;
const RESTORE: u16 = 51;
const REPORT: u16 = 52;
const ACK: u16 = 53;
const SAVE: u16 = 54;
const STATUS: u16 = 60;
const BM_GETCHECK_MSG: u32 = 0x00F0;
const BM_SETCHECK_MSG: u32 = 0x00F1;

type SampleSlot = Arc<Mutex<Option<AppResult<Snapshot>>>>;
struct State {
    window: HWND,
    widgets: BTreeMap<u16, HWND>,
    settings: Settings,
    snapshot: Snapshot,
    actions: BTreeMap<u32, ApprovedAction>,
    game: Option<Identity>,
    sample: SampleSlot,
    sampling: bool,
    minimized: bool,
    font: HFONT,
    mono_font: HFONT,
    smoke: bool,
    init_error: Option<String>,
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe {
            if !self.font.is_null() {
                DeleteObject(self.font);
            }
            if !self.mono_font.is_null() {
                DeleteObject(self.mono_font);
            }
        }
    }
}

impl State {
    fn new(smoke: bool) -> AppResult<Self> {
        let mut settings = if smoke {
            Settings::default()
        } else {
            runner::database()?.settings()?
        };
        // Experimental GPU scheduling always requires fresh opt-in on opening the UI.
        settings.options.gpu_priority = false;
        settings.options.launch_game = false;
        Ok(Self {
            window: null_mut(),
            widgets: BTreeMap::new(),
            settings,
            snapshot: Snapshot::default(),
            actions: BTreeMap::new(),
            game: None,
            sample: Arc::new(Mutex::new(None)),
            sampling: false,
            minimized: false,
            font: null_mut(),
            mono_font: null_mut(),
            smoke,
            init_error: None,
        })
    }
    fn h(&self, id: u16) -> HWND {
        *self.widgets.get(&id).unwrap_or(&null_mut())
    }
    fn text(&self, id: u16, value: &str) {
        unsafe {
            SetWindowTextW(self.h(id), wide(value).as_ptr());
        }
    }
    fn checked(&self, id: u16) -> bool {
        unsafe { SendMessageW(self.h(id), BM_GETCHECK_MSG, 0, 0) == 1 }
    }
    fn set_check(&self, id: u16, value: bool) {
        unsafe {
            SendMessageW(self.h(id), BM_SETCHECK_MSG, usize::from(value), 0);
        }
    }
    fn edit_text(&self, id: u16) -> String {
        let length = unsafe { GetWindowTextLengthW(self.h(id)) }.clamp(0, 32767) as usize;
        let mut value = vec![0u16; length + 1];
        let n = unsafe { GetWindowTextW(self.h(id), value.as_mut_ptr(), value.len() as i32) }.max(0)
            as usize;
        String::from_utf16_lossy(&value[..n])
    }
    fn widget(&mut self, id: u16, class: &str, label: &str, style: u32) -> AppResult<()> {
        let h = unsafe {
            CreateWindowExW(
                if class == "EDIT" || class == "LISTBOX" {
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
                "Could not create native control {id}: {}",
                std::io::Error::last_os_error()
            ));
        }
        self.widgets.insert(id, h);
        Ok(())
    }
    fn initialize(&mut self) -> AppResult<()> {
        self.widget(
            100,
            "STATIC",
            "PROCESS OPTIMIZER  /  GPU-FIRST GAME SESSIONS",
            0,
        )?;
        self.widget(101, "STATIC", "Select the actual game. Explicitly select background processes; nothing is chosen or closed automatically.", 0)?;
        self.widget(GAME, "EDIT", "", WS_TABSTOP | ES_AUTOHSCROLL as u32)?;
        self.widget(BROWSE, "BUTTON", "Browse game .exe", WS_TABSTOP)?;
        self.widget(USE_GAME, "BUTTON", "Use selected as game", WS_TABSTOP)?;
        self.widget(REFRESH, "BUTTON", "Refresh GPU sample", WS_TABSTOP)?;
        self.widget(102, "STATIC", "PID       Application                 GPU peak    Dedicated MiB   Planned action / protection", 0)?;
        self.widget(
            LIST,
            "LISTBOX",
            "",
            WS_TABSTOP
                | WS_VSCROLL
                | WS_HSCROLL
                | LBS_EXTENDEDSEL as u32
                | LBS_NOINTEGRALHEIGHT as u32
                | LBS_NOTIFY as u32,
        )?;
        self.widget(
            GPU_STATUS,
            "STATIC",
            "GPU samples are read-only. '-' means unavailable/not observed, not a confirmed zero.",
            0,
        )?;
        for (id, label) in [
            (LOWER, "Lower priorities"),
            (CLOSE, "Close gracefully"),
            (FORCE, "Force terminate"),
            (PROTECT, "Protect / unprotect app"),
            (KEEP, "Clear selection plan"),
        ] {
            self.widget(id, "BUTTON", label, WS_TABSTOP)?;
        }
        for (id, label) in [
            (OPT_GPU, "GPU (experimental)"),
            (OPT_CPU, "CPU priority"),
            (OPT_ECO, "EcoQoS"),
            (OPT_MEMORY, "Memory priority"),
            (OPT_LAUNCH, "Launch adapter unavailable"),
        ] {
            self.widget(
                id,
                "BUTTON",
                label,
                WS_TABSTOP
                    | BS_AUTOCHECKBOX as u32
                    | if id == OPT_LAUNCH { WS_DISABLED } else { 0 },
            )?;
        }
        self.widget(DETAILS, "EDIT", "Select processes with Ctrl/Shift, then choose an action.\r\nClosing an app is not a memory snapshot. Unsaved work cannot be restored.\r\nLower GPU priority is an experimental scheduling hint, not a GPU usage cap or exclusive reservation.", ES_MULTILINE as u32 | ES_READONLY as u32 | ES_AUTOVSCROLL as u32 | WS_VSCROLL)?;
        for (id, label) in [
            (START, "START GAME SESSION"),
            (RESTORE, "Restore now"),
            (REPORT, "Show last report"),
            (ACK, "Keep current / clear warning"),
            (SAVE, "Save settings"),
        ] {
            self.widget(id, "BUTTON", label, WS_TABSTOP)?;
        }
        self.widget(
            STATUS,
            "STATIC",
            "No session active. The worker can restore settings even after this window is closed.",
            0,
        )?;
        self.text(GAME, &self.settings.game_path);
        for (id, checked) in [
            (OPT_GPU, self.settings.options.gpu_priority),
            (OPT_CPU, self.settings.options.cpu_priority),
            (OPT_ECO, self.settings.options.eco_qos),
            (OPT_MEMORY, self.settings.options.memory_priority),
            (OPT_LAUNCH, self.settings.options.launch_game),
        ] {
            self.set_check(id, checked);
        }
        self.update_fonts();
        self.layout();
        if !self.smoke {
            self.refresh();
            unsafe {
                SetTimer(self.window, 1, 1000, None);
            }
        }
        Ok(())
    }
    fn update_fonts(&mut self) {
        let dpi = unsafe { GetDpiForWindow(self.window) }.max(96);
        let height = -(((15 * dpi) / 96) as i32);
        let (old, old_mono) = (self.font, self.mono_font);
        self.font = unsafe {
            CreateFontW(
                height,
                0,
                0,
                0,
                400,
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
        self.mono_font = unsafe {
            CreateFontW(
                height,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                0,
                wide("Consolas").as_ptr(),
            )
        };
        for (id, h) in &self.widgets {
            unsafe {
                SendMessageW(
                    *h,
                    WM_SETFONT,
                    if *id == LIST || *id == 102 {
                        self.mono_font as usize
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
            if !old_mono.is_null() {
                DeleteObject(old_mono);
            }
        }
    }
    fn layout(&self) {
        let mut rect: RECT = unsafe { zeroed() };
        unsafe {
            GetClientRect(self.window, &mut rect);
        }
        let dpi = unsafe { GetDpiForWindow(self.window) }.max(96) as f64 / 96.0;
        let w = ((rect.right - rect.left) as f64 / dpi) as i32;
        let h = ((rect.bottom - rect.top) as f64 / dpi) as i32;
        let put = |id, x: i32, y: i32, width: i32, height: i32| unsafe {
            MoveWindow(
                self.h(id),
                (x as f64 * dpi) as i32,
                (y as f64 * dpi) as i32,
                (width.max(1) as f64 * dpi) as i32,
                (height.max(1) as f64 * dpi) as i32,
                1,
            );
        };
        let inner = w - 32;
        put(100, 16, 14, inner, 24);
        put(101, 16, 42, inner, 24);
        put(GAME, 16, 72, inner - 480, 30);
        put(BROWSE, w - 488, 72, 146, 30);
        put(USE_GAME, w - 334, 72, 162, 30);
        put(REFRESH, w - 164, 72, 148, 30);
        put(102, 16, 112, inner, 22);
        let list_height = (h - 440).max(180);
        put(LIST, 16, 136, inner, list_height);
        let y = 136 + list_height;
        put(GPU_STATUS, 16, y + 5, inner, 42);
        let button_w = (inner - 32) / 5;
        for (i, id) in [LOWER, CLOSE, FORCE, PROTECT, KEEP].iter().enumerate() {
            put(*id, 16 + i as i32 * (button_w + 8), y + 50, button_w, 32);
        }
        for (i, id) in [OPT_GPU, OPT_CPU, OPT_ECO, OPT_MEMORY, OPT_LAUNCH]
            .iter()
            .enumerate()
        {
            put(*id, 16 + i as i32 * (button_w + 8), y + 90, button_w, 26);
        }
        put(DETAILS, 16, y + 124, inner, (h - y - 224).max(70));
        let bottom = h - 84;
        for (i, id) in [START, RESTORE, REPORT, ACK, SAVE].iter().enumerate() {
            put(*id, 16 + i as i32 * (button_w + 8), bottom, button_w, 36);
        }
        put(STATUS, 16, h - 39, inner, 34);
    }
    fn refresh(&mut self) {
        if self.sampling {
            return;
        }
        self.sampling = true;
        self.text(
            GPU_STATUS,
            "Sampling GPU activity; no process or setting is being changed...",
        );
        let slot = self.sample.clone();
        thread::spawn(move || {
            let result = gpu::snapshot();
            if let Ok(mut slot) = slot.lock() {
                *slot = Some(result);
            }
        });
    }
    fn selected(&self) -> Vec<usize> {
        let count = unsafe { SendMessageW(self.h(LIST), LB_GETSELCOUNT, 0, 0) };
        if count <= 0 || count as usize > self.snapshot.processes.len() {
            return vec![];
        }
        let mut items = vec![0i32; count as usize];
        let actual = unsafe {
            SendMessageW(
                self.h(LIST),
                LB_GETSELITEMS,
                items.len(),
                items.as_mut_ptr() as LPARAM,
            )
        };
        if actual < 0 {
            return vec![];
        }
        items
            .into_iter()
            .take(actual as usize)
            .filter_map(|i| usize::try_from(i).ok())
            .filter(|i| *i < self.snapshot.processes.len())
            .collect()
    }
    fn repopulate(&self) {
        unsafe {
            SendMessageW(self.h(LIST), WM_SETREDRAW, 0, 0);
            SendMessageW(self.h(LIST), LB_RESETCONTENT, 0, 0);
        }
        for p in &self.snapshot.processes {
            let name: String = p.name.chars().take(26).collect();
            let gpu = busiest_engine(&p.gpu)
                .map(|v| format!("{v:5.1}%"))
                .unwrap_or_else(|| "     -".into());
            let memory = dedicated_allocations(&p.gpu)
                .map(|v| format!("{:8.1}", v as f64 / 1048576.0))
                .unwrap_or_else(|| "       -".into());
            let user_protected =
                self.settings.protected_paths.iter().any(|v| {
                    policy::normalized_path(v) == policy::normalized_path(&p.identity.path)
                });
            let action = if self
                .game
                .as_ref()
                .is_some_and(|g| policy::same_process(g, &p.identity))
            {
                "GAME".into()
            } else if user_protected {
                "USER PROTECTED".into()
            } else if let Some(reason) = &p.protected_reason {
                format!("PROTECTED: {reason}")
            } else if let Some(action) = self.actions.get(&p.identity.pid) {
                format!("{:?}", action.action)
            } else {
                "Keep (default)".into()
            };
            let row = format!(
                "{:7}  {:26}  {}    {}    {}",
                p.identity.pid, name, gpu, memory, action
            );
            unsafe {
                SendMessageW(self.h(LIST), LB_ADDSTRING, 0, wide(&row).as_ptr() as LPARAM);
            }
        }
        unsafe {
            SendMessageW(self.h(LIST), LB_SETHORIZONTALEXTENT, 1800, 0);
            SendMessageW(self.h(LIST), WM_SETREDRAW, 1, 0);
            InvalidateRect(self.h(LIST), null(), 1);
        }
    }
    fn update_settings(&mut self) {
        self.settings.game_path = self.edit_text(GAME).trim().to_string();
        if self.game.as_ref().is_some_and(|g| {
            policy::normalized_path(&g.path) != policy::normalized_path(&self.settings.game_path)
        }) {
            self.game = None;
        }
        self.settings.options.gpu_priority = self.checked(OPT_GPU);
        self.settings.options.cpu_priority = self.checked(OPT_CPU);
        self.settings.options.eco_qos = self.checked(OPT_ECO);
        self.settings.options.memory_priority = self.checked(OPT_MEMORY);
        self.settings.options.launch_game = false;
    }
    fn plan_preview(&self) {
        let mut text = String::from("APPROVAL PLAN — exact processes, this session only\r\n");
        for action in self.actions.values() {
            text.push_str(&format!(
                "{:?}: PID {}  {}\r\n",
                action.action, action.target.pid, action.target.path
            ));
        }
        text.push_str("\r\nA helper process exiting does not prove the entire application exited. Unselected/new helpers are never force-closed.\r\nGPU priority is not a usage cap. No documents, RAM contents, tabs or unsaved work are snapshotted. No automatic relaunch.");
        self.text(DETAILS, &text);
    }
    fn mark(&mut self, action: Option<ActionKind>) -> AppResult<()> {
        let selected = self.selected();
        if selected.is_empty() {
            return Err("Select background process rows first (Ctrl/Shift for multiple).".into());
        }
        // Validate the entire selection before changing any plan entry.
        if action.is_some() {
            for &index in &selected {
                let row = &self.snapshot.processes[index];
                if let Some(reason) = &row.protected_reason {
                    return Err(format!("{} is protected: {reason}", row.name));
                }
                if self
                    .game
                    .as_ref()
                    .is_some_and(|g| policy::same_process(g, &row.identity))
                {
                    return Err("The selected game is protected.".into());
                }
                if self.settings.protected_paths.iter().any(|p| {
                    policy::normalized_path(p) == policy::normalized_path(&row.identity.path)
                }) {
                    return Err("This application is in your protected list.".into());
                }
            }
        }
        for index in selected {
            let row = &self.snapshot.processes[index];
            if let Some(action) = action {
                self.actions.insert(
                    row.identity.pid,
                    ApprovedAction {
                        target: row.identity.clone(),
                        action,
                    },
                );
            } else {
                self.actions.remove(&row.identity.pid);
            }
        }
        self.repopulate();
        self.plan_preview();
        Ok(())
    }
    fn use_game(&mut self) -> AppResult<()> {
        let selected = self.selected();
        if selected.len() != 1 {
            return Err("Select exactly one actual game process.".into());
        }
        let row = &self.snapshot.processes[selected[0]];
        if let Some(reason) = &row.protected_reason {
            return Err(format!(
                "Select the actual game, not a protected launcher: {reason}"
            ));
        }
        self.game = Some(row.identity.clone());
        self.settings.game_path = row.identity.path.clone();
        self.actions.remove(&row.identity.pid);
        self.text(GAME, &row.identity.path);
        self.repopulate();
        self.plan_preview();
        Ok(())
    }
    fn protect(&mut self) -> AppResult<()> {
        let selected = self.selected();
        if selected.is_empty() {
            return Err("Select the application(s) to protect or unprotect.".into());
        }
        let mut paths: Vec<_> = selected
            .iter()
            .map(|i| self.snapshot.processes[*i].identity.path.clone())
            .filter(|p| !p.is_empty())
            .collect();
        paths.sort();
        paths.dedup();
        let all_protected = !paths.is_empty()
            && paths.iter().all(|p| {
                self.settings
                    .protected_paths
                    .iter()
                    .any(|v| policy::normalized_path(v) == policy::normalized_path(p))
            });
        if all_protected {
            if !confirm(self.window, "Remove your saved protection for these applications?\n\nBuilt-in system/game-support protections remain active. This does not approve any closure.") { return Ok(()); }
            self.settings.protected_paths.retain(|p| {
                !paths
                    .iter()
                    .any(|v| policy::normalized_path(v) == policy::normalized_path(p))
            });
        } else {
            for path in paths {
                self.actions.retain(|_, a| {
                    policy::normalized_path(&a.target.path) != policy::normalized_path(&path)
                });
                if !self
                    .settings
                    .protected_paths
                    .iter()
                    .any(|p| policy::normalized_path(p) == policy::normalized_path(&path))
                {
                    self.settings.protected_paths.push(path);
                }
            }
        }
        runner::database()?.save_settings(&self.settings)?;
        self.repopulate();
        self.plan_preview();
        Ok(())
    }
    fn browse(&mut self) -> AppResult<()> {
        let mut filename = vec![0u16; 32768];
        let filter: Vec<u16> = "Executables (*.exe)\0*.exe\0\0".encode_utf16().collect();
        let mut dialog: OPENFILENAMEW = unsafe { zeroed() };
        dialog.lStructSize = size_of::<OPENFILENAMEW>() as u32;
        dialog.hwndOwner = self.window;
        dialog.lpstrFilter = filter.as_ptr();
        dialog.lpstrFile = filename.as_mut_ptr();
        dialog.nMaxFile = filename.len() as u32;
        dialog.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
        if unsafe { GetOpenFileNameW(&mut dialog) } != 0 {
            let n = filename
                .iter()
                .position(|v| *v == 0)
                .unwrap_or(filename.len());
            self.settings.game_path = String::from_utf16_lossy(&filename[..n]);
            self.game = None;
            self.text(GAME, &self.settings.game_path);
        }
        Ok(())
    }
    fn start(&mut self) -> AppResult<()> {
        self.update_settings();
        if process::is_elevated().map_err(|e| e.message)? {
            return Err(
                "Close this elevated window and run the app normally, not as administrator.".into(),
            );
        }
        let mut db = runner::database()?;
        if db.active()?.is_some() {
            return Err("Restore/review the existing session before starting another.".into());
        }
        if self.actions.is_empty() {
            return Err(
                "Choose at least one background process action. There is no automatic kill list."
                    .into(),
            );
        }
        if self
            .actions
            .values()
            .any(|a| a.action == ActionKind::LowerPriorities)
            && !(self.settings.options.gpu_priority
                || self.settings.options.cpu_priority
                || self.settings.options.eco_qos
                || self.settings.options.memory_priority)
        {
            return Err(
                "Choose at least one priority policy, or use a close action instead.".into(),
            );
        }
        if self.game.is_none() {
            return Err("Select the actual running game using 'Use selected as game'. Browsing a path is not lifetime approval.".into());
        }
        let mut plan = Plan {
            game_path: self.settings.game_path.clone(),
            game: self.game.clone(),
            actions: self.actions.values().cloned().collect(),
            protected_paths: self.settings.protected_paths.clone(),
            options: self.settings.options.clone(),
            consent: true,
            force_consent: true,
            experimental_consent: true,
        };
        policy::validate(&plan)?;
        self.plan_preview();
        let summary = format!("Start a session for:\n{}\n\nI have reviewed these {} explicitly listed processes and confirm they are unnecessary for my game, voice, accessibility and device operation. Apply their listed actions?\n\nClosing applications may lose unsaved work. A settings journal cannot restore that work. GPU priority is experimental, not a GPU lock; it has no established gaming-performance benefit on your machine.\n\nNo security, network, Bluetooth, audio or Windows service settings will be changed.", plan.game_path, plan.actions.len());
        if !confirm(self.window, &summary) {
            return Ok(());
        }
        plan.experimental_consent = !plan.options.gpu_priority || confirm(self.window,
            "EXPERIMENTAL GPU SCHEDULING\n\nEnable the separately selected GPU scheduling policy for this session? It is not a GPU quota. Performance benefit on your hardware is unverified. Original values must be readable before any change.");
        if !plan.experimental_consent {
            return Ok(());
        }
        let forced: Vec<_> = plan
            .actions
            .iter()
            .filter(|a| a.action == ActionKind::ForceClose)
            .collect();
        plan.force_consent = forced.is_empty() || confirm(self.window, &format!("SEPARATE FORCE-TERMINATION APPROVAL\n\nDirectly force-terminate these exact processes? No normal save/close workflow will be completed.\n{}\n\nThis can permanently lose unsaved work. There is no memory snapshot. Permission applies to this session only.", forced.iter().map(|a| format!("PID {} — {}", a.target.pid, a.target.path)).collect::<Vec<_>>().join("\n")));
        if !plan.force_consent {
            return Ok(());
        }
        policy::validate(&plan)?;
        let session = Session::new(runner::fresh_id(), plan);
        db.save_settings(&self.settings)?;
        db.create(&session)?;
        if let Err(e) = runner::spawn_worker("--session", Some(&session.id)) {
            let mut failed = session;
            failed.stage = Stage::Restored;
            failed.note(format!("Worker launch failed before mutation: {e}"));
            db.save(&failed)?;
            return Err(e);
        }
        self.actions.clear();
        self.repopulate();
        self.text(
            STATUS,
            "Worker starting. No application is touched until the actual game is running.",
        );
        Ok(())
    }
    fn restore(&self) -> AppResult<()> {
        let db = runner::database()?;
        let Some(session) = db.active()? else {
            self.text(STATUS, "No unfinished settings changes.");
            return Ok(());
        };
        db.request_stop(&session.id)?;
        if session
            .worker
            .as_ref()
            .is_some_and(|w| matches!(process::alive(w), Ok(true)))
        {
            self.text(STATUS, "Restore requested. The worker will stop applying changes and restore recorded settings.");
            Ok(())
        } else {
            runner::spawn_worker("--recover", None)
        }
    }
    fn report(&self) -> AppResult<()> {
        let db = runner::database()?;
        if let Some(session) = db.latest()? {
            let report = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
            self.text(DETAILS, &report.replace('\n', "\r\n"));
        } else {
            self.text(DETAILS, "No game session has been recorded yet.");
        }
        Ok(())
    }
    fn command(&mut self, id: u16, notification: u16) -> AppResult<()> {
        if id == LIST && notification == LBN_SELCHANGE as u16 {
            if let Some(index) = self.selected().first() {
                let row = &self.snapshot.processes[*index];
                self.text(DETAILS, &format!("{}\r\nPID: {} | Created: {} | Session: {}\r\n{}\r\n\r\nGPU adapter/engine detail:\r\n{}", row.name, row.identity.pid, row.identity.created, row.identity.session_id, row.identity.path, serde_json::to_string_pretty(&row.gpu).unwrap_or_default().replace('\n', "\r\n")));
            }
            return Ok(());
        }
        if notification != 0 {
            return Ok(());
        }
        match id {
            REFRESH => { self.refresh(); Ok(()) }, BROWSE => self.browse(), USE_GAME => self.use_game(),
            LOWER => self.mark(Some(ActionKind::LowerPriorities)), CLOSE => self.mark(Some(ActionKind::Close)), FORCE => self.mark(Some(ActionKind::ForceClose)), KEEP => self.mark(None), PROTECT => self.protect(),
            START => self.start(), RESTORE => self.restore(), REPORT => self.report(),
            ACK => {
                if confirm(self.window, "Keep the current values for all unresolved recovery items?\n\nThis discards automatic restoration for those items and clears the warning. It is NOT a successful restore. Review the report first.") { runner::spawn_worker("--acknowledge", None) } else { Ok(()) }
            }
            SAVE => { self.update_settings(); runner::database()?.save_settings(&self.settings)?; self.text(STATUS, "Settings saved. Closure approval is session-only. Experimental GPU priority needs fresh opt-in when reopening the UI."); Ok(()) }
            _ => Ok(()),
        }
    }
    fn poll(&mut self) {
        let ready = self.sample.lock().ok().and_then(|mut slot| slot.take());
        if let Some(result) = ready {
            self.sampling = false;
            match result {
                Ok(snapshot) => {
                    self.snapshot = snapshot;
                    self.actions.retain(|_, action| {
                        self.snapshot
                            .processes
                            .iter()
                            .any(|p| policy::same_process(&p.identity, &action.target))
                    });
                    self.text(GPU_STATUS, &self.snapshot.gpu_status);
                    self.repopulate();
                }
                Err(e) => self.text(GPU_STATUS, &format!("GPU sample unavailable: {e}")),
            }
        }
        if self.minimized {
            return;
        }
        if let Ok(db) = runner::database() {
            match db.latest() {
                Ok(Some(s)) => {
                    let worker_alive = s
                        .worker
                        .as_ref()
                        .is_some_and(|w| matches!(process::alive(w), Ok(true)));
                    let status = if !s.stage.finished()
                        && !worker_alive
                        && s.stage != Stage::Pending
                    {
                        format!("{:?}: worker not running. Click Restore now; original values remain in the recovery journal.", s.stage)
                    } else {
                        format!(
                            "Session {:?} | {} settings recorded | {} close attempts. {}",
                            s.stage,
                            s.changes.len(),
                            s.closed.len(),
                            if s.stage == Stage::RecoveryNeeded {
                                "Review conflicts before continuing."
                            } else {
                                "Closing/relaunching apps is not memory restoration."
                            }
                        )
                    };
                    self.text(STATUS, &status);
                }
                Err(e) => self.text(
                    STATUS,
                    &format!("Journal warning — no changes will be attempted: {e}"),
                ),
                _ => {}
            }
        }
    }
}

fn confirm(owner: HWND, message: &str) -> bool {
    unsafe {
        MessageBoxW(
            owner,
            wide(message).as_ptr(),
            wide("Process Optimizer — explicit approval").as_ptr(),
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
        ) == IDYES
    }
}
pub fn error(message: &str) {
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(message).as_ptr(),
            wide("Process Optimizer").as_ptr(),
            MB_OK | MB_ICONWARNING,
        );
    }
}

unsafe extern "system" fn window_proc(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    if message == WM_CREATE {
        let create = &*(l as *const CREATESTRUCTW);
        SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
    }
    if message == WM_CLOSE {
        DestroyWindow(window);
        return 0;
    }
    if message == WM_DESTROY {
        KillTimer(window, 1);
        PostQuitMessage(0);
        return 0;
    }
    if message == WM_NCDESTROY {
        SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        return DefWindowProcW(window, message, w, l);
    }
    let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *const RefCell<State>;
    if !pointer.is_null() {
        // Native messages re-enter this callback: never alias mutable State.
        if let Ok(mut state) = (*pointer).try_borrow_mut() {
            match message {
                WM_CREATE => {
                    state.window = window;
                    if let Err(e) = state.initialize() {
                        state.init_error = Some(e);
                        return -1;
                    }
                    return 0;
                }
                WM_SIZE => {
                    state.minimized = w == SIZE_MINIMIZED as usize;
                    if !state.minimized {
                        state.layout();
                    }
                    return 0;
                }
                WM_DPICHANGED => {
                    let rect = &*(l as *const RECT);
                    SetWindowPos(
                        window,
                        null_mut(),
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    state.update_fonts();
                    state.layout();
                    return 0;
                }
                WM_TIMER => {
                    state.poll();
                    return 0;
                }
                WM_COMMAND => {
                    if let Err(e) = state.command((w & 0xffff) as u16, ((w >> 16) & 0xffff) as u16)
                    {
                        error(&e);
                    }
                    return 0;
                }
                WM_GETMINMAXINFO => {
                    let minmax = &mut *(l as *mut MINMAXINFO);
                    let dpi = GetDpiForWindow(window).max(96);
                    minmax.ptMinTrackSize.x = (960 * dpi / 96) as i32;
                    minmax.ptMinTrackSize.y = (760 * dpi / 96) as i32;
                    return 0;
                }
                _ => {}
            }
        }
    }
    DefWindowProcW(window, message, w, l)
}

fn run_inner(smoke: bool) -> AppResult<()> {
    let state = Box::new(RefCell::new(State::new(smoke)?));
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(null());
        let name = wide("ProcessOptimizerNativeWindow");
        let mut class: WNDCLASSW = zeroed();
        class.hInstance = instance;
        class.lpszClassName = name.as_ptr();
        class.lpfnWndProc = Some(window_proc);
        class.hCursor = LoadCursorW(null_mut(), IDC_ARROW);
        class.hbrBackground = (COLOR_WINDOW + 1) as HBRUSH;
        if RegisterClassW(&class) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let dpi = GetDpiForSystem().max(96);
        let window = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            name.as_ptr(),
            wide("Process Optimizer — GPU-first game session").as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            (1120 * dpi / 96) as i32,
            (850 * dpi / 96) as i32,
            null_mut(),
            null_mut(),
            instance,
            (&*state as *const RefCell<State>).cast(),
        );
        if window.is_null() {
            return Err(state
                .borrow_mut()
                .init_error
                .take()
                .unwrap_or_else(|| std::io::Error::last_os_error().to_string()));
        }
        if smoke {
            let controls = state.borrow().widgets.len();
            DestroyWindow(window);
            if controls < 25 {
                return Err(format!("Incomplete UI: {controls} controls."));
            }
            return Ok(());
        }
        ShowWindow(window, SW_SHOW);
        UpdateWindow(window);
        let mut message: MSG = zeroed();
        loop {
            let status = GetMessageW(&mut message, null_mut(), 0, 0);
            if status == -1 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            if status == 0 {
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
        return Err("Run this app normally, not as administrator.".into());
    }
    run_inner(false)
}
pub fn smoke() -> AppResult<()> {
    run_inner(true)
}
