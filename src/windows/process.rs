use crate::{engine::Backend, model::*, policy};
use std::{
    collections::HashSet,
    mem::{size_of, zeroed},
    ptr::null_mut,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Security::Authorization::ConvertSidToStringSidW,
    Security::{
        GetTokenInformation, TokenElevation, TokenUser, TOKEN_ELEVATION, TOKEN_QUERY, TOKEN_USER,
    },
    System::{Diagnostics::ToolHelp::*, SystemInformation::GetWindowsDirectoryW, Threading::*},
    UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, WM_CLOSE},
};

// Standard access right from WinNT.h; windows-sys does not export it here.
const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;
#[link(name = "kernel32")]
unsafe extern "system" {
    fn ProcessIdToSessionId(pid: u32, session: *mut u32) -> i32;
    fn IsProcessCritical(process: HANDLE, critical: *mut i32) -> i32;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn D3DKMTGetProcessSchedulingPriorityClass(process: HANDLE, priority: *mut i32) -> i32;
    fn D3DKMTSetProcessSchedulingPriorityClass(process: HANDLE, priority: i32) -> i32;
}

pub struct Handle(pub HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

fn last_fault(context: &str) -> Fault {
    let error = std::io::Error::last_os_error();
    Fault::new(
        if error.raw_os_error() == Some(5) {
            FaultKind::Denied
        } else {
            FaultKind::Other
        },
        format!("{context}: {error}"),
    )
}

pub fn open(pid: u32, rights: u32) -> NativeResult<Handle> {
    let raw = unsafe { OpenProcess(rights, 0, pid) };
    if raw.is_null() {
        let e = std::io::Error::last_os_error();
        return Err(Fault::new(
            if e.raw_os_error() == Some(87) {
                FaultKind::Gone
            } else {
                FaultKind::Denied
            },
            format!("OpenProcess({pid}): {e}"),
        ));
    }
    Ok(Handle(raw))
}

fn filetime(t: FILETIME) -> u64 {
    (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
}

fn identity_from_handle(h: HANDLE, pid: u32) -> NativeResult<Identity> {
    let (mut created, mut exited, mut kernel, mut user): (FILETIME, FILETIME, FILETIME, FILETIME) =
        unsafe { zeroed() };
    if unsafe { GetProcessTimes(h, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
        return Err(last_fault("GetProcessTimes"));
    }
    let mut buffer = vec![0u16; 32768];
    let mut length = buffer.len() as u32;
    if unsafe { QueryFullProcessImageNameW(h, 0, buffer.as_mut_ptr(), &mut length) } == 0 {
        return Err(last_fault("QueryFullProcessImageName"));
    }
    let mut session = 0;
    if unsafe { ProcessIdToSessionId(pid, &mut session) } == 0 {
        return Err(last_fault("ProcessIdToSessionId"));
    }
    let path = String::from_utf16_lossy(&buffer[..length as usize]);
    let provenance = Some(Provenance {
        owner_sid: token_sid(h)?,
        logon_id: logon_id(h)?,
        image_file_id: image_file_id(&path)?,
    });
    Ok(Identity {
        pid,
        created: filetime(created),
        path,
        session_id: session,
        provenance,
    })
}

pub fn identity(pid: u32) -> NativeResult<Identity> {
    let h = open(pid, PROCESS_QUERY_LIMITED_INFORMATION)?;
    identity_from_handle(h.0, pid)
}

pub fn current_identity() -> NativeResult<Identity> {
    identity_from_handle(unsafe { GetCurrentProcess() }, unsafe {
        GetCurrentProcessId()
    })
}

fn token_sid(process: HANDLE) -> NativeResult<String> {
    let mut token = null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(last_fault("OpenProcessToken"));
    }
    let token = Handle(token);
    let mut length = 0;
    unsafe {
        GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut length);
    }
    if length == 0 || length > 65536 {
        return Err(last_fault("TokenUser length"));
    }
    let mut storage = vec![0u64; (length as usize).div_ceil(8)];
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            storage.as_mut_ptr().cast(),
            length,
            &mut length,
        )
    } == 0
    {
        return Err(last_fault("TokenUser"));
    }
    let info = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
    let mut text = null_mut();
    if unsafe { ConvertSidToStringSidW(info.User.Sid, &mut text) } == 0 {
        return Err(last_fault("SID conversion"));
    }
    let result = unsafe {
        let mut n = 0;
        while n < 1024 && *text.add(n) != 0 {
            n += 1;
        }
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(text, n));
        LocalFree(text.cast());
        s
    };
    Ok(result)
}

fn logon_id(process: HANDLE) -> NativeResult<u64> {
    use windows_sys::Win32::Security::{TokenStatistics, TOKEN_STATISTICS};
    let mut raw = null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut raw) } == 0 {
        return Err(last_fault("OpenProcessToken"));
    }
    let token = Handle(raw);
    let mut info: TOKEN_STATISTICS = unsafe { zeroed() };
    let mut bytes = 0;
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenStatistics,
            (&mut info as *mut TOKEN_STATISTICS).cast(),
            size_of::<TOKEN_STATISTICS>() as u32,
            &mut bytes,
        )
    } == 0
    {
        return Err(last_fault("TokenStatistics"));
    }
    Ok((u64::from(info.AuthenticationId.HighPart as u32) << 32)
        | u64::from(info.AuthenticationId.LowPart))
}

pub fn image_file_id(path: &str) -> NativeResult<String> {
    use std::os::windows::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .access_mode(0x80)
        .share_mode(7)
        .open(path)
        .map_err(|e| Fault::new(FaultKind::Denied, format!("Image identity: {e}")))?;
    image_file_id_from_file(&file)
}

pub fn image_file_id_from_file(file: &std::fs::File) -> NativeResult<String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileIdInfo, GetFileInformationByHandleEx, FILE_ID_INFO,
    };
    let mut info: FILE_ID_INFO = unsafe { zeroed() };
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            FileIdInfo,
            (&mut info as *mut FILE_ID_INFO).cast(),
            size_of::<FILE_ID_INFO>() as u32,
        )
    } == 0
    {
        return Err(last_fault("Image file identifier"));
    }
    Ok(format!(
        "{:016x}:{:02x?}",
        info.VolumeSerialNumber, info.FileId.Identifier
    ))
}

pub fn current_sid() -> NativeResult<String> {
    token_sid(unsafe { GetCurrentProcess() })
}

pub fn is_elevated() -> NativeResult<bool> {
    let mut token = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(last_fault("OpenProcessToken"));
    }
    let token = Handle(token);
    let mut elevation: TOKEN_ELEVATION = unsafe { zeroed() };
    let mut length = 0;
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut length,
        )
    } == 0
    {
        return Err(last_fault("TokenElevation"));
    }
    Ok(elevation.TokenIsElevated != 0)
}

pub fn exact_handle(id: &Identity, rights: u32) -> NativeResult<Handle> {
    let h = open(
        id.pid,
        rights | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
    )?;
    if unsafe { WaitForSingleObject(h.0, 0) } == WAIT_OBJECT_0 {
        return Err(Fault::new(FaultKind::Gone, "Original process exited."));
    }
    let current = identity_from_handle(h.0, id.pid)?;
    if !policy::same_process(id, &current) {
        return Err(Fault::new(
            FaultKind::Gone,
            "PID was reused or executable identity changed.",
        ));
    }
    Ok(h)
}

pub fn alive(id: &Identity) -> NativeResult<bool> {
    match exact_handle(id, 0) {
        Ok(_) => Ok(true),
        Err(e) if e.kind == FaultKind::Gone => Ok(false),
        Err(e) => Err(e),
    }
}

struct Safety {
    sid: String,
    session: u32,
    self_path: String,
    windows_path: String,
}

impl Safety {
    fn new() -> NativeResult<Self> {
        let me = current_identity()?;
        let mut path = vec![0u16; 32768];
        let n = unsafe { GetWindowsDirectoryW(path.as_mut_ptr(), path.len() as u32) };
        if n == 0 || n as usize >= path.len() {
            return Err(last_fault("GetWindowsDirectory"));
        }
        Ok(Self {
            sid: current_sid()?,
            session: me.session_id,
            self_path: policy::normalized_path(&me.path),
            windows_path: policy::normalized_path(&String::from_utf16_lossy(&path[..n as usize])),
        })
    }
    fn check(&self, h: HANDLE, id: &Identity) -> NativeResult<()> {
        if id.pid <= 4 || id.created == 0 {
            return Err(Fault::new(
                FaultKind::Denied,
                "System or unidentified process.",
            ));
        }
        if id.session_id != self.session {
            return Err(Fault::new(FaultKind::Denied, "Different Windows session."));
        }
        if token_sid(h)? != self.sid {
            return Err(Fault::new(
                FaultKind::Denied,
                "Different user or system account.",
            ));
        }
        let path = policy::normalized_path(&id.path);
        let name = path.rsplit('\\').next().unwrap_or("");
        if path == self.self_path || policy::reserved_name(name) {
            return Err(Fault::new(
                FaultKind::Denied,
                "Protected system, game-support, voice or optimizer application.",
            ));
        }
        if path.starts_with(&(self.windows_path.clone() + "\\")) {
            return Err(Fault::new(
                FaultKind::Denied,
                "Windows component: protected.",
            ));
        }
        let mut protection = PROCESS_PROTECTION_LEVEL_INFORMATION { ProtectionLevel: 0 };
        if unsafe {
            GetProcessInformation(
                h,
                ProcessProtectionLevelInfo,
                (&mut protection as *mut PROCESS_PROTECTION_LEVEL_INFORMATION).cast(),
                size_of::<PROCESS_PROTECTION_LEVEL_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(last_fault("Cannot establish process protection level"));
        }
        if protection.ProtectionLevel != PROTECTION_LEVEL_NONE {
            return Err(Fault::new(FaultKind::Denied, "Protected process."));
        }
        let mut critical = 0;
        if unsafe { IsProcessCritical(h, &mut critical) } == 0 {
            return Err(last_fault("Cannot establish critical-process status"));
        }
        if critical != 0 {
            return Err(Fault::new(FaultKind::Denied, "Critical Windows process."));
        }
        Ok(())
    }
}

pub fn enumerate() -> NativeResult<Vec<ProcessRow>> {
    let safety = Safety::new()?;
    let snapshot = Handle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) });
    if snapshot.0 == INVALID_HANDLE_VALUE {
        return Err(last_fault("Process snapshot"));
    }
    let mut entry: PROCESSENTRY32W = unsafe { zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut rows = Vec::new();
    let mut ok = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    while ok != 0 {
        let n = entry
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(entry.szExeFile.len());
        let name = String::from_utf16_lossy(&entry.szExeFile[..n]);
        let inspected =
            open(entry.th32ProcessID, PROCESS_QUERY_LIMITED_INFORMATION).and_then(|h| {
                let id = identity_from_handle(h.0, entry.th32ProcessID)?;
                let reason = safety.check(h.0, &id).err().map(|e| e.message);
                Ok((id, reason))
            });
        let (id, reason) = inspected.unwrap_or_else(|e| {
            (
                Identity {
                    pid: entry.th32ProcessID,
                    created: 0,
                    path: String::new(),
                    session_id: u32::MAX,
                    provenance: None,
                },
                Some(format!("Unavailable/protected: {}", e.message)),
            )
        });
        rows.push(ProcessRow {
            identity: id,
            parent_pid: entry.th32ParentProcessID,
            name,
            protected_reason: reason,
            gpu: vec![],
        });
        ok = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    Ok(rows)
}

pub struct WindowsBackend {
    safety: Safety,
    protected_paths: Vec<String>,
    guarded: HashSet<u32>,
    game: Option<Identity>,
    cancel: Option<Arc<AtomicBool>>,
}

impl WindowsBackend {
    pub fn new(
        game: Option<Identity>,
        protected_paths: Vec<String>,
        cancel: Option<Arc<AtomicBool>>,
    ) -> NativeResult<Self> {
        let rows = enumerate()?;
        let guarded = game
            .as_ref()
            .map(|g| policy::guarded_pids(&rows, g))
            .unwrap_or_default();
        Ok(Self {
            safety: Safety::new()?,
            protected_paths,
            guarded,
            game,
            cancel,
        })
    }
    fn checked(&self, id: &Identity, rights: u32) -> NativeResult<Handle> {
        let h = exact_handle(id, rights)?;
        self.safety.check(h.0, id)?;
        if self.guarded.contains(&id.pid)
            || self
                .protected_paths
                .iter()
                .any(|p| policy::normalized_path(p) == policy::normalized_path(&id.path))
        {
            return Err(Fault::new(
                FaultKind::Denied,
                "Game family or user-protected executable.",
            ));
        }
        Ok(h)
    }
    fn cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|v| v.load(Ordering::Relaxed))
            || self
                .game
                .as_ref()
                .is_some_and(|g| !matches!(alive(g), Ok(true)))
    }
}

impl Backend for WindowsBackend {
    fn validate_reopen(&mut self, id: &Identity, approval: &ReopenApproval) -> NativeResult<()> {
        self.authorize(id)?;
        super::reopen::validate_approval(id, approval)
    }
    fn reopen(&mut self, id: &Identity, approval: &ReopenApproval) -> NativeResult<ReopenRecord> {
        super::reopen::reopen(id, approval)
    }
    fn authorize(&mut self, id: &Identity) -> NativeResult<()> {
        if self.cancelled() {
            return Err(Fault::new(
                FaultKind::Denied,
                "Session cancelled or game no longer running.",
            ));
        }
        if self.game.is_some() {
            let settings = super::runner::database()
                .and_then(|db| db.settings())
                .map_err(|e| {
                    Fault::new(
                        FaultKind::Denied,
                        format!("Cannot revalidate user protection: {e}"),
                    )
                })?;
            for path in settings.protected_paths {
                if !self.protected_paths.contains(&path) {
                    self.protected_paths.push(path);
                }
            }
        }
        self.checked(id, 0).map(|_| ())
    }
    fn read(&mut self, id: &Identity, property: Property) -> NativeResult<Value> {
        let rights = if property == Property::GpuPriority {
            PROCESS_QUERY_INFORMATION
        } else {
            0
        };
        let h = self.checked(id, rights)?;
        match property {
            Property::GpuPriority => {
                let mut value = 0;
                let status = unsafe { D3DKMTGetProcessSchedulingPriorityClass(h.0, &mut value) };
                if status != 0 || !(0..=5).contains(&value) {
                    return Err(Fault::new(
                        FaultKind::Unsupported,
                        format!("GPU scheduling query unavailable (NTSTATUS {status:#x})."),
                    ));
                }
                Ok(Value::GpuPriority(value))
            }
            Property::CpuPriority => {
                let value = unsafe { GetPriorityClass(h.0) };
                if value == 0 {
                    return Err(last_fault("GetPriorityClass"));
                }
                Ok(Value::CpuPriority(value))
            }
            Property::EcoQos => {
                let mut value = PROCESS_POWER_THROTTLING_STATE {
                    Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
                    ControlMask: 0,
                    StateMask: 0,
                };
                if unsafe {
                    GetProcessInformation(
                        h.0,
                        ProcessPowerThrottling,
                        (&mut value as *mut PROCESS_POWER_THROTTLING_STATE).cast(),
                        size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
                    )
                } == 0
                {
                    return Err(last_fault(
                        "EcoQoS query unsupported or denied; left untouched",
                    ));
                }
                if value.Version != PROCESS_POWER_THROTTLING_CURRENT_VERSION {
                    return Err(Fault::new(
                        FaultKind::Unsupported,
                        "Unknown EcoQoS structure version; left untouched.",
                    ));
                }
                Ok(Value::EcoQos {
                    control: value.ControlMask,
                    state: value.StateMask,
                })
            }
            Property::MemoryPriority => {
                let mut value = MEMORY_PRIORITY_INFORMATION { MemoryPriority: 0 };
                if unsafe {
                    GetProcessInformation(
                        h.0,
                        ProcessMemoryPriority,
                        (&mut value as *mut MEMORY_PRIORITY_INFORMATION).cast(),
                        size_of::<MEMORY_PRIORITY_INFORMATION>() as u32,
                    )
                } == 0
                {
                    return Err(last_fault("Memory priority query"));
                }
                Ok(Value::MemoryPriority(value.MemoryPriority))
            }
        }
    }
    fn write(&mut self, id: &Identity, value: &Value) -> NativeResult<()> {
        if !value.valid() {
            return Err(Fault::new(
                FaultKind::Other,
                "Invalid native policy value; no setter was called.",
            ));
        }
        if self.cancelled() {
            return Err(Fault::new(
                FaultKind::Denied,
                "Cancelled before native setter.",
            ));
        }
        let h = self.checked(id, PROCESS_SET_INFORMATION)?;
        let ok = match *value {
            Value::GpuPriority(priority) => {
                if !(0..=5).contains(&priority) {
                    return Err(Fault::new(
                        FaultKind::Other,
                        "Invalid GPU priority in journal.",
                    ));
                }
                let status = unsafe { D3DKMTSetProcessSchedulingPriorityClass(h.0, priority) };
                if status != 0 {
                    return Err(Fault::new(
                        FaultKind::Unsupported,
                        format!("GPU priority setter unavailable (NTSTATUS {status:#x})."),
                    ));
                }
                1
            }
            Value::CpuPriority(priority) => unsafe { SetPriorityClass(h.0, priority) },
            Value::EcoQos { control, state } => {
                let v = PROCESS_POWER_THROTTLING_STATE {
                    Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
                    ControlMask: control,
                    StateMask: state,
                };
                unsafe {
                    SetProcessInformation(
                        h.0,
                        ProcessPowerThrottling,
                        (&v as *const PROCESS_POWER_THROTTLING_STATE).cast(),
                        size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
                    )
                }
            }
            Value::MemoryPriority(priority) => {
                let v = MEMORY_PRIORITY_INFORMATION {
                    MemoryPriority: priority,
                };
                unsafe {
                    SetProcessInformation(
                        h.0,
                        ProcessMemoryPriority,
                        (&v as *const MEMORY_PRIORITY_INFORMATION).cast(),
                        size_of::<MEMORY_PRIORITY_INFORMATION>() as u32,
                    )
                }
            }
        };
        if ok == 0 {
            return Err(last_fault("Priority write"));
        }
        Ok(())
    }
    fn close(&mut self, id: &Identity, force: bool, timeout_ms: u32) -> NativeResult<CloseState> {
        self.authorize(id)?;
        if force {
            // Direct force is a distinct reviewed action, never a WM_CLOSE fallback.
            let h = self.checked(id, PROCESS_TERMINATE)?;
            if self.cancelled() {
                return Ok(CloseState::KeptOpen);
            }
            if unsafe { TerminateProcess(h.0, 1) } == 0 {
                return Err(last_fault("TerminateProcess"));
            }
            return if unsafe { WaitForSingleObject(h.0, 3000) } == WAIT_OBJECT_0 {
                Ok(CloseState::Terminated)
            } else {
                Err(Fault::new(
                    FaultKind::Other,
                    "Termination requested; exit remains unconfirmed.",
                ))
            };
        }
        let h = self.checked(id, 0)?;
        struct Context {
            pid: u32,
            windows: Vec<HWND>,
        }
        unsafe extern "system" fn callback(window: HWND, data: LPARAM) -> i32 {
            let ctx = &mut *(data as *mut Context);
            let mut pid = 0;
            GetWindowThreadProcessId(window, &mut pid);
            if pid == ctx.pid && ctx.windows.len() < 16 {
                ctx.windows.push(window);
            }
            i32::from(ctx.windows.len() < 16)
        }
        let mut context = Context {
            pid: id.pid,
            windows: Vec::new(),
        };
        unsafe {
            EnumWindows(Some(callback), (&mut context as *mut Context) as LPARAM);
        }
        if context.windows.is_empty() {
            return Ok(CloseState::KeptOpen);
        }
        let mut requested = false;
        for window in context.windows {
            if self.cancelled() {
                break;
            }
            let mut owner = 0;
            unsafe {
                GetWindowThreadProcessId(window, &mut owner);
            }
            if owner != id.pid || unsafe { WaitForSingleObject(h.0, 0) } == WAIT_OBJECT_0 {
                continue;
            }
            // Timeout may mean delivery already occurred: never mark it safely undone.
            requested = true;
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::SendMessageTimeoutW(
                    window,
                    WM_CLOSE,
                    0,
                    0,
                    0x0002 | 0x0020,
                    1000,
                    null_mut(),
                );
            }
        }
        let deadline =
            Instant::now() + Duration::from_millis(u64::from(timeout_ms.clamp(1000, 15000)));
        while Instant::now() < deadline {
            if unsafe { WaitForSingleObject(h.0, 100) } == WAIT_OBJECT_0 {
                return Ok(CloseState::ClosedGracefully);
            }
            if self.cancelled() {
                break;
            }
        }
        Ok(if requested {
            CloseState::RequestedPending
        } else {
            CloseState::KeptOpen
        })
    }
}

/// A file-sharing lock coordinates every session using this per-user store,
/// including different Windows interactive sessions. The filename is not ownership.
pub struct SessionLock {
    _file: std::fs::File,
}
impl SessionLock {
    pub fn acquire() -> AppResult<Self> {
        Self::acquire_at(&super::runner::state_dir()?.join("controller.lock"))
    }
    pub fn acquire_at(path: &std::path::Path) -> AppResult<Self> {
        use std::os::windows::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(path)
            .map_err(|e| {
                format!("A controller owns this store or the lock cannot be opened: {e}")
            })?;
        Ok(Self { _file: file })
    }
}
