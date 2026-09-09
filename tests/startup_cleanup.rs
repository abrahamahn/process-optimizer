#![cfg(windows)]

use process_optimizer::{
    manual::StartupValueKind,
    windows::{startup, wide},
};
use std::{mem::size_of, ptr::null_mut, time::{SystemTime, UNIX_EPOCH}};
use windows_sys::Win32::{
    Foundation::ERROR_SUCCESS,
    System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegSetValueExW, HKEY, HKEY_CURRENT_USER,
        KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ,
    },
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

struct TestValue {
    key: HKEY,
    name: String,
}

impl Drop for TestValue {
    fn drop(&mut self) {
        unsafe {
            let name = wide(&self.name);
            let _ = RegDeleteValueW(self.key, name.as_ptr());
            RegCloseKey(self.key);
        }
    }
}

fn create_test_value(name: &str, command: &str) -> TestValue {
    let mut key = null_mut();
    let mut disposition = 0u32;
    let path = wide(RUN_KEY);
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            null_mut(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            null_mut(),
            &mut key,
            &mut disposition,
        )
    };
    assert_eq!(status, ERROR_SUCCESS, "could not create/open test-owned HKCU Run key");

    let name_w = wide(name);
    let command_w = wide(command);
    let bytes = command_w.len() * size_of::<u16>();
    let status = unsafe {
        RegSetValueExW(
            key,
            name_w.as_ptr(),
            0,
            REG_SZ,
            command_w.as_ptr().cast(),
            bytes as u32,
        )
    };
    assert_eq!(status, ERROR_SUCCESS, "could not create test-owned startup value");
    TestValue { key, name: name.into() }
}

#[test]
fn current_user_run_entry_round_trips_without_touching_real_startup_values() {
    let exe = std::env::current_exe().unwrap();
    let command = format!("\"{}\" --optimizer-startup-fixture", exe.display());
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let name = format!("ProcessOptimizerV04Test_{}_{}", std::process::id(), nonce);
    let _guard = create_test_value(&name, &command);

    let entry = startup::entries()
        .unwrap()
        .into_iter()
        .find(|entry| entry.value_name == name)
        .expect("test-owned startup value should be enumerated");
    assert_eq!(entry.command, command);
    assert_eq!(entry.kind, StartupValueKind::String);

    startup::disable(&entry).expect("test-owned startup value should disable cleanly");
    assert!(
        startup::entries().unwrap().into_iter().all(|entry| entry.value_name != name),
        "disabled test value must be absent"
    );

    startup::restore(&name, &command, &StartupValueKind::String)
        .expect("test-owned startup value should restore cleanly");
    let restored = startup::entries()
        .unwrap()
        .into_iter()
        .find(|entry| entry.value_name == name)
        .expect("restored test value should be present");
    assert_eq!(restored.command, command);

    assert!(
        startup::restore(&name, &command, &StartupValueKind::String).is_err(),
        "restore must refuse to overwrite an existing value with the same name"
    );
}
