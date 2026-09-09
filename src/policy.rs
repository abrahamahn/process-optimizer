use crate::model::*;
use std::collections::HashSet;

pub fn normalized_path(path: &str) -> String {
    path.trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .to_lowercase()
}

pub fn same_process(a: &Identity, b: &Identity) -> bool {
    a.pid == b.pid
        && a.created == b.created
        && a.session_id == b.session_id
        && normalized_path(&a.path) == normalized_path(&b.path)
}

pub fn validate(plan: &Plan) -> AppResult<()> {
    if !plan.consent {
        return Err("Explicit session approval is required.".into());
    }
    if plan.game_path.trim().is_empty() {
        return Err("Choose the actual game executable, not its launcher.".into());
    }
    if plan.actions.len() > 128 {
        return Err("At most 128 explicitly selected processes per session.".into());
    }
    if !(1000..=15000).contains(&plan.options.close_timeout_ms) {
        return Err("Close timeout must be between 1 and 15 seconds.".into());
    }
    let mut pids = HashSet::new();
    for item in &plan.actions {
        if item.target.pid <= 4 || item.target.created == 0 || item.target.path.is_empty() {
            return Err("A selected process has no trustworthy identity.".into());
        }
        if !pids.insert(item.target.pid) {
            return Err("Duplicate process in the plan.".into());
        }
        if normalized_path(&item.target.path) == normalized_path(&plan.game_path) {
            return Err("The game cannot be an optimization target.".into());
        }
        if plan
            .protected_paths
            .iter()
            .any(|p| normalized_path(p) == normalized_path(&item.target.path))
        {
            return Err("The plan contains an application you protected.".into());
        }
        if item.action == ActionKind::ForceClose && !plan.force_consent {
            return Err("Force termination needs its own explicit approval.".into());
        }
    }
    if let Some(game) = &plan.game {
        if normalized_path(&game.path) != normalized_path(&plan.game_path) {
            return Err("Selected game identity and executable do not match.".into());
        }
    }
    Ok(())
}

/// Protection wins over selection. Unknown processes are never automatically selected.
pub fn guarded_pids(rows: &[ProcessRow], game: &Identity) -> HashSet<u32> {
    let mut guard = HashSet::from([game.pid]);
    let mut parent = rows
        .iter()
        .find(|p| same_process(&p.identity, game))
        .map(|p| p.parent_pid)
        .unwrap_or(0);
    for _ in 0..64 {
        if parent == 0 || !guard.insert(parent) {
            break;
        }
        parent = rows
            .iter()
            .find(|p| p.identity.pid == parent)
            .map(|p| p.parent_pid)
            .unwrap_or(0);
    }
    // Only the game's descendants, not every child of a shared launcher, are guarded.
    let mut descendants = HashSet::from([game.pid]);
    for _ in 0..64 {
        let mut changed = false;
        for row in rows {
            if descendants.contains(&row.parent_pid) {
                changed |= descendants.insert(row.identity.pid);
            }
        }
        if !changed {
            break;
        }
    }
    guard.extend(descendants);
    guard
}

pub fn reserved_name(name: &str) -> bool {
    let n = name.to_lowercase();
    matches!(
        n.as_str(),
        "system"
            | "registry"
            | "secure system"
            | "memory compression"
            | "smss.exe"
            | "csrss.exe"
            | "wininit.exe"
            | "services.exe"
            | "lsass.exe"
            | "winlogon.exe"
            | "svchost.exe"
            | "dwm.exe"
            | "explorer.exe"
            | "audiodg.exe"
            | "fontdrvhost.exe"
            | "sihost.exe"
            | "ctfmon.exe"
            | "runtimebroker.exe"
            | "steam.exe"
            | "steamservice.exe"
            | "steamwebhelper.exe"
            | "epicgameslauncher.exe"
            | "epicwebhelper.exe"
            | "gamingservices.exe"
            | "gamingservicesnet.exe"
            | "gamebar.exe"
            | "xboxpcapp.exe"
            | "goggalaxy.exe"
            | "galaxyclient.exe"
            | "ubisoftconnect.exe"
            | "upc.exe"
            | "eadesktop.exe"
            | "eabackgroundservice.exe"
            | "battle.net.exe"
            | "riotclientservices.exe"
            | "discord.exe"
            | "teamspeak.exe"
            | "ts3client_win64.exe"
            | "nvcontainer.exe"
            | "nvdisplay.container.exe"
            | "atiesrxx.exe"
            | "atieclxx.exe"
            | "msmpeng.exe"
            | "mssense.exe"
            | "securityhealthservice.exe"
            | "vgc.exe"
            | "vgtray.exe"
            | "beservice.exe"
            | "beservice_x64.exe"
            | "easyanticheat.exe"
            | "easyanticheat_eos.exe"
            | "faceitservice.exe"
            | "process-optimizer.exe"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(pid: u32) -> Identity {
        Identity {
            pid,
            created: 100,
            path: format!(r"C:\Apps\{pid}.exe"),
            session_id: 1,
        }
    }
    fn plan() -> Plan {
        Plan {
            game_path: r"C:\Game\game.exe".into(),
            game: None,
            actions: vec![],
            protected_paths: vec![],
            options: Options::default(),
            consent: true,
            force_consent: false,
        }
    }
    #[test]
    fn no_consent_no_changes() {
        let mut p = plan();
        p.consent = false;
        assert!(validate(&p).is_err());
    }
    #[test]
    fn force_consent_is_separate() {
        let mut p = plan();
        p.actions.push(ApprovedAction {
            target: id(20),
            action: ActionKind::ForceClose,
        });
        assert!(validate(&p).is_err());
        p.force_consent = true;
        assert!(validate(&p).is_ok());
    }
    #[test]
    fn pid_reuse_is_not_same_process() {
        let a = id(20);
        let mut b = a.clone();
        b.created += 1;
        assert!(!same_process(&a, &b));
    }
    #[test]
    fn case_and_prefix_are_normalized() {
        assert_eq!(
            normalized_path(r"\\?\C:\Apps\FOO.exe"),
            normalized_path("c:/apps/foo.exe")
        );
    }
    #[test]
    fn protected_path_wins() {
        let mut p = plan();
        p.protected_paths.push(id(20).path);
        p.actions.push(ApprovedAction {
            target: id(20),
            action: ActionKind::Close,
        });
        assert!(validate(&p).is_err());
    }
    #[test]
    fn duplicate_pid_rejected() {
        let mut p = plan();
        let a = ApprovedAction {
            target: id(20),
            action: ActionKind::Close,
        };
        p.actions = vec![a.clone(), a];
        assert!(validate(&p).is_err());
    }
    #[test]
    fn game_cannot_be_target() {
        let mut p = plan();
        let mut i = id(20);
        i.path = p.game_path.clone();
        p.actions.push(ApprovedAction {
            target: i,
            action: ActionKind::Close,
        });
        assert!(validate(&p).is_err());
    }
    #[test]
    fn lower_never_raises_idle() {
        assert_eq!(Value::GpuPriority(0).background(), Value::GpuPriority(0));
        assert_eq!(
            Value::CpuPriority(0x40).background(),
            Value::CpuPriority(0x40)
        );
    }
    #[test]
    fn eco_preserves_other_flags() {
        assert_eq!(
            Value::EcoQos {
                control: 4,
                state: 4
            }
            .background(),
            Value::EcoQos {
                control: 5,
                state: 5
            }
        );
    }
    #[test]
    fn unknown_names_are_not_auto_classified() {
        assert!(!reserved_name("my-app.exe"));
        assert!(reserved_name("DWM.EXE"));
    }
    #[test]
    fn protects_game_family_without_sweeping_launcher_siblings() {
        let rows: Vec<_> = [(10, 0), (20, 10), (30, 20), (40, 10)]
            .into_iter()
            .map(|(p, parent)| ProcessRow {
                identity: id(p),
                parent_pid: parent,
                name: "fixture".into(),
                protected_reason: None,
                gpu: vec![],
            })
            .collect();
        let guard = guarded_pids(&rows, &id(20));
        assert!(guard.contains(&10) && guard.contains(&20) && guard.contains(&30));
        assert!(!guard.contains(&40));
    }
}
