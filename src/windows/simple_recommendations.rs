//! Process classification, safety categorization, and recommendation logic for the Simple UI.
use super::startup::RunEntry;
use crate::{
    gpu::{busiest_engine, dedicated_allocations},
    manual::{Config, Rule, RuleAction},
    model::*,
    policy,
};
use std::collections::BTreeMap;

pub struct Choice {
    pub path: String,
    pub name: String,
    pub id: Option<Identity>,
    pub protected: bool,
    pub essential: bool,
    pub gpu: Option<f64>,
    pub startup_enabled: bool,
    pub startup_disabled: bool,
}

pub fn describe(name: &str) -> (&'static str, &'static str) {
    let n = name.to_ascii_lowercase();
    if matches!(
        n.as_str(),
        "chrome.exe" | "msedge.exe" | "firefox.exe" | "brave.exe"
    ) {
        ("Web browser", "Usually a Game Mode-only choice")
    } else if n.contains("nvidia overlay")
        || matches!(
            n.as_str(),
            "rtss.exe" | "rtsshooksloader64.exe" | "msiafterburner.exe" | "radeonsoftware.exe"
        )
    {
        (
            "Overlay / monitoring",
            "Close for gaming only if you do not use its overlay or capture",
        )
    } else if matches!(
        n.as_str(),
        "windowsterminal.exe" | "powershell.exe" | "cncmd.exe" | "code.exe"
    ) {
        ("Developer tool", "Usually a Game Mode-only choice")
    } else if matches!(
        n.as_str(),
        "applephotostreams.exe" | "apsdaemon.exe" | "mdnsresponder.exe"
    ) {
        (
            "Apple sync / discovery",
            "Permanent startup cleanup can make sense if unused",
        )
    } else if n.contains("apogee") || n.contains("antelopeaudio") {
        (
            "Audio hardware software",
            "Keep if you use that audio device",
        )
    } else if n.starts_with("asus") {
        (
            "ASUS utility",
            "Review carefully; device hotkeys or power features may depend on it",
        )
    } else if n.contains("nvbroadcast") {
        (
            "NVIDIA Broadcast",
            "Keep if you use microphone or camera effects",
        )
    } else if n == "everything.exe" {
        (
            "File search utility",
            "Game Mode or startup cleanup if you do not need it",
        )
    } else if n == "msedgewebview2.exe" {
        (
            "App web component",
            "Keep unless you understand which parent app owns it",
        )
    } else if n == "postgres.exe" || n == "pg_ctl.exe" {
        (
            "Developer database",
            "Use a proper database stop; do not force-close it",
        )
    } else {
        ("Optional app", "Review before changing")
    }
}

pub fn essential_process(name: &str, reason: Option<&str>) -> bool {
    let n = name.to_ascii_lowercase();
    if matches!(
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
            | "audiodg.exe"
            | "fontdrvhost.exe"
            | "sihost.exe"
            | "ctfmon.exe"
            | "msmpeng.exe"
            | "mssense.exe"
            | "securityhealthservice.exe"
    ) {
        return true;
    }
    reason.is_some_and(|r| {
        let r = r.to_ascii_lowercase();
        r.contains("critical windows process") || r.contains("windows component: protected")
    })
}

pub fn recommendation(c: &Choice) -> &'static str {
    if c.essential {
        return "KEEP — REQUIRED";
    }
    if c.protected {
        return "KEEP — PROTECTED";
    }
    let n = c.name.to_ascii_lowercase();
    if n.contains("apogee")
        || n.contains("antelopeaudio")
        || n.starts_with("asus")
        || n.contains("nvbroadcast")
        || n == "msedgewebview2.exe"
        || n == "postgres.exe"
        || n == "pg_ctl.exe"
    {
        return "KEEP / REVIEW";
    }
    if c.startup_enabled
        && (matches!(
            n.as_str(),
            "applephotostreams.exe"
                | "apsdaemon.exe"
                | "mdnsresponder.exe"
                | "everything.exe"
                | "onedrive.exe"
                | "dropbox.exe"
                | "googledrivefs.exe"
        ) || n.contains("updater"))
    {
        return "DISABLE STARTUP IF UNUSED";
    }
    if matches!(
        n.as_str(),
        "chrome.exe"
            | "msedge.exe"
            | "firefox.exe"
            | "brave.exe"
            | "windowsterminal.exe"
            | "powershell.exe"
            | "cncmd.exe"
            | "code.exe"
            | "rtss.exe"
            | "rtsshooksloader64.exe"
            | "msiafterburner.exe"
            | "radeonsoftware.exe"
            | "everything.exe"
            | "onedrive.exe"
            | "dropbox.exe"
            | "googledrivefs.exe"
    ) || n.contains("nvidia overlay")
        || c.gpu.is_some_and(|v| v >= 1.0)
    {
        return "LOWER PRIORITY";
    }
    "KEEP / REVIEW"
}

pub fn build_choices(
    processes: &[ProcessRow],
    config: &Config,
    startup_entries: &[RunEntry],
) -> Vec<Choice> {
    let mut choices = BTreeMap::new();
    for row in processes {
        let complete = policy::complete_identity(&row.identity);
        let essential = essential_process(&row.name, row.protected_reason.as_deref());
        if !complete && !essential {
            continue;
        }
        let path = if complete {
            policy::normalized_path(&row.identity.path)
        } else {
            format!(
                "protected://{}:{}",
                row.name.to_ascii_lowercase(),
                row.identity.pid
            )
        };
        let entry = choices.entry(path.clone()).or_insert_with(|| Choice {
            path,
            name: row.name.clone(),
            id: if complete {
                Some(row.identity.clone())
            } else {
                None
            },
            protected: false,
            essential,
            gpu: None,
            startup_enabled: false,
            startup_disabled: false,
        });
        entry.protected |= row.protected_reason.is_some() || essential;
        entry.essential |= essential;
        if let Some(v) = busiest_engine(&row.gpu) {
            entry.gpu = Some(entry.gpu.unwrap_or(0.0).max(v));
        }
    }
    for rule in &config.rules {
        choices
            .entry(rule.app.path.clone())
            .or_insert_with(|| Choice {
                path: rule.app.path.clone(),
                name: rule.app.path.rsplit('\\').next().unwrap_or("App").into(),
                id: None,
                protected: false,
                essential: false,
                gpu: None,
                startup_enabled: false,
                startup_disabled: false,
            });
    }
    for backup in &config.startup_disabled {
        choices
            .entry(backup.app.path.clone())
            .or_insert_with(|| Choice {
                path: backup.app.path.clone(),
                name: backup.app.path.rsplit('\\').next().unwrap_or("App").into(),
                id: None,
                protected: false,
                essential: false,
                gpu: None,
                startup_enabled: false,
                startup_disabled: true,
            });
    }
    for c in choices.values_mut() {
        c.startup_enabled = startup_entries
            .iter()
            .any(|e| e.executable.as_deref() == Some(c.path.as_str()));
        c.startup_disabled = config.startup_disabled.iter().any(|b| b.app.path == c.path);
    }
    let mut list: Vec<Choice> = choices.into_values().collect();
    list.sort_by_key(|c| {
        (
            (c.protected || c.essential),
            c.essential,
            c.name.to_lowercase(),
        )
    });
    list
}

pub fn format_choice_row(c: &Choice, config: &Config, now: u64) -> String {
    let game = config
        .rules
        .iter()
        .find(|r| r.app.path == c.path)
        .map(|r| {
            if !r.active(now) {
                "Keep (approval expired)"
            } else {
                match r.action {
                    RuleAction::Close => "Close",
                    RuleAction::ReduceLoad => "Lower priority",
                }
            }
        })
        .unwrap_or("Keep");
    let category = if c.essential {
        "Windows / core"
    } else if c.protected {
        "Protected app"
    } else {
        describe(&c.name).0
    };
    let value = c.gpu.map(|v| format!(" | GPU {v:.1}%")).unwrap_or_default();
    let startup = if c.startup_disabled {
        "Disabled"
    } else if c.startup_enabled {
        "Keep"
    } else {
        "N/A"
    };
    let safety = if c.essential {
        "ESSENTIAL — KEEP"
    } else if c.protected {
        "PROTECTED — KEEP"
    } else {
        "OPTIONAL"
    };
    format!(
        "{} | {} — {} | Game: {} | Startup: {} | RECOMMENDED: {}{}{}",
        safety,
        c.name,
        category,
        game,
        startup,
        recommendation(c),
        if c.id.is_none() { " | Not running" } else { "" },
        value
    )
}

pub fn format_choice_detail(c: &Choice, rule: Option<&Rule>) -> String {
    let game = match rule.map(|r| &r.action) {
        Some(RuleAction::Close) => "Close",
        Some(RuleAction::ReduceLoad) => "Lower priority",
        None => "Keep",
    };
    let startup = if c.startup_disabled {
        "Disabled"
    } else if c.startup_enabled {
        "Keep / starts with Windows"
    } else {
        "No supported startup entry"
    };
    let (category, guidance) = if c.essential {
        ("Windows / core", "Required; do not change")
    } else if c.protected {
        ("Protected app", "Kept outside optimizer actions")
    } else {
        describe(&c.name)
    };
    let safety = if c.essential {
        "ESSENTIAL — KEEP. This Windows/core process is never an optimization target."
    } else if c.protected {
        "PROTECTED — KEEP. Process Optimizer will not change this app."
    } else {
        "Optional app."
    };
    format!(
        "{} {}: {}. Current: Game Mode = {}; Startup = {}. Recommended: {}. Turn OFF restores priority changes already applied.",
        safety,
        category,
        guidance,
        game,
        startup,
        recommendation(c)
    )
}

pub fn format_process_report(processes: &[ProcessRow]) -> String {
    let mut rows: Vec<_> = processes.iter().collect();
    rows.sort_by_key(|r| (r.name.to_ascii_lowercase(), r.identity.pid));
    let mut text = String::from("Process Optimizer — copyable process report\r\nNo executable paths, command lines, documents, or account IDs are included.\r\n\r\nPID\tApplication\tGPU peak\tDedicated MiB\tStatus\r\n");
    for row in rows {
        let gpu = busiest_engine(&row.gpu)
            .map(|v| format!("{v:.1}%"))
            .unwrap_or_else(|| "-".into());
        let mem = dedicated_allocations(&row.gpu)
            .map(|b| format!("{:.1}", b as f64 / 1024.0 / 1024.0))
            .unwrap_or_else(|| "-".into());
        let status = row
            .protected_reason
            .as_deref()
            .unwrap_or("Optional / not approved");
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\r\n",
            row.identity.pid, row.name, gpu, mem, status
        ));
    }
    text
}

#[cfg(test)]
mod recommendation_tests {
    use super::*;

    fn choice(
        name: &str,
        protected: bool,
        essential: bool,
        startup: bool,
        gpu: Option<f64>,
    ) -> Choice {
        Choice {
            path: format!(r"c:\apps\{name}"),
            name: name.into(),
            id: None,
            protected,
            essential,
            gpu,
            startup_enabled: startup,
            startup_disabled: false,
        }
    }

    #[test]
    fn essential_and_protected_are_always_keep_recommendations() {
        assert_eq!(
            recommendation(&choice("svchost.exe", true, true, false, Some(20.0))),
            "KEEP — REQUIRED"
        );
        assert_eq!(
            recommendation(&choice("steam.exe", true, false, false, Some(20.0))),
            "KEEP — PROTECTED"
        );
    }

    #[test]
    fn browser_and_gpu_active_optional_apps_prefer_lower_priority() {
        assert_eq!(
            recommendation(&choice("chrome.exe", false, false, false, None)),
            "LOWER PRIORITY"
        );
        assert_eq!(
            recommendation(&choice("optional.exe", false, false, false, Some(2.0))),
            "LOWER PRIORITY"
        );
    }

    #[test]
    fn unused_sync_style_startup_apps_get_persistent_cleanup_hint() {
        assert_eq!(
            recommendation(&choice("Everything.exe", false, false, true, None)),
            "DISABLE STARTUP IF UNUSED"
        );
    }
}
