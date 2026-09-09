"""Reviewed one-off transformation of the formatted e9ea5fa source baseline.
Fail on source drift. Used only on the validation branch; removed before delivery.
"""
from pathlib import Path
import re


def edit(path, old, new, count=1):
    p=Path(path); s=p.read_text(encoding='utf-8')
    if s.count(old)!=count: raise RuntimeError(f'{path}: expected {count} matches for {old[:100]!r}, got {s.count(old)}')
    p.write_text(s.replace(old,new),encoding='utf-8',newline='\n')


def span(path, start, end, body):
    p=Path(path);s=p.read_text(encoding='utf-8')
    a=s.index(start);b=s.index(end,a)
    p.write_text(s[:a]+body+s[b:],encoding='utf-8',newline='\n')

edit('src/model.rs','pub const SCHEMA_VERSION: u32 = 1;','pub const SCHEMA_VERSION: u32 = 2;')
edit('src/model.rs','pub struct Identity {','#[serde(deny_unknown_fields)]\npub struct Identity {')
edit('src/model.rs','    pub session_id: u32,\n}', '''    pub session_id: u32,
    #[serde(default)]
    pub provenance: Option<Provenance>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    pub owner_sid: String,
    pub logon_id: u64,
    pub image_file_id: String,
}''')
for struct in ['ApprovedAction','Options','Settings','Plan','Session']:
    edit('src/model.rs',f'pub struct {struct} {{',f'#[serde(deny_unknown_fields)]\npub struct {struct} {{')
edit('src/model.rs','    pub force_consent: bool,','    pub force_consent: bool,\n    #[serde(default)]\n    pub experimental_consent: bool,')
edit('src/model.rs','            gpu_priority: true,','            gpu_priority: false,')
edit('src/model.rs','    KeptOpen,\n','    KeptOpen,\n    RequestedPending,\n')
for path in ['src/engine.rs','src/policy.rs']:
    edit(path,'            session_id: 1,','            session_id: 1,\n            provenance: None,')
for path in ['src/engine.rs','src/policy.rs','src/journal.rs','tests/windows_contract.rs','src/windows/ui.rs']:
    p=Path(path);s=p.read_text(encoding='utf-8')
    s,n=re.subn(r'(\n([ \t]*)force_consent: [^\n]+,\n)',r'\1\2experimental_consent: true,\n',s)
    if n!=1: raise RuntimeError(f'{path}: expected one Plan initializer, got {n}')
    p.write_text(s,encoding='utf-8',newline='\n')
edit('src/engine.rs','            options: Options::default(),','            options: Options { gpu_priority: true, ..Options::default() },')
edit('src/policy.rs','        && a.session_id == b.session_id','        && a.session_id == b.session_id\n        && a.provenance == b.provenance')
edit('src/policy.rs','plan.actions.len() > 128','plan.actions.len() > 32')
edit('src/policy.rs','At most 128 explicitly selected processes per session.','At most 32 explicitly selected processes per session.')
edit('src/policy.rs','    let mut pids = HashSet::new();', '''    if serde_json::to_vec(plan).map_err(|e| e.to_string())?.len() > 256 * 1024 {
        return Err("Approval plan exceeds the 256 KiB limit.".into());
    }
    if plan.options.launch_game {
        return Err("Automatic game launch is not supported in this version. Start the game normally and select its running process.".into());
    }
    let lower = plan.actions.iter().any(|a| a.action == ActionKind::LowerPriorities);
    if lower && !(plan.options.gpu_priority || plan.options.cpu_priority || plan.options.eco_qos || plan.options.memory_priority) {
        return Err("No scheduling policy was selected.".into());
    }
    if lower && plan.options.gpu_priority && !plan.experimental_consent {
        return Err("Experimental GPU scheduling needs separate session approval.".into());
    }
    let mut pids = HashSet::new();''')
edit('src/policy.rs','        if !pids.insert(item.target.pid) {', '''        let image = normalized_path(&item.target.path);
        if reserved_name(image.rsplit('\\\\').next().unwrap_or("")) {
            return Err("The plan contains protected game, device or system infrastructure.".into());
        }
        if !pids.insert(item.target.pid) {''')
edit('src/policy.rs','            | "process-optimizer.exe"','''            | "process-optimizer.exe"
            | "ghelper.exe" | "g-helper.exe" | "armourycrate.exe" | "armourycrate.service.exe"
            | "asusoptimization.exe" | "asusframework.exe" | "asusfancontrolservice.exe"
            | "narrator.exe" | "nvda.exe" | "jfw.exe" | "osk.exe" | "magnify.exe"''')
edit('src/engine.rs','    policy::validate(&s.plan)?;\n    s.stage = Stage::Preparing;', '''    if s.schema != SCHEMA_VERSION || s.stage != Stage::Pending || !s.changes.is_empty() || !s.closed.is_empty() {
        return Err("Only a fresh, current-schema session may apply changes; recover existing records instead.".into());
    }
    policy::validate(&s.plan)?;
    for action in &s.plan.actions {
        b.authorize(&action.target).map_err(|e| format!("Preflight rejected PID {}: {}", action.target.pid, e.message))?;
    }
    s.stage = Stage::Preparing;''')
edit('src/engine.rs','''        if let Err(e) = b.authorize(&action.target) {
            s.note(format!("Skipped {}: {}", action.target.pid, e.message));
            j.save(s)?;
            continue;
        }''','''        b.authorize(&action.target).map_err(|e| format!("Application stopped before further mutations: {}", e.message))?;''')
edit('src/engine.rs','                    let before = match b.read(&action.target, property) {','''                    b.authorize(&action.target).map_err(|e| e.message)?;
                    let before = match b.read(&action.target, property) {''')
edit('src/engine.rs','                    match b.write(&action.target, &desired) {','''                    b.authorize(&action.target).map_err(|e| e.message)?;
                    // Observe external changes again immediately before writing. This is
                    // conservative comparison, not an atomic OS compare-and-swap.
                    if b.read(&action.target, property).map_err(|e| e.message)? != s.changes[idx].before {
                        s.changes[idx].state = ChangeState::Conflict;
                        s.changes[idx].detail = "Value changed after approval; no setting was written.".into();
                        j.save(s)?;
                        return Err("A competing setting change invalidated the plan.".into());
                    }
                    match b.write(&action.target, &desired) {''')
edit('src/engine.rs','                match b.close(&action.target, force, s.plan.options.close_timeout_ms) {','''                b.authorize(&action.target).map_err(|e| e.message)?;
                match b.close(&action.target, force, s.plan.options.close_timeout_ms) {''')
edit('src/engine.rs','    s.stage = if s.changes.iter().all(|c| c.state.resolved()) {','''    let uncertain_close = s.closed.iter().any(|c| matches!(c.state, CloseState::IntentRecorded | CloseState::RequestedPending));
    s.stage = if s.changes.iter().all(|c| c.state.resolved()) && !uncertain_close {''')
edit('src/engine.rs','    fn protected_targets_are_skipped() {','    fn protected_targets_reject_preflight() {')
edit('src/engine.rs','''        b.denied = true;
        apply(&mut s, &mut b, &mut j).unwrap();''','''        b.denied = true;
        assert!(apply(&mut s, &mut b, &mut j).is_err());''')
edit('src/journal.rs','        c.busy_timeout(Duration::from_secs(3))', '''        let version: u32 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        if version > 1 { return Err("Unsupported database schema; preserve this store for the matching application version.".into()); }
        c.busy_timeout(Duration::from_secs(3))''')
edit('src/journal.rs','        Ok(Self { connection: c })','''        c.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS one_unfinished_session ON sessions(finished) WHERE finished=0; PRAGMA user_version=1;").map_err(|e| e.to_string())?;
        Ok(Self { connection: c })''')
edit('src/journal.rs',r'''        let mut s = Settings::default();
        s.game_path = r"C:\게임\test.exe".into();''',r'''        let s = Settings { game_path: r"C:\게임\test.exe".into(), ..Settings::default() };''')
edit('src/windows/process.rs','pub fn identity_from_handle','fn identity_from_handle')
edit('src/windows/process.rs','    Ok(Identity {\n        pid,','''    let path = String::from_utf16_lossy(&buffer[..length as usize]);
    let provenance = Some(Provenance { owner_sid: token_sid(h)?, logon_id: logon_id(h)?, image_file_id: image_file_id(&path)? });
    Ok(Identity {
        pid,''')
edit('src/windows/process.rs','        path: String::from_utf16_lossy(&buffer[..length as usize]),\n        session_id: session,','        path,\n        session_id: session,\n        provenance,')
edit('src/windows/process.rs','                    session_id: u32::MAX,','                    session_id: u32::MAX,\n                    provenance: None,')
idx='pub fn current_sid() -> NativeResult<String> {'
edit('src/windows/process.rs',idx,'''fn logon_id(process: HANDLE) -> NativeResult<u64> {
    use windows_sys::Win32::Security::{TokenStatistics, TOKEN_STATISTICS};
    let mut raw = null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut raw) } == 0 { return Err(last_fault("OpenProcessToken")); }
    let token = Handle(raw);
    let mut info: TOKEN_STATISTICS = unsafe { zeroed() };
    let mut bytes = 0;
    if unsafe { GetTokenInformation(token.0, TokenStatistics, (&mut info as *mut TOKEN_STATISTICS).cast(), size_of::<TOKEN_STATISTICS>() as u32, &mut bytes) } == 0 {
        return Err(last_fault("TokenStatistics"));
    }
    Ok((u64::from(info.AuthenticationId.HighPart as u32) << 32) | u64::from(info.AuthenticationId.LowPart))
}

pub fn image_file_id(path: &str) -> NativeResult<String> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{GetFileInformationByHandleEx, FileIdInfo, FILE_ID_INFO};
    let file = std::fs::OpenOptions::new().access_mode(0x80).share_mode(7).open(path)
        .map_err(|e| Fault::new(FaultKind::Denied, format!("Image identity: {e}")))?;
    let mut info: FILE_ID_INFO = unsafe { zeroed() };
    if unsafe { GetFileInformationByHandleEx(file.as_raw_handle(), FileIdInfo, (&mut info as *mut FILE_ID_INFO).cast(), size_of::<FILE_ID_INFO>() as u32) } == 0 {
        return Err(last_fault("Image file identifier"));
    }
    Ok(format!("{:016x}:{:02x?}", info.VolumeSerialNumber, info.FileId.Identifier))
}

'''+idx)
edit('src/windows/process.rs','        let mut critical = 0;','''        let mut protection = PROCESS_PROTECTION_LEVEL_INFORMATION { ProtectionLevel: 0 };
        if unsafe { GetProcessInformation(h, ProcessProtectionLevelInfo, (&mut protection as *mut PROCESS_PROTECTION_LEVEL_INFORMATION).cast(), size_of::<PROCESS_PROTECTION_LEVEL_INFORMATION>() as u32) } == 0 {
            return Err(last_fault("Cannot establish process protection level"));
        }
        if protection.ProtectionLevel != PROTECTION_LEVEL_NONE { return Err(Fault::new(FaultKind::Denied, "Protected process.")); }
        let mut critical = 0;''')
edit('src/windows/process.rs','''    fn read(&mut self, id: &Identity, property: Property) -> NativeResult<Value> {
        let h = self.checked(id, 0)?;''','''    fn read(&mut self, id: &Identity, property: Property) -> NativeResult<Value> {
        let rights = if property == Property::GpuPriority { PROCESS_QUERY_INFORMATION } else { 0 };
        let h = self.checked(id, rights)?;''')
edit('src/windows/process.rs','        self.checked(id, 0).map(|_| ())','''        if self.game.is_some() {
            let settings = super::runner::database().and_then(|db| db.settings())
                .map_err(|e| Fault::new(FaultKind::Denied, format!("Cannot revalidate user protection: {e}")))?;
            for path in settings.protected_paths {
                if !self.protected_paths.contains(&path) { self.protected_paths.push(path); }
            }
        }
        self.checked(id, 0).map(|_| ())''')
span('src/windows/process.rs','    fn close(&mut self, id: &Identity, force: bool, timeout_ms: u32)', '\n}\n\npub struct SessionLock', '''    fn close(&mut self, id: &Identity, force: bool, timeout_ms: u32) -> NativeResult<CloseState> {
        self.authorize(id)?;
        if force {
            // Direct force is a distinct reviewed action, never a WM_CLOSE fallback.
            let h = self.checked(id, PROCESS_TERMINATE)?;
            if self.cancelled() { return Ok(CloseState::KeptOpen); }
            if unsafe { TerminateProcess(h.0, 1) } == 0 { return Err(last_fault("TerminateProcess")); }
            return if unsafe { WaitForSingleObject(h.0, 3000) } == WAIT_OBJECT_0 { Ok(CloseState::Terminated) }
                else { Err(Fault::new(FaultKind::Other, "Termination requested; exit remains unconfirmed.")) };
        }
        let h = self.checked(id, 0)?;
        struct Context { pid: u32, windows: Vec<HWND> }
        unsafe extern "system" fn callback(window: HWND, data: LPARAM) -> i32 {
            let ctx = &mut *(data as *mut Context);
            let mut pid = 0;
            GetWindowThreadProcessId(window, &mut pid);
            if pid == ctx.pid && ctx.windows.len() < 16 { ctx.windows.push(window); }
            i32::from(ctx.windows.len() < 16)
        }
        let mut context = Context { pid: id.pid, windows: Vec::new() };
        unsafe { EnumWindows(Some(callback), (&mut context as *mut Context) as LPARAM); }
        if context.windows.is_empty() { return Ok(CloseState::KeptOpen); }
        let mut requested = false;
        for window in context.windows {
            if self.cancelled() { break; }
            let mut owner = 0;
            unsafe { GetWindowThreadProcessId(window, &mut owner); }
            if owner != id.pid || unsafe { WaitForSingleObject(h.0, 0) } == WAIT_OBJECT_0 { continue; }
            // Timeout may mean delivery already occurred: never mark it safely undone.
            requested = true;
            unsafe { windows_sys::Win32::UI::WindowsAndMessaging::SendMessageTimeoutW(window, WM_CLOSE, 0, 0, 0x0002 | 0x0020, 1000, null_mut()); }
        }
        let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_ms.clamp(1000, 15000)));
        while Instant::now() < deadline {
            if unsafe { WaitForSingleObject(h.0, 100) } == WAIT_OBJECT_0 { return Ok(CloseState::ClosedGracefully); }
            if self.cancelled() { break; }
        }
        Ok(if requested { CloseState::RequestedPending } else { CloseState::KeptOpen })
    }''')
edit('src/windows/process.rs',', PostMessageW, WM_CLOSE',', WM_CLOSE')
p=Path('src/windows/process.rs');s=p.read_text(encoding='utf-8');s=s[:s.index('pub struct SessionLock')]+'''/// A file-sharing lock coordinates every session using this per-user store,
/// including different Windows interactive sessions. The filename is not ownership.
pub struct SessionLock { _file: std::fs::File }
impl SessionLock {
    pub fn acquire() -> AppResult<Self> { Self::acquire_at(&super::runner::state_dir()?.join("controller.lock")) }
    pub fn acquire_at(path: &std::path::Path) -> AppResult<Self> {
        use std::os::windows::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).share_mode(0).open(path)
            .map_err(|e| format!("A controller owns this store or the lock cannot be opened: {e}"))?;
        Ok(Self { _file: file })
    }
}
''';p.write_text(s,encoding='utf-8',newline='\n')
span('src/windows/runner.rs','fn find_game(', '\npub fn run(', '''fn attach_or_launch(plan: &Plan) -> AppResult<Identity> {
    let game = plan.game.as_ref().ok_or("Start the actual game normally, refresh, and select its exact running process.")?;
    if plan.options.launch_game { return Err("Automatic launcher tracking is not implemented; use an existing game process.".into()); }
    let rows = process::enumerate().map_err(|e| e.message)?;
    let row = rows.iter().find(|r| policy::same_process(&r.identity, game))
        .ok_or("The selected game lifetime ended or changed. Refresh and select it again; no background app was changed.")?;
    if let Some(reason) = &row.protected_reason { return Err(format!("The selected process is protected infrastructure, not an eligible game: {reason}")); }
    Ok(game.clone())
}
''')
edit('src/windows/runner.rs','time::{Duration, Instant, SystemTime, UNIX_EPOCH}', 'time::{Duration, SystemTime, UNIX_EPOCH}')
edit('src/windows/runner.rs','    let game = match attach_or_launch(&s.plan) {', '''    if db.stop_requested(id)? { s.stage = Stage::Restored; s.note("Cancelled before any mutation."); db.save(&s)?; return Ok(()); }
    let game = match attach_or_launch(&s.plan) {''')
edit('src/windows/runner.rs','''        let db = database();
        while !monitor_done.load(Ordering::Relaxed) {
            let should_stop = !matches!(process::alive(&monitor_game), Ok(true))''','''        let db = database();
        let handle = process::exact_handle(&monitor_game, 0);
        while !monitor_done.load(Ordering::Relaxed) {
            let should_stop = handle.as_ref().map_or(true, |h| unsafe { windows_sys::Win32::System::Threading::WaitForSingleObject(h.0, 0) } != windows_sys::Win32::Foundation::WAIT_TIMEOUT)''')
span('src/windows/runner.rs','pub fn state_dir()', '\npub fn database()', '''pub fn state_dir() -> AppResult<PathBuf> {
    static PATH: std::sync::OnceLock<AppResult<PathBuf>> = std::sync::OnceLock::new();
    PATH.get_or_init(super::storage::prepare_state_directory).clone()
}
''')
edit('src/windows/runner.rs','use std::os::windows::{fs::MetadataExt, process::CommandExt};','use std::os::windows::process::CommandExt;')
edit('src/windows/mod.rs','pub mod runner;','pub mod runner;\npub mod storage;')
edit('src/windows/ui.rs','(FORCE, "Force-close allowed")','(FORCE, "Force terminate")')
edit('src/windows/ui.rs','(OPT_LAUNCH, "Launch game if needed")','(OPT_LAUNCH, "Launch adapter unavailable")')
edit('src/windows/ui.rs','        settings.options.gpu_priority = false;','        settings.options.gpu_priority = false;\n        settings.options.launch_game = false;')
edit('src/windows/ui.rs','        self.update_fonts();\n        self.layout();','''        unsafe { EnableWindow(self.h(OPT_LAUNCH), 0); }
        self.update_fonts();
        self.layout();''')
edit('src/windows/ui.rs','        self.settings.options.launch_game = self.checked(OPT_LAUNCH);','        self.settings.options.launch_game = false;')
edit('src/windows/ui.rs','        let mut plan = Plan {','''        if self.game.is_none() { return Err("Select the actual running game using 'Use selected as game'. Browsing a path is not lifetime approval.".into()); }
        let mut plan = Plan {''')
edit('src/windows/ui.rs','        let forced: Vec<_> = plan','''        plan.experimental_consent = !plan.options.gpu_priority || confirm(self.window,
            "EXPERIMENTAL GPU SCHEDULING\\n\\nEnable the separately selected GPU scheduling policy for this session? It is not a GPU quota. Performance benefit on your hardware is unverified. Original values must be readable before any change.");
        if !plan.experimental_consent { return Ok(()); }
        let forced: Vec<_> = plan''')
edit('src/windows/ui.rs','After a normal close request times out, terminate these exact processes?','Directly force-terminate these exact processes? No normal save/close workflow will be completed.')
edit('src/windows/ui.rs','Apply the {} explicitly listed process actions?','I have reviewed these {} explicitly listed processes and confirm they are unnecessary for my game, voice, accessibility and device operation. Apply their listed actions?')
edit('src/windows/ui.rs','        let session = Session::new(runner::fresh_id(), plan);','        policy::validate(&plan)?;\n        let session = Session::new(runner::fresh_id(), plan);')
edit('src/windows/ui.rs','''pub fn run() -> AppResult<()> {
    run_inner(false)''','''pub fn run() -> AppResult<()> {
    if process::is_elevated().map_err(|e| e.message)? { return Err("Run this app normally, not as administrator.".into()); }
    run_inner(false)''')
edit('src/gpu.rs','value.is_finite() && value >= 0.0','value.is_finite() && (0.0..=100.0).contains(&value)')
edit('src/gpu.rs','Some(value.min(100.0))','Some(value)')
edit('src/gpu.rs','assert_eq!(valid_percent(120.0), Some(100.0));','assert_eq!(valid_percent(120.0), None);')
edit('src/gpu.rs','.filter(|v| v.is_finite())','.filter(|v| valid_percent(*v).is_some())')
edit('src/gpu.rs','''    if adapters.is_empty() {
        None
    } else {
        Some(adapters.values().fold(0u64, |a, b| a.saturating_add(*b)))
    }''','''    // A one-number UI summary is ambiguous across adapters. Detailed rows remain separate.
    if adapters.len() == 1 { adapters.values().next().copied() } else { None }''')
edit('src/gpu.rs','.unwrap_or("0");','.unwrap_or("unknown");')
edit('src/windows/gpu.rs','        if count as usize > capacity / size_of::<CounterItem>() {','        if bytes as usize > capacity || count as usize > (bytes as usize) / size_of::<CounterItem>() {')
edit('src/windows/gpu.rs','        let end = start + capacity;','        let end = start + bytes as usize;')
edit('src/windows/gpu.rs','thread::sleep(Duration::from_millis(1100));','thread::sleep(Duration::from_millis(2000));')
edit('src/windows/gpu.rs','        notices.push(format!("Collection status: {first:#x}/{second:#x}."));','''        return Ok(Snapshot { processes: process::enumerate().map_err(|e| e.message)?, gpu_status: format!("GPU collection failed ({first:#x}/{second:#x}); all values are unknown.") });''')
edit('src/windows/runner.rs','    let _lock = SessionLock::acquire()?;\n    normal_token_required()?;', '    normal_token_required()?;\n    let _lock = SessionLock::acquire()?;', count=3)
edit('src/windows/runner.rs','pub fn database() -> AppResult<Database> {\n    Database::open(&state_dir()?.join("state.sqlite3"))\n}', 'pub fn database() -> AppResult<Database> {\n    let path = state_dir()?;\n    super::storage::verify_store_entries(&path)?;\n    Database::open(&path.join("state.sqlite3"))\n}')
edit('src/windows/process.rs','use super::wide;\n','')
edit('src/windows/process.rs','    ptr::{null, null_mut},','    ptr::null_mut,')
print('Applied behavioral hardening: consent, exact identity, write-ahead cancellation, bounded close and durable ownership.')
