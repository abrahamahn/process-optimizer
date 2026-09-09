//! Narrow GUI launch adapter. No shell, arguments, elevation, inherited approval or retry.
use super::process;
use crate::{applications, model::*, policy};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    mem::size_of,
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::Path,
    ptr::null_mut,
};
use windows_sys::Win32::{
    Foundation::*,
    System::{RemoteDesktop::*, StationsAndDesktops::*},
    UI::WindowsAndMessaging::*,
};

fn denied(message: impl Into<String>) -> Fault {
    Fault::new(FaultKind::Denied, message)
}

/// Open while denying writes and deletion, keeping that handle through CreateProcess.
fn image(id: &Identity) -> NativeResult<(File, ReopenApproval)> {
    if !policy::complete_identity(id) {
        return Err(denied("Incomplete executable identity."));
    }
    let path = Path::new(&id.path);
    let leaf = policy::normalized_path(&id.path)
        .rsplit('\\')
        .next()
        .unwrap_or("")
        .to_owned();
    if policy::reserved_name(&leaf)
        || matches!(
            leaf.as_str(),
            "cmd.exe"
                | "powershell.exe"
                | "pwsh.exe"
                | "wsl.exe"
                | "bash.exe"
                | "python.exe"
                | "pythonw.exe"
                | "node.exe"
                | "dotnet.exe"
                | "rundll32.exe"
                | "regsvr32.exe"
                | "msiexec.exe"
                | "conhost.exe"
                | "wt.exe"
        )
    {
        return Err(denied("System infrastructure, shells, interpreters and job hosts cannot be generically reopened."));
    }
    for ancestor in path.ancestors() {
        if ancestor.parent().is_none() {
            continue;
        }
        let meta = std::fs::symlink_metadata(ancestor).map_err(|e| denied(e.to_string()))?;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(denied(
                "Reparse-path executable reopening is not supported.",
            ));
        }
    }
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(0x00200000)
        .open(path)
        .map_err(|e| denied(e.to_string()))?;
    let meta = file.metadata().map_err(|e| denied(e.to_string()))?;
    if !meta.is_file() || meta.file_attributes() & 0x400 != 0 {
        return Err(denied("Not an ordinary executable file."));
    }
    let approval = ReopenApproval {
        image_file_id: process::image_file_id_from_file(&file)?,
        file_size: meta.file_size(),
        modified: meta.last_write_time(),
    };
    if id.provenance.as_ref().map(|p| &p.image_file_id) != Some(&approval.image_file_id) {
        return Err(denied("Executable was replaced."));
    }
    // Bound all reads. PE optional-header Subsystem is at offset 68 for PE32/PE32+.
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos)
        .map_err(|e| denied(e.to_string()))?;
    if &dos[..2] != b"MZ" {
        return Err(denied("Not a PE executable."));
    }
    let offset = u32::from_le_bytes(dos[60..64].try_into().unwrap()) as u64;
    if !(64..=1024 * 1024).contains(&offset) || offset + 94 > meta.file_size() {
        return Err(denied("Invalid executable header."));
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| denied(e.to_string()))?;
    let mut pe = [0u8; 94];
    file.read_exact(&mut pe)
        .map_err(|e| denied(e.to_string()))?;
    let magic = u16::from_le_bytes([pe[24], pe[25]]);
    let optional_size = u16::from_le_bytes([pe[20], pe[21]]);
    let subsystem = u16::from_le_bytes([pe[92], pe[93]]);
    if &pe[..4] != b"PE\0\0"
        || !matches!(magic, 0x10b | 0x20b)
        || optional_size < 70
        || subsystem != 2
    {
        return Err(denied("Only ordinary Windows GUI executables can be reopened; console workloads are excluded."));
    }
    Ok((file, approval))
}

fn has_window(pid: u32) -> bool {
    struct Search {
        pid: u32,
        found: bool,
    }
    unsafe extern "system" fn callback(window: HWND, data: LPARAM) -> i32 {
        let search = &mut *(data as *mut Search);
        let mut owner = 0;
        GetWindowThreadProcessId(window, &mut owner);
        if owner == search.pid {
            search.found = true;
            return 0;
        }
        1
    }
    let mut search = Search { pid, found: false };
    unsafe {
        EnumWindows(Some(callback), (&mut search as *mut Search) as LPARAM);
    }
    search.found
}

pub fn approval(id: &Identity) -> NativeResult<ReopenApproval> {
    let _process = process::exact_handle(id, 0)?;
    if !has_window(id.pid) {
        return Err(denied(
            "Select this app's main GUI process, not a GPU helper or headless job.",
        ));
    }
    let (_, stamp) = image(id)?;
    Ok(stamp)
}
pub fn validate_approval(id: &Identity, expected: &ReopenApproval) -> NativeResult<()> {
    if approval(id)? != *expected {
        return Err(denied(
            "The executable changed after reopening was approved.",
        ));
    }
    Ok(())
}

/// WTS lock/connection evidence plus input desktop; unknown or disconnected is not unlocked.
pub fn desktop_ready(session_id: u32) -> bool {
    unsafe {
        let mut buffer = null_mut();
        let mut bytes = 0;
        if WTSQuerySessionInformationW(
            WTS_CURRENT_SERVER_HANDLE,
            session_id,
            WTSSessionInfoEx,
            &mut buffer,
            &mut bytes,
        ) == 0
        {
            return false;
        }
        let ready = if !buffer.is_null() && bytes as usize >= size_of::<WTSINFOEXW>() {
            let info = &*buffer.cast::<WTSINFOEXW>();
            if info.Level != 1 {
                false
            } else {
                let level = info.Data.WTSInfoExLevel1;
                level.SessionId == session_id
                    && level.SessionState == WTSActive
                    && level.SessionFlags == WTS_SESSIONSTATE_UNLOCK as i32
            }
        } else {
            false
        };
        WTSFreeMemory(buffer.cast());
        if !ready {
            return false;
        }
        let desktop = OpenInputDesktop(0, 0, DESKTOP_READOBJECTS);
        if desktop.is_null() {
            return false;
        }
        let mut name = [0u16; 128];
        let mut needed = 0;
        let ok = GetUserObjectInformationW(
            desktop,
            UOI_NAME,
            name.as_mut_ptr().cast(),
            size_of::<[u16; 128]>() as u32,
            &mut needed,
        ) != 0;
        CloseDesktop(desktop);
        if !ok {
            return false;
        }
        let n = name.iter().position(|v| *v == 0).unwrap_or(name.len());
        String::from_utf16_lossy(&name[..n]).eq_ignore_ascii_case("default")
    }
}

pub fn reopen(id: &Identity, expected: &ReopenApproval) -> NativeResult<ReopenRecord> {
    let outcome = |state, detail: String| ReopenRecord {
        target: id.clone(),
        state,
        child: None,
        detail,
    };
    let attempt = || -> NativeResult<ReopenRecord> {
        if process::is_elevated()? {
            return Err(denied("Reopening is never elevated."));
        }
        let caller = process::current_identity()?;
        if !applications::same_user_session(&caller, id) {
            return Err(denied("Original user/logon/session is no longer current."));
        }
        let (file, observed) = image(id)?;
        if observed != *expected {
            return Err(denied(
                "Executable size, write time or file identity changed; open manually.",
            ));
        }
        let settings = super::runner::database()
            .and_then(|db| db.settings())
            .map_err(denied)?;
        if settings
            .protected_paths
            .iter()
            .any(|p| policy::normalized_path(p) == policy::normalized_path(&id.path))
        {
            return Ok(outcome(
                ReopenState::Skipped,
                "Newly protected app; no launch was attempted.".into(),
            ));
        }
        let rows = process::enumerate()?;
        let path = policy::normalized_path(&id.path);
        if rows.iter().any(|r| {
            policy::normalized_path(&r.identity.path) == path
                && applications::same_user_session(&r.identity, id)
        }) {
            return Ok(outcome(
                ReopenState::AlreadyRunning,
                "An app instance or helper is already running. No duplicate was launched.".into(),
            ));
        }
        if rows.iter().any(|r| {
            r.name
                .eq_ignore_ascii_case(path.rsplit('\\').next().unwrap_or(""))
                && !policy::complete_identity(&r.identity)
        }) {
            return Ok(outcome(
                ReopenState::Deferred,
                "A possible existing instance cannot be identified. Reopen manually after review."
                    .into(),
            ));
        }
        if !desktop_ready(id.session_id) {
            return Ok(outcome(ReopenState::Deferred, "Desktop is locked, disconnected or unverifiable. No launch; reopen manually after returning.".into()));
        }
        let child = std::process::Command::new(&id.path)
            .current_dir(
                Path::new(&id.path)
                    .parent()
                    .ok_or_else(|| denied("Missing application directory"))?,
            )
            .creation_flags(0x20) // NORMAL_PRIORITY_CLASS, never replay old per-process settings.
            .spawn()
            .map_err(|e| denied(format!("GUI launch failed: {e}")))?;
        let child_id = process::identity(child.id());
        drop(file); // File identity was held from validation through native creation.
        match child_id {
            Ok(child) if applications::app_key(&child).ok() == applications::app_key(id).ok() && applications::same_user_session(&child, id) => {
                Ok(ReopenRecord { target: id.clone(), state: ReopenState::Started, child: Some(child), detail: "A new GUI process was started without arguments. This does not prove its UI is ready or recover unsaved work.".into() })
            }
            _ => Ok(outcome(ReopenState::Indeterminate, "Launch returned but the new lifetime could not be verified (it may have exited or handed off). No retry will occur.".into())),
        }
    };
    // Errors before a successful spawn are known no-launch outcomes. Post-spawn uncertainty
    // is returned explicitly above, so recovery never turns it into a retry.
    match attempt() {
        Ok(r) => Ok(r),
        Err(e) => Ok(outcome(ReopenState::Skipped, e.message)),
    }
}
