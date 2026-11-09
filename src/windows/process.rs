use crate::{model::*, policy};
use std::{
    mem::{size_of, zeroed},
    ptr::null_mut,
};
use windows_sys::Win32::{
    Foundation::*,
    Security::Authorization::ConvertSidToStringSidW,
    Security::{
        GetTokenInformation, TokenElevation, TokenUser, TOKEN_ELEVATION, TOKEN_QUERY, TOKEN_USER,
    },
    System::{Diagnostics::ToolHelp::*, SystemInformation::GetWindowsDirectoryW, Threading::*},
};

// Standard access right from WinNT.h; windows-sys does not export it here.
const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;
#[link(name = "kernel32")]
unsafe extern "system" {
    fn ProcessIdToSessionId(pid: u32, session: *mut u32) -> i32;
    fn IsProcessCritical(process: HANDLE, critical: *mut i32) -> i32;
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

pub(super) fn last_fault(context: &str) -> Fault {
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

pub(super) struct Safety {
    sid: String,
    session: u32,
    self_path: String,
    windows_path: String,
}

impl Safety {
    pub(super) fn new() -> NativeResult<Self> {
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
    pub(super) fn check(&self, h: HANDLE, id: &Identity) -> NativeResult<()> {
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
