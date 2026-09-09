//! User-toggled mode and explicit recurring background-app permissions.
//! No game detection, wildcard rules, future-process enforcement, force or reopening.
use crate::{
    applications::{self, AppKey},
    model::*,
    policy,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const APPROVAL_SECONDS: u64 = 30 * 24 * 60 * 60;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleAction {
    Close,
    ReduceLoad,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub app: AppKey,
    pub stamp: ReopenApproval,
    pub owner_sid: String,
    pub action: RuleAction,
    pub granted_unix: u64,
    pub expires_unix: u64,
}
impl Rule {
    pub fn active(&self, now: u64) -> bool {
        now >= self.granted_unix && now < self.expires_unix
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: u32,
    pub rules: Vec<Rule>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            schema: 1,
            rules: vec![],
        }
    }
}

pub fn validate(c: &Config) -> AppResult<()> {
    if c.schema != 1
        || c.rules.len() > 32
        || serde_json::to_vec(c).map_err(|e| e.to_string())?.len() > 256 * 1024
    {
        return Err(
            "Unsupported or oversized Game Mode settings. Existing settings were preserved.".into(),
        );
    }
    let mut paths = HashSet::new();
    for rule in &c.rules {
        let shape = Identity {
            pid: 99,
            created: 1,
            session_id: 1,
            path: rule.app.path.clone(),
            provenance: Some(Provenance {
                owner_sid: rule.owner_sid.clone(),
                logon_id: 1,
                image_file_id: rule.app.image_file_id.clone(),
            }),
        };
        if !policy::complete_identity(&shape)
            || rule.owner_sid.len() > 256
            || rule.app.image_file_id.len() > 256
            || rule.app.path != policy::normalized_path(&rule.app.path)
            || rule.stamp.image_file_id != rule.app.image_file_id
            || rule.stamp.file_size == 0
            || rule.stamp.modified == 0
            || rule.granted_unix == 0
            || rule.expires_unix <= rule.granted_unix
            || rule.expires_unix - rule.granted_unix > APPROVAL_SECONDS
            || !paths.insert(&rule.app.path)
            || policy::reserved_name(rule.app.path.rsplit('\\').next().unwrap_or(""))
        {
            return Err(
                "Invalid, duplicate or protected recurring permission. Nothing was changed.".into(),
            );
        }
    }
    Ok(())
}

pub fn approve(
    id: &Identity,
    stamp: ReopenApproval,
    action: RuleAction,
    now: u64,
) -> AppResult<Rule> {
    let rule = Rule {
        app: applications::app_key(id)?,
        stamp,
        owner_sid: id
            .provenance
            .as_ref()
            .ok_or("Unknown owner")?
            .owner_sid
            .clone(),
        action,
        granted_unix: now,
        expires_unix: now.checked_add(APPROVAL_SECONDS).ok_or("Invalid time")?,
    };
    validate(&Config {
        schema: 1,
        rules: vec![rule.clone()],
    })?;
    Ok(rule)
}

pub fn matching_rule<'a>(
    c: &'a Config,
    id: &Identity,
    stamp: &ReopenApproval,
    now: u64,
) -> Option<&'a Rule> {
    let key = applications::app_key(id).ok()?;
    let owner = &id.provenance.as_ref()?.owner_sid;
    c.rules
        .iter()
        .find(|r| r.active(now) && r.app == key && &r.owner_sid == owner && &r.stamp == stamp)
}

/// Resolve only the finite set observed at this On press. All other apps are kept.
/// Native callers must filter file stamps before calling this function, then recheck before mutation.
pub fn plan(
    c: &Config,
    rows: &[ProcessRow],
    caller: &Identity,
    protected: &[String],
    now: u64,
    experimental_gpu: bool,
) -> AppResult<Plan> {
    validate(c)?;
    if !policy::complete_identity(caller) {
        return Err("Current user/session cannot be established.".into());
    }
    if !c.rules.iter().any(|r| r.active(now)) {
        return Err(
            "Choose background apps once in Settings. No games or game files need to be selected."
                .into(),
        );
    }
    let mut actions = Vec::new();
    for rule in c.rules.iter().filter(|r| r.active(now)) {
        let group: Vec<_> = rows
            .iter()
            .filter(|r| {
                applications::app_key(&r.identity).as_ref() == Ok(&rule.app)
                    && applications::same_user_session(&r.identity, caller)
                    && r.identity
                        .provenance
                        .as_ref()
                        .is_some_and(|p| p.owner_sid == rule.owner_sid)
            })
            .collect();
        if group.iter().any(|r| {
            r.protected_reason.is_some()
                || r.identity.pid == caller.pid
                || rule.app.path == policy::normalized_path(&caller.path)
                || protected
                    .iter()
                    .any(|p| policy::normalized_path(p) == rule.app.path)
        }) {
            continue;
        }
        for row in group {
            actions.push(ApprovedAction {
                target: row.identity.clone(),
                reopen: None,
                action: match rule.action {
                    RuleAction::Close => ActionKind::Close,
                    RuleAction::ReduceLoad => ActionKind::LowerPriorities,
                },
            });
        }
        if actions.len() > 32 {
            return Err("Approved apps expand beyond 32 processes. Reduce the selection in Settings; no partial cleanup was started.".into());
        }
    }
    actions.sort_by_key(|a| a.target.pid);
    let lower = actions
        .iter()
        .any(|a| a.action == ActionKind::LowerPriorities);
    let plan = Plan {
        manual_mode: true,
        game: None,
        game_path: String::new(),
        actions,
        protected_paths: protected.to_vec(),
        options: Options {
            cpu_priority: lower,
            eco_qos: lower,
            memory_priority: lower,
            gpu_priority: lower && experimental_gpu,
            ..Options::default()
        },
        consent: true,
        force_consent: false,
        experimental_consent: lower && experimental_gpu,
    };
    policy::validate(&plan)?;
    Ok(plan)
}
