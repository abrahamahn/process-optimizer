//! Reversible current-user startup cleanup.
//! This module intentionally supports only HKCU\...\Run values whose executable
//! token can be matched exactly to a reviewed local executable. It does not
//! disable services, drivers, scheduled tasks, shell extensions, or machine-wide
//! startup entries.
use super::wide;
use crate::{manual::StartupValueKind, model::*, policy};
use std::ptr::null_mut;
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS},
    System::Registry::{
        RegCloseKey, RegDeleteValueW, RegEnumValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_EXPAND_SZ,
        REG_SZ,
    },
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const MAX_VALUES: u32 = 256;
const MAX_NAME_CHARS: usize = 16 * 1024;
const MAX_DATA_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunEntry {
    pub value_name: String,
    pub command: String,
    pub kind: StartupValueKind,
    pub executable: Option<String>,
}

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

fn fault(operation: &str, code: u32) -> Fault {
    Fault::new(
        FaultKind::Denied,
        format!("{operation} failed with Windows error {code}."),
    )
}

fn open(access: u32) -> NativeResult<Option<Key>> {
    let mut key = null_mut();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            wide(RUN_KEY).as_ptr(),
            0,
            access,
            &mut key,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if status != ERROR_SUCCESS {
        return Err(fault("Open current-user startup key", status));
    }
    Ok(Some(Key(key)))
}

fn decode_string(data: &[u8]) -> NativeResult<String> {
    if data.len() % 2 != 0 {
        return Err(Fault::new(
            FaultKind::Other,
            "Malformed UTF-16 startup value.",
        ));
    }
    let mut words = Vec::with_capacity(data.len() / 2);
    for pair in data.chunks_exact(2) {
        words.push(u16::from_le_bytes([pair[0], pair[1]]));
    }
    if let Some(end) = words.iter().position(|v| *v == 0) {
        words.truncate(end);
    }
    String::from_utf16(&words)
        .map_err(|_| Fault::new(FaultKind::Other, "Invalid UTF-16 startup value."))
}

/// Conservative executable-token parsing. Environment-variable and shell
/// commands are deliberately not matched automatically.
pub fn command_executable(command: &str) -> Option<String> {
    let text = command.trim();
    if text.is_empty() || text.starts_with('%') {
        return None;
    }
    let token = if let Some(rest) = text.strip_prefix('"') {
        let end = rest.find('"')?;
        &rest[..end]
    } else {
        text.split_whitespace().next()?
    };
    if token.is_empty() || token.contains('%') {
        return None;
    }
    Some(policy::normalized_path(token))
}

fn query(key: HKEY, value_name: &str) -> NativeResult<Option<(String, StartupValueKind)>> {
    let name = wide(value_name);
    let mut kind = 0u32;
    let mut bytes = 0u32;
    let first = unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            null_mut(),
            &mut kind,
            null_mut(),
            &mut bytes,
        )
    };
    if first == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if first != ERROR_SUCCESS {
        return Err(fault("Read startup value", first));
    }
    if bytes as usize > MAX_DATA_BYTES {
        return Err(Fault::new(
            FaultKind::Other,
            "Startup value is too large to manage safely.",
        ));
    }
    let value_kind = match kind {
        REG_SZ => StartupValueKind::String,
        REG_EXPAND_SZ => StartupValueKind::ExpandString,
        _ => {
            return Err(Fault::new(
                FaultKind::Unsupported,
                "Startup entry is not a string command.",
            ))
        }
    };
    let mut data = vec![0u8; bytes as usize];
    let mut actual = bytes;
    let second = unsafe {
        RegQueryValueExW(
            key,
            name.as_ptr(),
            null_mut(),
            &mut kind,
            data.as_mut_ptr(),
            &mut actual,
        )
    };
    if second != ERROR_SUCCESS {
        return Err(fault("Read startup value data", second));
    }
    data.truncate(actual as usize);
    Ok(Some((decode_string(&data)?, value_kind)))
}

pub fn entries() -> NativeResult<Vec<RunEntry>> {
    let Some(key) = open(KEY_QUERY_VALUE)? else {
        return Ok(vec![]);
    };
    let mut out = Vec::new();
    for index in 0..MAX_VALUES {
        let mut name = vec![0u16; MAX_NAME_CHARS];
        let mut name_len = (name.len() - 1) as u32;
        let mut kind = 0u32;
        let mut data = vec![0u8; MAX_DATA_BYTES];
        let mut data_len = data.len() as u32;
        let status = unsafe {
            RegEnumValueW(
                key.0,
                index,
                name.as_mut_ptr(),
                &mut name_len,
                null_mut(),
                &mut kind,
                data.as_mut_ptr(),
                &mut data_len,
            )
        };
        if status == ERROR_NO_MORE_ITEMS {
            return Ok(out);
        }
        if status == ERROR_MORE_DATA {
            return Err(Fault::new(
                FaultKind::Other,
                "A startup value exceeds the safe enumeration limit.",
            ));
        }
        if status != ERROR_SUCCESS {
            return Err(fault("Enumerate startup values", status));
        }
        if !matches!(kind, REG_SZ | REG_EXPAND_SZ) {
            continue;
        }
        name.truncate(name_len as usize);
        data.truncate(data_len as usize);
        let value_name = String::from_utf16(&name)
            .map_err(|_| Fault::new(FaultKind::Other, "Invalid startup value name."))?;
        let command = decode_string(&data)?;
        let kind = if kind == REG_EXPAND_SZ {
            StartupValueKind::ExpandString
        } else {
            StartupValueKind::String
        };
        out.push(RunEntry {
            value_name,
            executable: command_executable(&command),
            command,
            kind,
        });
    }
    Err(Fault::new(
        FaultKind::Other,
        "Too many current-user startup values to manage safely.",
    ))
}

pub fn matching(path: &str) -> NativeResult<Vec<RunEntry>> {
    let path = policy::normalized_path(path);
    Ok(entries()?
        .into_iter()
        .filter(|entry| entry.executable.as_deref() == Some(path.as_str()))
        .collect())
}

pub fn disable(entry: &RunEntry) -> NativeResult<()> {
    let Some(key) = open(KEY_QUERY_VALUE | KEY_SET_VALUE)? else {
        return Err(Fault::new(FaultKind::Gone, "Startup entry is already absent."));
    };
    let Some((current, kind)) = query(key.0, &entry.value_name)? else {
        return Ok(());
    };
    if current != entry.command || kind != entry.kind {
        return Err(Fault::new(
            FaultKind::Denied,
            "Startup entry changed after review; it was not disabled.",
        ));
    }
    let status = unsafe { RegDeleteValueW(key.0, wide(&entry.value_name).as_ptr()) };
    if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
        return Err(fault("Disable startup value", status));
    }
    if query(key.0, &entry.value_name)?.is_some() {
        return Err(Fault::new(
            FaultKind::Other,
            "Startup value still exists after disable request.",
        ));
    }
    Ok(())
}

pub fn restore(value_name: &str, command: &str, kind: &StartupValueKind) -> NativeResult<()> {
    let Some(key) = open(KEY_QUERY_VALUE | KEY_SET_VALUE)? else {
        return Err(Fault::new(
            FaultKind::Gone,
            "Current-user startup registry key is unavailable.",
        ));
    };
    if let Some((current, current_kind)) = query(key.0, value_name)? {
        if current == command && &current_kind == kind {
            return Ok(());
        }
        return Err(Fault::new(
            FaultKind::Denied,
            "A different startup value now uses this name; recovery will not overwrite it.",
        ));
    }
    let data = wide(command);
    let registry_kind = match kind {
        StartupValueKind::String => REG_SZ,
        StartupValueKind::ExpandString => REG_EXPAND_SZ,
    };
    let status = unsafe {
        RegSetValueExW(
            key.0,
            wide(value_name).as_ptr(),
            0,
            registry_kind,
            data.as_ptr().cast(),
            (data.len() * 2) as u32,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(fault("Restore startup value", status));
    }
    match query(key.0, value_name)? {
        Some((current, current_kind)) if current == command && &current_kind == kind => Ok(()),
        _ => Err(Fault::new(
            FaultKind::Other,
            "Startup value could not be verified after restore.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_plain_executable_tokens() {
        assert_eq!(
            command_executable(r#""C:\Program Files\Example\app.exe" --background"#),
            Some(r"c:\program files\example\app.exe".into())
        );
        assert_eq!(
            command_executable(r"C:\Tools\tool.exe --quiet"),
            Some(r"c:\tools\tool.exe".into())
        );
        assert_eq!(command_executable(r"%LOCALAPPDATA%\App\app.exe"), None);
    }
}
