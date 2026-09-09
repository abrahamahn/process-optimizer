use super::process::{self, SessionLock, WindowsBackend};
use crate::{
    engine,
    journal::{Database, Journal},
    model::*,
    policy,
};
use std::os::windows::{fs::MetadataExt, process::CommandExt};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn state_dir() -> AppResult<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable.")?;
    let path = PathBuf::from(base).join("ProcessOptimizer");
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.file_attributes() & 0x400 != 0 {
        return Err("State directory must be a normal, non-reparse directory.".into());
    }
    Ok(path)
}

pub fn database() -> AppResult<Database> {
    Database::open(&state_dir()?.join("state.sqlite3"))
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

fn find_game(plan: &Plan) -> AppResult<Option<Identity>> {
    if let Some(game) = &plan.game {
        match process::alive(game) {
            Ok(true) => return Ok(Some(game.clone())),
            Ok(false) => {}
            Err(e) => return Err(e.message),
        }
    }
    let rows = process::enumerate().map_err(|e| e.message)?;
    let matches: Vec<_> = rows
        .into_iter()
        .filter(|r| {
            r.identity.created != 0
                && policy::normalized_path(&r.identity.path)
                    == policy::normalized_path(&plan.game_path)
        })
        .collect();
    if matches.len() > 1 {
        return Err(
            "Multiple matching game processes. Select the exact running game in the process list."
                .into(),
        );
    }
    if let Some(row) = matches.into_iter().next() {
        if row.protected_reason.is_some() {
            return Err("The selected executable is protected or is a launcher/support process. Select the actual game.".into());
        }
        Ok(Some(row.identity))
    } else {
        Ok(None)
    }
}

fn attach_or_launch(plan: &Plan) -> AppResult<Identity> {
    if let Some(game) = find_game(plan)? {
        return Ok(game);
    }
    if !plan.options.launch_game {
        return Err("Start the game normally, refresh, and select its actual process; or enable Launch game.".into());
    }
    let path = Path::new(&plan.game_path);
    if !path.is_absolute()
        || !path.is_file()
        || path
            .extension()
            .and_then(|s| s.to_str())
            .is_none_or(|s| !s.eq_ignore_ascii_case("exe"))
    {
        return Err("Game launch requires an existing absolute .exe path.".into());
    }
    if policy::reserved_name(path.file_name().and_then(|v| v.to_str()).unwrap_or("")) {
        return Err("Select a game executable rather than a protected launcher.".into());
    }
    let mut launch = Command::new(path);
    if let Some(parent) = path.parent() {
        launch.current_dir(parent);
    }
    launch.spawn().map_err(|e| format!("Game launch failed: {e}. DRM titles may need to be launched normally through their launcher."))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if let Some(game) = find_game(plan)? {
            return Ok(game);
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err("No matching game process appeared. No background app was changed. Launch normally and attach to the actual game.".into())
}

pub fn run(id: &str) -> AppResult<()> {
    let _lock = SessionLock::acquire()?;
    normal_token_required()?;
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
        while !monitor_done.load(Ordering::Relaxed) {
            let should_stop = !matches!(process::alive(&monitor_game), Ok(true))
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
    export_report(&s).ok();
    result
}

pub fn recover() -> AppResult<()> {
    let _lock = SessionLock::acquire()?;
    normal_token_required()?;
    let mut db = database()?;
    let Some(mut s) = db.active()? else {
        return Ok(());
    };
    s.worker = Some(process::current_identity().map_err(|e| e.message)?);
    let mut backend = WindowsBackend::new(None, vec![], None).map_err(|e| e.message)?;
    engine::restore(&mut s, &mut backend, &mut db)?;
    export_report(&s)
}

/// Explicit user acknowledgement is not reported as successful restoration.
pub fn acknowledge() -> AppResult<()> {
    let _lock = SessionLock::acquire()?;
    normal_token_required()?;
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
    s.note("User accepted unresolved settings. This is not a fully restored session.");
    s.stage = Stage::UserAcknowledged;
    db.save(&s)?;
    export_report(&s)
}

fn export_report(s: &Session) -> AppResult<()> {
    let body = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(state_dir()?.join("last-session.json"), body).map_err(|e| e.to_string())
}

pub fn write_probe(path: &Path) -> AppResult<()> {
    let snapshot = super::gpu::snapshot()?;
    let body = serde_json::to_string_pretty(&snapshot).map_err(|e| e.to_string())?;
    std::fs::write(path, body).map_err(|e| e.to_string())
}
