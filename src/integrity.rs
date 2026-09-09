//! Structural validation of local recovery records, not a tamper-proof log.
//! Native identity/ownership checks still run immediately before each OS operation.
use crate::{model::*, policy};
use std::collections::HashSet;

pub const MAX_RECORD_BYTES: usize = 4 * 1024 * 1024;

pub fn encode(session: &Session) -> AppResult<String> {
    validate_record(session)?;
    let body = serde_json::to_string(session).map_err(|e| e.to_string())?;
    if body.len() > MAX_RECORD_BYTES {
        return Err(
            "Recovery record exceeds its size limit; no further mutation is allowed.".into(),
        );
    }
    Ok(body)
}

pub fn uncertain_close(state: &CloseState) -> bool {
    matches!(
        state,
        CloseState::IntentRecorded | CloseState::RequestedPending
    )
}

pub fn validate_record(s: &Session) -> AppResult<()> {
    if (s.schema != SCHEMA_VERSION && s.schema != 2 && s.schema != 3)
        || (s.schema < 4 && s.plan.manual_mode)
        || (s.schema == 2
            && (s.plan.actions.iter().any(|a| a.reopen.is_some()) || !s.reopened.is_empty()))
    {
        return Err("Unsupported recovery schema; preserve it for the matching build.".into());
    }
    if s.id.is_empty()
        || s.id.len() > 128
        || !s
            .id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("Invalid recovery record ID.".into());
    }
    policy::validate(&s.plan)?;
    if s.changes.len() > 128
        || s.closed.len() > 32
        || s.events.len() > 2048
        || s.reopened.len() > 32
    {
        return Err("Recovery record exceeds its bounded action/event count.".into());
    }
    if s.stage == Stage::Pending
        && (!s.changes.is_empty() || !s.closed.is_empty() || !s.reopened.is_empty())
    {
        return Err("A pending session cannot contain previously attempted mutations.".into());
    }
    if let Some(game) = &s.game {
        if !s
            .plan
            .game
            .as_ref()
            .is_some_and(|g| policy::same_process(g, game))
        {
            return Err("Runtime game differs from the approved game lifetime.".into());
        }
    }
    let mut changed = HashSet::new();
    for change in &s.changes {
        let Some(action) = s
            .plan
            .actions
            .iter()
            .find(|a| policy::same_process(&a.target, &change.target))
        else {
            return Err("A recovery target was not part of the approved plan.".into());
        };
        if action.action != ActionKind::LowerPriorities
            || !change.before.valid()
            || !change.applied.valid()
            || change.before == change.applied
            || change.before.background() != change.applied
        {
            return Err("Invalid original/desired policy values in the recovery record.".into());
        }
        let property = change.before.property();
        let enabled = match property {
            Property::GpuPriority => s.plan.options.gpu_priority,
            Property::CpuPriority => s.plan.options.cpu_priority,
            Property::EcoQos => s.plan.options.eco_qos,
            Property::MemoryPriority => s.plan.options.memory_priority,
        };
        if !enabled || !changed.insert((change.target.pid, property as u8)) {
            return Err("Duplicate or unapproved recovery property.".into());
        }
    }
    let mut closed = HashSet::new();
    for close in &s.closed {
        let Some(action) = s
            .plan
            .actions
            .iter()
            .find(|a| policy::same_process(&a.target, &close.target))
        else {
            return Err("Close record target is outside the original approval.".into());
        };
        if !matches!(action.action, ActionKind::Close | ActionKind::ForceClose)
            || (action.action == ActionKind::ForceClose) != close.force_allowed
            || !closed.insert(close.target.pid)
            || (close.state == CloseState::Terminated && !close.force_allowed)
        {
            return Err("Invalid or duplicate close record.".into());
        }
    }
    let mut reopened = HashSet::new();
    for item in &s.reopened {
        if !s
            .plan
            .actions
            .iter()
            .any(|a| a.reopen.is_some() && policy::same_process(&a.target, &item.target))
            || !reopened.insert(policy::normalized_path(&item.target.path))
        {
            return Err("Reopen record is duplicated or outside the original approval.".into());
        }
        if matches!(
            item.state,
            ReopenState::IntentRecorded | ReopenState::Started | ReopenState::Indeterminate
        ) && !s.closed.iter().any(|c| {
            policy::same_process(&c.target, &item.target) && c.state == CloseState::ClosedGracefully
        }) {
            return Err("Reopen intent requires this session's verified graceful closure.".into());
        }
        if (item.state == ReopenState::Started) != item.child.is_some() {
            return Err("A started reopen requires a verified new process identity.".into());
        }
        if let Some(child) = &item.child {
            if !policy::complete_identity(child)
                || policy::same_process(child, &item.target)
                || crate::applications::app_key(child)?
                    != crate::applications::app_key(&item.target)?
                || !crate::applications::same_user_session(child, &item.target)
            {
                return Err("Invalid replacement identity in reopen record.".into());
            }
        }
    }
    if s.stage == Stage::Restored
        && s.plan.actions.iter().any(|a| {
            a.reopen.is_some()
                && s.closed.iter().any(|c| {
                    policy::same_process(&c.target, &a.target)
                        && c.state == CloseState::ClosedGracefully
                })
                && !s
                    .reopened
                    .iter()
                    .any(|r| policy::same_process(&r.target, &a.target))
        })
    {
        return Err("A completed session is missing an approved reopening outcome.".into());
    }
    if s.stage == Stage::Restored
        && (!s.changes.iter().all(|c| c.state.resolved())
            || s.closed.iter().any(|c| uncertain_close(&c.state))
            || s.reopened.iter().any(|r| !r.state.resolved()))
    {
        return Err("Unresolved actions cannot be labeled as fully restored.".into());
    }
    Ok(())
}

/// Apply plans and original-value snapshots are immutable once recorded.
/// This catches accidental rewrites; an authorized user can still modify their own files.
pub fn validate_update(previous: &Session, next: &Session) -> AppResult<()> {
    validate_record(previous)?;
    validate_record(next)?;
    if previous.id != next.id
        || previous.schema != next.schema
        || serde_json::to_value(&previous.plan).map_err(|e| e.to_string())?
            != serde_json::to_value(&next.plan).map_err(|e| e.to_string())?
    {
        return Err("The approved plan cannot be rewritten during an existing session.".into());
    }
    if next.changes.len() < previous.changes.len()
        || next.closed.len() < previous.closed.len()
        || next.reopened.len() < previous.reopened.len()
    {
        return Err("Recovery evidence cannot be removed by a session update.".into());
    }
    for (old, new) in previous.changes.iter().zip(&next.changes) {
        if old.target != new.target || old.before != new.before || old.applied != new.applied {
            return Err("Original process identity or policy values were rewritten.".into());
        }
    }
    for (old, new) in previous.closed.iter().zip(&next.closed) {
        if old.target != new.target || old.force_allowed != new.force_allowed {
            return Err("A recorded close target or force authorization was rewritten.".into());
        }
    }
    for (old, new) in previous.reopened.iter().zip(&next.reopened) {
        if old.target != new.target
            || (old.state.resolved()
                && serde_json::to_value(old).map_err(|e| e.to_string())?
                    != serde_json::to_value(new).map_err(|e| e.to_string())?)
            || (old.state == ReopenState::Indeterminate
                && new.state != ReopenState::Indeterminate
                && new.state != ReopenState::UserKept)
        {
            return Err(
                "Reopen evidence cannot be rewritten or retried after an uncertain launch.".into(),
            );
        }
    }
    if previous.stage.finished()
        && serde_json::to_value(previous).map_err(|e| e.to_string())?
            != serde_json::to_value(next).map_err(|e| e.to_string())?
    {
        return Err("A finished session is read-only; start a new reviewed plan.".into());
    }
    Ok(())
}
