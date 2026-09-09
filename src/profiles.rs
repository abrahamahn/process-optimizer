//! Saved recipes, not saved permission. Resolving always produces a new, unapproved preview.
use crate::{
    applications::{self, AppKey},
    model::*,
    policy,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const PROFILE_SCHEMA: u32 = 1;
pub const MAX_PROFILES: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecipeAction {
    Close,
    LowerPriorities,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeTarget {
    pub app: AppKey,
    pub action: RecipeAction,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameProfile {
    pub schema: u32,
    pub game: AppKey,
    pub targets: Vec<RecipeTarget>,
    pub options: Options,
}
#[derive(Clone, Debug)]
pub struct Preview {
    pub actions: Vec<ApprovedAction>,
    pub options: Options,
    pub notices: Vec<String>,
}

fn valid_key(key: &AppKey) -> bool {
    let id = Identity {
        pid: 99,
        created: 1,
        path: key.path.clone(),
        session_id: 1,
        provenance: Some(Provenance {
            owner_sid: "shape-check-only".into(),
            logon_id: 1,
            image_file_id: key.image_file_id.clone(),
        }),
    };
    policy::complete_identity(&id)
        && key.image_file_id.len() <= 256
        && key.path == policy::normalized_path(&key.path)
        && !policy::reserved_name(key.path.rsplit('\\').next().unwrap_or(""))
}

pub fn validate(p: &GameProfile) -> AppResult<()> {
    if p.schema != PROFILE_SCHEMA
        || !valid_key(&p.game)
        || p.targets.len() > 32
        || p.options.gpu_priority
        || p.options.launch_game
        || !(1000..=15000).contains(&p.options.close_timeout_ms)
        || serde_json::to_vec(p).map_err(|e| e.to_string())?.len() > 256 * 1024
    {
        return Err(
            "Invalid profile. Profiles cannot retain experimental or automatic-launch permission."
                .into(),
        );
    }
    let mut seen = std::collections::HashSet::new();
    for t in &p.targets {
        if !valid_key(&t.app) || t.app.path == p.game.path || !seen.insert(t.app.path.clone()) {
            return Err("Invalid, duplicate or game-targeting profile entry.".into());
        }
        if t.action == RecipeAction::LowerPriorities
            && !(p.options.cpu_priority || p.options.eco_qos || p.options.memory_priority)
        {
            return Err("A saved lowering recipe needs a nonexperimental policy.".into());
        }
    }
    Ok(())
}

pub fn capture(
    game: &Identity,
    actions: &[ApprovedAction],
    options: &Options,
) -> AppResult<GameProfile> {
    let mut targets = BTreeMap::new();
    for action in actions {
        let recipe = match action.action {
            ActionKind::ForceClose => return Err(
                "Remove force actions before saving a profile; force permission is never saved."
                    .into(),
            ),
            ActionKind::Close => RecipeAction::Close,
            ActionKind::LowerPriorities => RecipeAction::LowerPriorities,
        };
        let app = applications::app_key(&action.target)?;
        if let Some(previous) = targets.insert(
            app.path.clone(),
            RecipeTarget {
                app: app.clone(),
                action: recipe.clone(),
            },
        ) {
            if previous.action != recipe || previous.app != app {
                return Err("A profile cannot combine different file identities or actions at one executable path.".into());
            }
        }
    }
    let profile = GameProfile {
        schema: PROFILE_SCHEMA,
        game: applications::app_key(game)?,
        targets: targets.into_values().collect(),
        options: Options {
            gpu_priority: false,
            launch_game: false,
            ..options.clone()
        },
    };
    validate(&profile)?;
    Ok(profile)
}

pub fn resolve(
    profile: &GameProfile,
    game: &Identity,
    rows: &[ProcessRow],
    protected: &[String],
) -> AppResult<Preview> {
    validate(profile)?;
    if applications::app_key(game)? != profile.game
        || !rows.iter().any(|r| policy::same_process(&r.identity, game))
    {
        return Err("The running game no longer matches this profile's executable. Review and save a new profile after an update.".into());
    }
    let mut actions = Vec::new();
    let mut notices = Vec::new();
    for recipe in &profile.targets {
        let matched = rows
            .iter()
            .filter(|r| {
                applications::app_key(&r.identity).as_ref() == Ok(&recipe.app)
                    && applications::same_user_session(&r.identity, game)
            })
            .collect::<Vec<_>>();
        if matched.is_empty() {
            notices.push(format!(
                "Not included (not running, updated or unverifiable): {}",
                recipe.app.path
            ));
            continue;
        }
        if matched
            .iter()
            .any(|r| !applications::eligible(r, game, rows, protected))
        {
            notices.push(format!(
                "Not included (protected group): {}",
                recipe.app.path
            ));
            continue;
        }
        for row in matched {
            actions.push(ApprovedAction {
                target: row.identity.clone(),
                reopen: None,
                action: match recipe.action {
                    RecipeAction::Close => ActionKind::Close,
                    RecipeAction::LowerPriorities => ActionKind::LowerPriorities,
                },
            });
        }
        if actions.len() > 32 {
            return Err(
                "Profile expands to more than 32 current processes. No partial plan was loaded."
                    .into(),
            );
        }
    }
    actions.sort_by_key(|a| a.target.pid);
    notices.push("Preview only. Review every current process before Start. Force, reopening and experimental approvals were not restored.".into());
    Ok(Preview {
        actions,
        options: profile.options.clone(),
        notices,
    })
}
