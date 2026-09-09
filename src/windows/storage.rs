//! Owner-restricted local storage. No elevated, remote or reparse-path fallback.
use super::{process, wide};
use crate::model::AppResult;
use std::{
    os::windows::fs::MetadataExt,
    path::{Component, Path, PathBuf, Prefix},
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW, SE_FILE_OBJECT,
    },
    Security::{
        GetSecurityDescriptorDacl, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
    },
};

pub fn prepare_state_directory() -> AppResult<PathBuf> {
    if process::is_elevated().map_err(|e| e.message)? {
        return Err("Run the application normally, not as administrator.".into());
    }
    let base =
        PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable.")?);
    if !base.is_absolute()
        || !matches!(base.components().next(), Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)))
    {
        return Err("The state directory must be on a local absolute Windows drive path.".into());
    }
    for ancestor in base.ancestors() {
        if ancestor.parent().is_none() {
            continue;
        }
        let metadata = std::fs::symlink_metadata(ancestor).map_err(|e| e.to_string())?;
        if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
            return Err(
                "Reparse or non-directory ancestor in the state path; no fallback is allowed."
                    .into(),
            );
        }
    }
    let path = base.join("ProcessOptimizer");
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    secure_directory(&path)?;
    verify_store_entries(&path)?;
    Ok(path)
}

/// Public only for integration tests on a directory the test itself creates.
pub fn secure_directory(path: &Path) -> AppResult<()> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.file_attributes() & 0x400 != 0 {
        return Err("Recovery storage must be a normal non-reparse directory.".into());
    }
    let sid = process::current_sid().map_err(|e| e.message)?;
    let sddl = wide(&format!("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;{sid})"));
    let mut descriptor = null_mut();
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            null_mut(),
        )
    } == 0
    {
        return Err(format!(
            "Cannot prepare recovery ACL: {}",
            std::io::Error::last_os_error()
        ));
    }
    let result = (|| {
        let (mut present, mut inherited) = (0, 0);
        let mut acl = null_mut();
        if unsafe { GetSecurityDescriptorDacl(descriptor, &mut present, &mut acl, &mut inherited) }
            == 0
            || present == 0
            || acl.is_null()
        {
            return Err("Could not obtain the restrictive recovery ACL.".into());
        }
        let mut name = wide(&path.to_string_lossy());
        let status = unsafe {
            SetNamedSecurityInfoW(
                name.as_mut_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                acl,
                null(),
            )
        };
        if status != 0 {
            return Err(format!(
                "Cannot restrict recovery directory permissions: {status}"
            ));
        }
        Ok(())
    })();
    unsafe {
        LocalFree(descriptor);
    }
    result
}

pub fn verify_store_entries(path: &Path) -> AppResult<()> {
    let directory = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !directory.is_dir() || directory.file_attributes() & 0x400 != 0 {
        return Err("Recovery directory changed to a reparse or non-directory entry.".into());
    }
    for name in [
        "state.sqlite3",
        "state.sqlite3-wal",
        "state.sqlite3-shm",
        "controller.lock",
        "last-session.json",
    ] {
        match std::fs::symlink_metadata(path.join(name)) {
            Ok(meta) if !meta.is_file() || meta.file_attributes() & 0x400 != 0 => {
                return Err(format!("Recovery entry {name} is not a normal file."))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}
