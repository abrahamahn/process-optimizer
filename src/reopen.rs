//! Optional reopening is a new irreversible launch, not settings restoration.
use crate::{engine::Backend, integrity, journal::Journal, model::*, policy};

pub fn restore_apps<B: Backend, J: Journal>(
    s: &mut Session,
    backend: &mut B,
    journal: &mut J,
) -> AppResult<()> {
    integrity::validate_record(s)?;
    for action in s.plan.actions.clone() {
        let Some(approval) = action.reopen else {
            continue;
        };
        if let Some(record) = s
            .reopened
            .iter_mut()
            .find(|r| policy::same_process(&r.target, &action.target))
        {
            if record.state == ReopenState::IntentRecorded {
                record.state = ReopenState::Indeterminate;
                record.detail = "An earlier launch intent survived interruption. No duplicate launch was attempted; review manually.".into();
                journal.save(s)?;
            }
            continue;
        }
        let verified = s.closed.iter().any(|c| {
            policy::same_process(&c.target, &action.target)
                && c.state == CloseState::ClosedGracefully
        });
        let record = ReopenRecord {
            target: action.target.clone(),
            child: None,
            state: if verified {
                ReopenState::IntentRecorded
            } else {
                ReopenState::Skipped
            },
            detail: if verified {
                "Launch intent persisted before checking the desktop and starting a new process."
            } else {
                "No verified graceful closure by this session. The app was not reopened."
            }
            .into(),
        };
        s.reopened.push(record);
        // If this write fails, no launch call occurs.
        journal.save(s)?;
        if !verified {
            continue;
        }
        let idx = s.reopened.len() - 1;
        match backend.reopen(&action.target, &approval) {
            Ok(record)
                if record.target == action.target
                    && record.state != ReopenState::IntentRecorded =>
            {
                s.reopened[idx] = record
            }
            Ok(_) => {
                s.reopened[idx].state = ReopenState::Indeterminate;
                s.reopened[idx].detail =
                    "Invalid launch acknowledgement; no retry is permitted.".into();
            }
            Err(e) => {
                s.reopened[idx].state = ReopenState::Indeterminate;
                s.reopened[idx].detail = format!(
                    "Launch outcome unconfirmed: {}. No retry is permitted.",
                    e.message
                );
            }
        }
        journal.save(s)?;
    }
    Ok(())
}
