use super::process::{self, SessionLock, WindowsBackend};
use crate::{
    engine,
    journal::{Database, Journal},
    model::*,
    policy,
};
use std::os::windows::process::CommandExt;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn state_dir() -> AppResult<PathBuf> {
    static PATH: std::sync::OnceLock<AppResult<PathBuf>> = std::sync::OnceLock::new();
    PATH.get_or_init(super::storage::prepare_state_directory)
        .clone()
}

pub fn database() -> AppResult<Database> {
    let path = state_dir()?;
    super::storage::verify_store_entries(&path)?;
    Database::open(&path.join("state.sqlite3"))
}

pub fn fresh_id() -> String {
    format!(
        "{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id()
    )
}

pub fn spawn_worker(mode: &str, id: Option<&str>) -> AppResult<()> {
    let mut command = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    command.arg(mode).creation_flags(0x08000000); // CREATE_NO_WINDOW, no elevation.
    if let Some(id) = id {
        command.arg(id);
    }
    command.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

fn normal_token_required() -> AppResult<()> {
    if process::is_elevated().map_err(|e| e.message)? {
        Err("Run Process Optimizer normally, not as administrator. This version deliberately does not operate with elevated privileges.".into())
    } else {
        Ok(())
    }
}

fn attach_or_launch(plan: &Plan) -> AppResult<Identity> {
    let game = plan
        .game
        .as_ref()
        .ok_or("Start the actual game normally, refresh, and select its exact running process.")?;
    if plan.options.launch_game {
        return Err(
            "Automatic launcher tracking is not implemented; use an existing game process.".into(),
        );
    }
    let rows = process::enumerate().map_err(|e| e.message)?;
    let row = rows.iter().find(|r| policy::same_process(&r.identity, game))
        .ok_or("The selected game lifetime ended or changed. Refresh and select it again; no background app was changed.")?;
    if let Some(reason) = &row.protected_reason {
        return Err(format!(
            "The selected process is protected infrastructure, not an eligible game: {reason}"
        ));
    }
    Ok(game.clone())
}

pub fn run(id: &str) -> AppResult<()> {
    normal_token_required()?;
    let _lock = SessionLock::acquire()?;
    let mut db = database()?;
    let mut s = db.get(id)?;
    if s.stage != Stage::Pending {
        return Err(
            "This session is not pending; destructive actions will not be replayed.".into(),
        );
    }
    policy::validate(&s.plan)?;
    s.worker = Some(process::current_identity().map_err(|e| e.message)?);
    db.save(&s)?;
    if db.stop_requested(id)? {
        s.stage = Stage::Restored;
        s.note("Cancelled before any mutation.");
        db.save(&s)?;
        return Ok(());
    }
    let game = match attach_or_launch(&s.plan) {
        Ok(g) => g,
        Err(e) => {
            s.note(e);
            s.stage = Stage::Restored;
            db.save(&s)?;
            return Ok(());
        }
    };
    s.game = Some(game.clone());
    db.save(&s)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let monitor_cancel = cancel.clone();
    let monitor_done = done.clone();
    let monitor_game = game.clone();
    let monitor_id = id.to_owned();
    let monitor = thread::spawn(move || {
        let db = database();
        let handle = process::exact_handle(&monitor_game, 0);
        while !monitor_done.load(Ordering::Relaxed) {
            let should_stop = handle.as_ref().map_or(true, |h| unsafe { windows_sys::Win32::System::Threading::WaitForSingleObject(h.0, 0) } != windows_sys::Win32::Foundation::WAIT_TIMEOUT)
                || db
                    .as_ref()
                    .map_or(true, |db| db.stop_requested(&monitor_id).unwrap_or(true));
            if should_stop {
                monitor_cancel.store(true, Ordering::Relaxed);
                break;
            }
            thread::sleep(Duration::from_millis(500));
        }
    });
    let result = (|| {
        let mut backend = WindowsBackend::new(
            Some(game.clone()),
            s.plan.protected_paths.clone(),
            Some(cancel.clone()),
        )
        .map_err(|e| e.message)?;
        if let Err(e) = engine::apply(&mut s, &mut backend, &mut db) {
            s.note(format!("Apply interrupted: {e}. Attempting recovery."));
            cancel.store(true, Ordering::Relaxed);
        }
        while !cancel.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(250));
        }
        // Fresh backend removes the active-game/cancel gate; identity/security checks remain.
        let mut recovery = WindowsBackend::new(None, vec![], None).map_err(|e| e.message)?;
        engine::restore(&mut s, &mut recovery, &mut db)
    })();
    done.store(true, Ordering::Relaxed);
    let _ = monitor.join();
    if let Err(ref e) = result {
        s.stage = Stage::RecoveryNeeded;
        s.note(format!("Recovery worker error: {e}"));
        let _ = db.save(&s);
    }
    result
}

pub fn recover() -> AppResult<()> {
    normal_token_required()?;
    let _lock = SessionLock::acquire()?;
    let mut db = database()?;
    let Some(mut s) = db.active()? else {
        return Ok(());
    };
    s.worker = Some(process::current_identity().map_err(|e| e.message)?);
    let mut backend = WindowsBackend::new(None, vec![], None).map_err(|e| e.message)?;
    engine::restore(&mut s, &mut backend, &mut db)?;
    Ok(())
}

/// Explicit user acknowledgement is not reported as successful restoration.
pub fn acknowledge() -> AppResult<()> {
    normal_token_required()?;
    let _lock = SessionLock::acquire()?;
    let mut db = database()?;
    let Some(mut s) = db.active()? else {
        return Ok(());
    };
    if s.stage != Stage::RecoveryNeeded {
        return Err("Only a reviewed recovery warning can be acknowledged.".into());
    }
    for change in &mut s.changes {
        if !change.state.resolved() {
            change.state = ChangeState::UserKept;
            change.detail =
                "User explicitly chose to retain current settings instead of restoring this value."
                    .into();
        }
    }
    for item in &mut s.reopened {
        if !item.state.resolved() {
            item.state = ReopenState::UserKept;
            item.detail = "User acknowledged an uncertain launch; no launch was retried.".into();
        }
    }
    s.note("User accepted unresolved settings. This is not a fully restored session.");
    s.stage = Stage::UserAcknowledged;
    db.save(&s)?;
    Ok(())
}

pub fn write_probe(path: &Path) -> AppResult<()> {
    let snapshot = super::gpu::snapshot()?;
    let body = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
    std::fs::write(path, body).map_err(|e| e.to_string())
}
