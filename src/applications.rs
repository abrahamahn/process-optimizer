//! Conservative executable groups. A group is a finite snapshot, never a kill rule.
use crate::{model::*, policy};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppKey {
    pub path: String,
    pub image_file_id: String,
}

pub fn app_key(id: &Identity) -> AppResult<AppKey> {
    if !policy::complete_identity(id) {
        return Err("Complete executable evidence is required.".into());
    }
    Ok(AppKey {
        path: policy::normalized_path(&id.path),
        image_file_id: id
            .provenance
            .as_ref()
            .ok_or("Missing provenance")?
            .image_file_id
            .clone(),
    })
}

pub fn same_user_session(a: &Identity, b: &Identity) -> bool {
    a.session_id == b.session_id
        && match (&a.provenance, &b.provenance) {
            (Some(a), Some(b)) => a.owner_sid == b.owner_sid && a.logon_id == b.logon_id,
            _ => false,
        }
}

pub fn eligible(
    row: &ProcessRow,
    game: &Identity,
    rows: &[ProcessRow],
    protected: &[String],
) -> bool {
    row.protected_reason.is_none()
        && policy::complete_identity(&row.identity)
        && same_user_session(&row.identity, game)
        && !policy::reserved_name(&row.name)
        && policy::normalized_path(&row.identity.path) != policy::normalized_path(&game.path)
        && !policy::guarded_pids(rows, game).contains(&row.identity.pid)
        && !protected
            .iter()
            .any(|p| policy::normalized_path(p) == policy::normalized_path(&row.identity.path))
}

/// Matching file/owner/logon/session only; unrelated executables and future children stay out.
pub fn group(rows: &[ProcessRow], selected: &Identity) -> AppResult<Vec<Identity>> {
    let key = app_key(selected)?;
    if !rows
        .iter()
        .any(|r| policy::same_process(&r.identity, selected))
    {
        return Err("The selected process is no longer in this snapshot.".into());
    }
    let mut result = rows
        .iter()
        .filter(|r| {
            same_user_session(&r.identity, selected) && app_key(&r.identity).as_ref() == Ok(&key)
        })
        .map(|r| r.identity.clone())
        .collect::<Vec<_>>();
    result.sort_by_key(|r| r.pid);
    result.dedup_by_key(|r| r.pid);
    if result.len() > 32 {
        return Err(
            "This executable group exceeds 32 processes. Review a smaller explicit selection."
                .into(),
        );
    }
    Ok(result)
}
