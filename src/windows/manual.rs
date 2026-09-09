//! Native validation for recurring, normal-user-only Game Mode permissions.
use super::{process, runner};
use crate::{applications, manual, model::*, policy};
use std::{
    os::windows::fs::{MetadataExt, OpenOptionsExt},
    time::{SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn denied(s: impl Into<String>) -> Fault {
    Fault::new(FaultKind::Denied, s)
}

pub fn stamp(id: &Identity) -> NativeResult<ReopenApproval> {
    if !policy::complete_identity(id) {
        return Err(denied("Incomplete application identity."));
    }
    for p in std::path::Path::new(&id.path).ancestors() {
        if p.parent().is_none() {
            continue;
        }
        if std::fs::symlink_metadata(p)
            .map_err(|e| denied(e.to_string()))?
            .file_attributes()
            & 0x400
            != 0
        {
            return Err(denied("Recurring permissions cannot follow reparse paths."));
        }
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&id.path)
        .map_err(|e| denied(e.to_string()))?;
    let meta = file.metadata().map_err(|e| denied(e.to_string()))?;
    if !meta.is_file() {
        return Err(denied("Not a normal executable file."));
    }
    Ok(ReopenApproval {
        image_file_id: process::image_file_id_from_file(&file)?,
        file_size: meta.file_size(),
        modified: meta.last_write_time(),
    })
}

pub fn foreground_path() -> NativeResult<Option<String>> {
    let window = unsafe { GetForegroundWindow() };
    if window.is_null() {
        return Ok(None);
    }
    let mut pid = 0;
    if unsafe { GetWindowThreadProcessId(window, &mut pid) } == 0 || pid == 0 {
        return Err(denied("Cannot identify the foreground application."));
    }
    // Our native UI is the usual foreground window when On is pressed.
    if pid == std::process::id() {
        return Ok(None);
    }
    process::identity(pid).map(|id| Some(policy::normalized_path(&id.path)))
}

pub fn authorize_current_rule(id: &Identity) -> NativeResult<manual::RuleAction> {
    let config = runner::database()
        .and_then(|d| d.manual_settings())
        .map_err(denied)?;
    let observed = stamp(id)?;
    let rule = manual::matching_rule(&config, id, &observed, now()).ok_or_else(|| {
        denied("Background-app approval expired, changed or was revoked. Review Settings.")
    })?;
    if foreground_path()?.is_some_and(|p| p == rule.app.path) {
        return Err(denied(
            "The approved app is now in the foreground; Game Mode will not interrupt it.",
        ));
    }
    Ok(rule.action.clone())
}

pub fn build_plan(experimental_gpu: bool) -> AppResult<(Plan, usize)> {
    let db = runner::database()?;
    let config = db.manual_settings()?;
    let settings = db.settings()?;
    let caller = process::current_identity().map_err(|e| e.message)?;
    let foreground = foreground_path().map_err(|e| e.message)?;
    let mut rows = process::enumerate().map_err(|e| e.message)?;
    let mut excluded = 0;
    for row in &mut rows {
        if !applications::same_user_session(&row.identity, &caller) {
            continue;
        }
        let path = policy::normalized_path(&row.identity.path);
        if config.rules.iter().any(|r| r.app.path == path) {
            let valid = stamp(&row.identity).ok().is_some_and(|s| {
                manual::matching_rule(&config, &row.identity, &s, now()).is_some()
            });
            if !valid || foreground.as_ref() == Some(&path) {
                row.protected_reason =
                    Some("Approval is stale, file changed or app is in the foreground.".into());
                excluded += 1;
            }
        }
    }
    let plan = manual::plan(
        &config,
        &rows,
        &caller,
        &settings.protected_paths,
        now(),
        experimental_gpu,
    )?;
    Ok((plan, excluded))
}
