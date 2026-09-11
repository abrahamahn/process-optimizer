#[cfg(windows)]
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        if let Some(path) = std::env::args()
            .collect::<Vec<_>>()
            .windows(2)
            .find(|pair| pair[0] == "--error-file")
            .map(|pair| pair[1].clone())
        {
            let _ = std::fs::write(path, &error);
        }
        if !std::env::args().any(|arg| arg == "--test-no-restart") {
            show_error(&error);
        }
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("The Process Optimizer updater runs on Windows.");
}

#[cfg(windows)]
fn value(args: &[String], name: &str) -> Result<String, String> {
    let index = args
        .iter()
        .position(|arg| arg == name)
        .ok_or_else(|| format!("Missing updater argument {name}."))?;
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("Missing value for updater argument {name}."))
}

#[cfg(windows)]
fn wait_for_parent(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_TIMEOUT},
        System::Threading::{OpenProcess, WaitForSingleObject},
    };
    const SYNCHRONIZE_ACCESS: u32 = 0x0010_0000;
    if pid == 0 {
        return Ok(());
    }
    unsafe {
        let handle = OpenProcess(SYNCHRONIZE_ACCESS, 0, pid);
        if handle.is_null() {
            // The parent may already be gone between spawn and this call.
            return Ok(());
        }
        let result = WaitForSingleObject(handle, 30_000);
        CloseHandle(handle);
        if result == WAIT_TIMEOUT {
            Err("Process Optimizer did not exit within 30 seconds; update was cancelled.".into())
        } else {
            Ok(())
        }
    }
}

#[cfg(windows)]
fn backup(path: &std::path::Path) -> Result<Option<std::path::PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let backup = path.with_extension("exe.old");
    let _ = std::fs::remove_file(&backup);
    std::fs::copy(path, &backup).map_err(|e| format!("Could not create update backup: {e}"))?;
    Ok(Some(backup))
}

#[cfg(windows)]
fn restore(target: &std::path::Path, backup: &Option<std::path::PathBuf>) {
    if let Some(backup) = backup {
        let _ = std::fs::copy(backup, target);
    } else {
        let _ = std::fs::remove_file(target);
    }
}

#[cfg(windows)]
fn run() -> Result<(), String> {
    use process_optimizer::update;
    use std::{path::PathBuf, process::Command, thread, time::Duration};

    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|arg| arg == "--apply") {
        return Err("Updater must be launched by Process Optimizer.".into());
    }
    let install_dir = PathBuf::from(value(&args, "--install-dir")?);
    let pending_app = PathBuf::from(value(&args, "--app")?);
    let pending_updater = PathBuf::from(value(&args, "--updater")?);
    let app_hash = value(&args, "--app-sha256")?;
    let updater_hash = value(&args, "--updater-sha256")?;
    let parent_pid = value(&args, "--parent-pid")?
        .parse::<u32>()
        .map_err(|_| "Updater parent PID is invalid.".to_string())?;
    let no_restart = args.iter().any(|arg| arg == "--test-no-restart");

    if !install_dir.is_dir() {
        return Err("Installed application directory no longer exists.".into());
    }
    update::verify_file(&pending_app, &app_hash)?;
    update::verify_file(&pending_updater, &updater_hash)?;
    wait_for_parent(parent_pid)?;

    let target_app = install_dir.join("process-optimizer.exe");
    let target_updater = install_dir.join("process-optimizer-updater.exe");
    let app_backup = backup(&target_app)?;
    let updater_backup = backup(&target_updater)?;

    let apply = (|| -> Result<(), String> {
        std::fs::copy(&pending_updater, &target_updater)
            .map_err(|e| format!("Could not install updater: {e}"))?;
        update::verify_file(&target_updater, &updater_hash)?;
        std::fs::copy(&pending_app, &target_app)
            .map_err(|e| format!("Could not install Process Optimizer: {e}"))?;
        update::verify_file(&target_app, &app_hash)?;
        Ok(())
    })();

    if let Err(error) = apply {
        restore(&target_app, &app_backup);
        restore(&target_updater, &updater_backup);
        return Err(format!(
            "Update failed and the previous files were restored. {error}"
        ));
    }

    if no_restart {
        if let Some(path) = app_backup {
            let _ = std::fs::remove_file(path);
        }
        if let Some(path) = updater_backup {
            let _ = std::fs::remove_file(path);
        }
        return Ok(());
    }

    let mut child = match Command::new(&target_app).spawn() {
        Ok(child) => child,
        Err(error) => {
            restore(&target_app, &app_backup);
            restore(&target_updater, &updater_backup);
            let _ = Command::new(&target_app).spawn();
            return Err(format!(
                "The updated app could not restart and the previous version was restored: {error}"
            ));
        }
    };
    thread::sleep(Duration::from_millis(900));
    if let Ok(Some(status)) = child.try_wait() {
        if !status.success() {
            restore(&target_app, &app_backup);
            restore(&target_updater, &updater_backup);
            let _ = Command::new(&target_app).spawn();
            return Err(
                "The updated app exited immediately; the previous version was restored.".into(),
            );
        }
    }

    if let Some(path) = app_backup {
        let _ = std::fs::remove_file(path);
    }
    if let Some(path) = updater_backup {
        let _ = std::fs::remove_file(path);
    }
    let _ = std::fs::remove_file(&pending_app);
    // pending_updater is the currently running image and is intentionally left for the next cleanup.
    Ok(())
}

#[cfg(windows)]
fn show_error(error: &str) {
    use process_optimizer::windows::wide;
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide(error).as_ptr(),
            wide("Process Optimizer update failed").as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}
