use super::{runner, wide};
use crate::{model::AppResult, update as contract};
use std::{
    ffi::c_void,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::HWND,
    Networking::WinHttp::*,
    UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_DEFBUTTON2, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_YESNO,
    },
};

const MAX_DOWNLOAD_BYTES: usize = 64 * 1024 * 1024;

struct Internet(*mut c_void);
impl Drop for Internet {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                WinHttpCloseHandle(self.0);
            }
        }
    }
}

fn split_https(url: &str) -> AppResult<(String, String)> {
    let rest = url
        .strip_prefix("https://")
        .ok_or("Only HTTPS update URLs are accepted.")?;
    let (host, path) = rest
        .split_once('/')
        .ok_or("Update URL is missing a path.")?;
    if host != "github.com" {
        return Err("Update URL host is not the expected GitHub host.".into());
    }
    Ok((host.into(), format!("/{path}")))
}

fn get_https(url: &str, max_bytes: usize) -> AppResult<Vec<u8>> {
    let (host, path) = split_https(url)?;
    let agent = wide(&format!("ProcessOptimizer/{}", env!("CARGO_PKG_VERSION")));
    let host = wide(&host);
    let path = wide(&path);
    let verb = wide("GET");
    let headers = wide(&format!(
        "User-Agent: ProcessOptimizer/{}\r\nAccept: application/octet-stream\r\n",
        env!("CARGO_PKG_VERSION")
    ));

    unsafe {
        let session = Internet(WinHttpOpen(
            agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            null(),
            null(),
            0,
        ));
        if session.0.is_null() {
            return Err(format!(
                "Could not open update network session: {}",
                std::io::Error::last_os_error()
            ));
        }
        if WinHttpSetTimeouts(session.0, 10_000, 10_000, 30_000, 30_000) == 0 {
            return Err(format!(
                "Could not configure update network timeouts: {}",
                std::io::Error::last_os_error()
            ));
        }
        let connect = Internet(WinHttpConnect(
            session.0,
            host.as_ptr(),
            INTERNET_DEFAULT_HTTPS_PORT,
            0,
        ));
        if connect.0.is_null() {
            return Err(format!(
                "Could not connect to GitHub: {}",
                std::io::Error::last_os_error()
            ));
        }
        let request = Internet(WinHttpOpenRequest(
            connect.0,
            verb.as_ptr(),
            path.as_ptr(),
            null(),
            null(),
            null(),
            WINHTTP_FLAG_SECURE,
        ));
        if request.0.is_null() {
            return Err(format!(
                "Could not create update request: {}",
                std::io::Error::last_os_error()
            ));
        }
        let header_len = (headers.len().saturating_sub(1)) as u32;
        if WinHttpSendRequest(
            request.0,
            headers.as_ptr(),
            header_len,
            null(),
            0,
            0,
            0,
        ) == 0
        {
            return Err(format!(
                "Could not send update request: {}",
                std::io::Error::last_os_error()
            ));
        }
        if WinHttpReceiveResponse(request.0, null_mut()) == 0 {
            return Err(format!(
                "Could not receive update response: {}",
                std::io::Error::last_os_error()
            ));
        }

        let mut output = Vec::new();
        let mut chunk = [0u8; 64 * 1024];
        loop {
            let mut read = 0u32;
            if WinHttpReadData(
                request.0,
                chunk.as_mut_ptr().cast::<c_void>(),
                chunk.len() as u32,
                &mut read,
            ) == 0
            {
                return Err(format!(
                    "Could not read update response: {}",
                    std::io::Error::last_os_error()
                ));
            }
            if read == 0 {
                break;
            }
            output.extend_from_slice(&chunk[..read as usize]);
            if output.len() > max_bytes {
                return Err("Update download exceeded the safety size limit.".into());
            }
        }
        if output.is_empty() {
            return Err("GitHub returned an empty update response.".into());
        }
        Ok(output)
    }
}

pub fn check_latest() -> AppResult<Option<contract::Manifest>> {
    let bytes = get_https(contract::LATEST_MANIFEST_URL, 256 * 1024)?;
    let manifest = contract::parse_manifest(&bytes)?;
    if contract::is_newer(env!("CARGO_PKG_VERSION"), &manifest.version)? {
        Ok(Some(manifest))
    } else {
        Ok(None)
    }
}

fn update_dir() -> AppResult<PathBuf> {
    let path = runner::state_dir()?.join("updates");
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

fn download_verified(url: &str, expected: &str, destination: &Path) -> AppResult<()> {
    let bytes = get_https(url, MAX_DOWNLOAD_BYTES)?;
    if contract::sha256_bytes(&bytes).eq_ignore_ascii_case(expected) {
        std::fs::write(destination, bytes).map_err(|e| e.to_string())?;
        contract::verify_file(destination, expected)
    } else {
        Err("Downloaded update failed SHA-256 verification and was not saved.".into())
    }
}

fn prepare(manifest: &contract::Manifest) -> AppResult<(PathBuf, PathBuf)> {
    manifest.validate()?;
    let dir = update_dir()?;
    let app = dir.join(format!("process-optimizer-{}.exe", manifest.version));
    let updater = dir.join(format!("process-optimizer-updater-{}.exe", manifest.version));
    let _ = std::fs::remove_file(&app);
    let _ = std::fs::remove_file(&updater);
    download_verified(&manifest.app_url, &manifest.app_sha256, &app)?;
    if let Err(error) = download_verified(&manifest.updater_url, &manifest.updater_sha256, &updater) {
        let _ = std::fs::remove_file(&app);
        return Err(error);
    }
    Ok((app, updater))
}

fn launch_updater(
    manifest: &contract::Manifest,
    pending_app: &Path,
    pending_updater: &Path,
) -> AppResult<()> {
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let install_dir = current
        .parent()
        .ok_or("Could not determine the installed application directory.")?;
    let mut command = Command::new(pending_updater);
    command
        .arg("--apply")
        .arg("--install-dir")
        .arg(install_dir)
        .arg("--app")
        .arg(pending_app)
        .arg("--updater")
        .arg(pending_updater)
        .arg("--app-sha256")
        .arg(&manifest.app_sha256)
        .arg("--updater-sha256")
        .arg(&manifest.updater_sha256)
        .arg("--parent-pid")
        .arg(std::process::id().to_string())
        .creation_flags(0x08000000);
    command
        .spawn()
        .map_err(|e| format!("Could not launch updater: {e}"))?;
    Ok(())
}

fn confirm(window: HWND, text: &str) -> bool {
    unsafe {
        MessageBoxW(
            window,
            wide(text).as_ptr(),
            wide("Process Optimizer update").as_ptr(),
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
        ) == IDYES
    }
}

fn info(window: HWND, text: &str) {
    unsafe {
        MessageBoxW(
            window,
            wide(text).as_ptr(),
            wide("Process Optimizer update").as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}

/// Returns true only after a verified updater process has been launched and the UI should exit.
pub fn interactive(window: HWND) -> AppResult<bool> {
    if runner::database()?.active()?.is_some() {
        return Err("Turn Game Mode OFF and finish recovery before updating Process Optimizer.".into());
    }
    let Some(manifest) = check_latest()? else {
        info(
            window,
            &format!(
                "You are up to date.\n\nInstalled version: {}",
                env!("CARGO_PKG_VERSION")
            ),
        );
        return Ok(false);
    };
    let message = format!(
        "Process Optimizer {} is available.\n\nInstalled: {}\nAvailable: {}\n\nDownload, verify, replace the local app, and restart now?",
        manifest.version,
        env!("CARGO_PKG_VERSION"),
        manifest.version
    );
    if !confirm(window, &message) {
        return Ok(false);
    }
    let (app, updater) = prepare(&manifest)?;
    launch_updater(&manifest, &app, &updater)?;
    info(
        window,
        "The update was downloaded and verified. Process Optimizer will close, install it, and restart.",
    );
    Ok(true)
}
