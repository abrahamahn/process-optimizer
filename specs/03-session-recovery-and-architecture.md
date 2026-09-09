# 03 - Session recovery and architecture

Status: initial specification, 2026-09-09. Architecture and data shapes below are proposed contracts; no binaries exist yet.

## Snapshot means a scoped change record

A snapshot captures original settings and relevant application state for our approved actions. It is not System Restore, a filesystem snapshot, a VM checkpoint or a capture of process/GPU memory.

| Recovery category | What recovery means |
| --- | --- |
| Reversible setting | Restore the exact observed pre-session value when the same target exists and the value has not visibly diverged |
| Cooperative pause | Resume only a workload we paused, if it is still in the expected paused state |
| Application closure | Cannot undo termination; optionally launch a new application instance with separate consent |
| Gone or replaced target | Do not apply an old process's settings to a new PID lifetime |
| Ambiguous/conflicting state | Preserve evidence and request review; do not guess or overwrite |

Restoring a memory-priority setting does not restore previous RAM/VRAM residency. Restarting an app does not restore its unsaved work. The UI must use these exact distinctions rather than a single misleading 'everything restored' label.

## Components

Use Rust and native Win32 UI. Microsoft's Rust for Windows bindings expose Windows APIs including native window creation. This supports the selected direction; dependency versions and build tooling are chosen and pinned when implementation starts. [A1]

Keep one repository and initially one Cargo package with modules and two entry points, not a collection of services or reusable frameworks:

| Component | Ownership |
| --- | --- |
| Native UI/tray | Preview, consent, protected-app settings, Start/Restore and readable reports |
| Session controller | Single writer; game lifetime, policy plan, action dispatch, journal, recovery and bounded collection |
| Policy engine | Pure decisions from observations, capabilities, consent and protections; no hidden OS mutations |
| Windows adapters | Narrow FFI for process discovery, GPU telemetry, close requests and individually supported policies |
| Recovery store | Durable local session/action records and app-specific restart intents |

Proposed future source layout, not files to create before they are needed:

```text
src/
  bin/ui.rs
  bin/session_agent.rs
  policy/
  session/
  windows/
  journal/
  telemetry/
tests/
  policy/
  recovery/
  windows_integration/
```

The initial controller runs in the user's session and modifies only eligible same-user targets using available rights. Do not require a system service for the first user-mode slice. Later privileged operations may use a narrowly scoped broker with independently authorized actions; do not run the complete UI elevated by default.

## Identity and authorization

`ProcessIdentity` includes PID, process creation time, owning user SID, Windows session, executable path and validated executable/file identity. Hold a process handle across observation and action where practical. `GetProcessTimes` provides creation time; PID alone is never a durable identity. [A2]

An application group contains an explicitly resolved set of such identities and evidence for membership. Re-check protection, owner, lifetime and action permission at mutation time. A signed binary is not automatically disposable. Unknown, critical, protected, system, other-user or ambiguous targets are excluded from automated mutation.

Automatic policy rules refer to an application identity and permitted operations, not shell patterns. Identity drift, process replacement, policy revision or elevation changes require re-evaluation. Never replay privileged operations from untrusted raw log text.

If a broker is introduced, use local-only authenticated IPC with a restrictive Named Pipe DACL, verified client user/session, a small versioned command schema, bounded message size and replay protection. Windows provides access control for Named Pipes; do not accept its defaults without review. [A3]

The broker re-derives target identity/protection and authorizes each typed operation. It does not accept arbitrary shell commands, registry paths, DLL paths or executable arguments to run as administrator. No remote listener, debug-privilege fallback, protected-process bypass or kernel driver is part of the initial product.

## Minimal data contracts

These are logical records, not a finalized wire format:

```text
Session:
  session_id, schema_version, policy_version, user_sid, windows_session_id
  boot_fingerprint, host_capability_fingerprint, selected_game
  state, controller_epoch, created_at, finished_at, recovery_summary

Consent:
  consent_id, session_or_saved_rule_scope, application_identity
  allowed_actions, target_set_revision, restart_permission, granted_at, revoked_at

Action:
  action_id, session_id, sequence, capability_id, target_identity
  action_kind, recovery_category, consent_id, dependencies
  before_value, requested_value, observed_after_value
  state, error_or_conflict, timestamps, optional_restart_intent

Observation:
  source, interval, target_identity, adapter_identity, engine_identity
  value, units, quality_status, reason_if_unavailable
```

A boot fingerprint and host capability fingerprint must come from a tested provider; they are not invented Windows API names. Do not use wall-clock timestamps alone to resolve ownership or process identity. Keep monotonic ordering for deadlines and a sequence number for action order.

Use a small local SQLite store as the initial durability proposal, with a single writer, transactions and explicitly tested synchronous durability settings. SQLite protects its own records; it does not make external Windows API operations atomic. A snapshot should contain only necessary settings, not copied user files or a full system registry.

## State machine

```text
Idle -> Inspecting -> AwaitingConsent -> Prepared -> Applying
     -> AwaitingGame -> Active -> Restoring -> Completed
```

An attached-running-game flow may move from Applying directly to Active after verification. Cancellation, launch failure or partial application transitions to Restoring when mutations may have happened. An unresolved or corrupt state is RecoveryRequired, not Completed.

Only one active mutation session is allowed per user in the initial product. Repeated Start/Restore requests are idempotent. A second game is protected and reported; do not kill it or silently reinterpret the existing plan. A later multi-game feature requires explicit ownership/ref-count semantics.

Track the actual game process and verified successor lifetimes. Launcher exit is not proof of game exit. Temporary focus loss, lock screen, Alt-Tab and sleep are not termination. On resume, re-check identities, adapter availability and capability state before any new action. An uncertain game lifetime pauses new mutations and requires review.

## Write-ahead apply protocol

For each action, in dependency order:

1. Verify target identity, protection, current consent, capability and original state. An unreadable original value makes a reversible mutation ineligible.
2. Persist the action intent and original value durably before attempting the external call. Abort before mutation if the store is unavailable or full.
3. Re-check target and relevant preconditions, perform one bounded mutation and read back or otherwise verify the observable result.
4. Persist the outcome as applied, skipped, failed or indeterminate. The API returning success is not sufficient evidence of application exit or performance gain.

Do not hold a database transaction open while waiting for a save dialog or a long OS operation. Persist intent first and record completion in a subsequent transaction.

Action states distinguish `planned`, `intent_persisted`, `applied_verified`, `failed`, `indeterminate`, `restoring`, `restored`, `target_gone`, `conflict`, `reopen_offered`, `reopened` and `manual_review`.

Before any unhandled failure stops the controller, preserve the best available recovery state. If persistence fails after a mutation, the already-durable intent is the recovery anchor. Do not erase it or mark the session clean.

## Restoration and interruption handling

Disable new optimization actions before restoration. Process reversible actions in reverse dependency/order where valid, without blocking unrelated restoration on one failed action.

For a setting action, revalidate identity and read current state:

- Current equals original: no write is needed; record already restored.
- Current equals our verified applied value and target is still the same: attempt restoration, then verify.
- Current differs from both: mark conflict; do not overwrite a user's or another tool's observed change.
- Target exited or identity changed: mark target gone; do not transfer the old setting to a replacement process.
- Readback is denied or unavailable: retain an unresolved record and expose a retry/manual path.

A pending intent without a completion record requires reconciliation. If an irreversible close may already have happened, never reissue it blindly. Configuration actions can use state comparison; ambiguous ones require review.

These comparisons are best-effort ownership checks, not atomic compare-and-swap across all Windows settings. Another tool can change a value between reads or change it away and back; tests and documentation must acknowledge this limitation. Never continuously force a competing setting back into our preferred value.

### Controller and machine failure

Closing/crashing the UI must leave the independent session controller able to finish the session. Controller startup must inspect unfinished journals before accepting new mutations.

If the controller itself dies and no tested supervisor exists, immediate recovery is not guaranteed. The initial recovery guarantee is reconciliation on the next controller start. A production supervised/broker design must separately prove recovery after controller failure before advertising unattended recovery.

A reboot does not resurrect terminated applications or old processes. Discard process-scoped restore writes for previous-boot lifetimes; revalidate any app-specific persistent pause state before resuming it. The initial slice avoids persistent system configuration changes, reducing the scope of post-reboot recovery.

A corrupt/unsupported journal must be preserved for inspection. Disable new mutation sessions and never guess original values. A reviewed user acknowledgement can resolve an irrecoverable record without falsely claiming restoration.

## Optional application reopening

Only reopen an app that was observed running, was closed by this session, is still absent, and has explicit restart consent. Do not start an application that was already closed before the session.

Use a validated application launch identity or reviewed adapter. Launch with the user's normal privileges, not inherited broker elevation. Do not replay arbitrary captured command lines, environment variables, one-shot commands or shell scripts. Exclude database migrations, build jobs and other non-idempotent workloads unless an app-specific reviewed restart contract exists.

Wait for an interactive unlocked user session before automatic reopening. If the user already reopened the application, skip duplicate launch. Cancellation after app closure can restore settings and offer reopening, but cannot undo the closure itself.

## Local privacy and maintenance

Restrict recovery/config files to their owner and the specifically authorized broker identity. No telemetry upload, browser URLs, document contents, screenshots, window-title collection or command-line collection by default. Redact local paths and user identifiers in exported reports; never commit actual inventories or raw traces to this public repository.

Installer update/uninstall, when implemented, must first detect active sessions, restore eligible changes or expose unresolved recovery, and preserve required recovery data until acknowledged. Do not replace the controller mid-mutation.

## Primary evidence

- [A1 - Rust for Windows](https://learn.microsoft.com/en-us/windows/dev-environment/rust/rust-for-windows)
- [A2 - GetProcessTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes)
- [A3 - Named Pipe security and access rights](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights)

Other OS/API contracts are owned by [GPU and process policy](02-gpu-and-process-policy.md). Journal/state-machine choices in this document are our design, not guarantees supplied by those APIs.
