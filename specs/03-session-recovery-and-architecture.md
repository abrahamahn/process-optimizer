# 03 - Session, persistence and recovery behavior

Behavioral baseline v1, 2026-09-09. This owns identity, durable state and recovery. [Product](01-product.md) owns consent and [policy](02-gpu-and-process-policy.md) owns Windows mechanisms.

## Architecture

One Rust Cargo package; native UI and independent session-agent executables. Keep pure model/policy/reconciliation separate from Windows adapters and SQLite persistence. No always-on privileged service or general plugin framework is necessary for the first user-session implementation. Microsoft's Windows bindings support the selected Win32 direction. [R1]

The controller is the sole mutation writer. UI process exit must not own the lifetime of the game-session controller. Normal operation is unelevated and same-user/session. Privileged service control, if later implemented, requires a separate reviewed broker; it must not inherit the UI's unchecked commands.

An initial local request-file transport is permitted only inside the restricted per-user state directory, with bounded typed JSON, exact schema validation, unique request/session identifiers and no arbitrary shell-command fields. Requests are consumed once. A future Named Pipe broker needs restrictive DACL, local-only transport, client SID/session authentication and independent operation authorization. [R2]

## Logical data model

Unknown fields in external requests are rejected; supported schema version is explicit. Implementation type names may differ, but meaning must not.

| Record | Required content |
| --- | --- |
| ProcessIdentity | PID, creation FILETIME, owner SID, interactive Windows session, logon identity where available, canonical image path and file identity/version evidence |
| Game | Explicitly selected process identity; no implicit foreground or launcher replacement |
| RequestedAction | Exact target identity, operation, nonessential designation, normal/force/experimental consent as appropriate, optional separately reviewed reopen intent |
| Plan | Schema, unique ID, game, bounded immutable action set, explicit review acknowledgement |
| SessionRecord | Schema, ID, game, state, ordered action records, interruption/result details |
| ActionRecord | Target, operation, original/desired/observed values, recovery category, phase, outcome and error/conflict |
| Observation | Source, interval, process lifetime, adapter/engine keys, units, optional value and quality |
| Settings | Schema and user-protected canonical application paths; no persisted force or reopen approval by default |

D-01: PID and filename alone never identify a target. Windows exposes process creation times through `GetProcessTimes`. Where practical retain the process handle from revalidation through mutation. File-path normalization is not a substitute for checking the current file identity. [R3]

D-02: A restarted process receives no restore settings from an old lifetime. A changed logon/session/lifetime is ineligible. Reboot reconciliation never blindly replays old process changes. If identity/provenance cannot be validated, classify unknown and retain evidence rather than writing.

D-03: Use monotonic time for deadlines; wall clock is display/audit data only. Do not infer a reliable boot identifier from a rounded wall-clock estimate. Process/logon identity and live validation are the initial guard; persistent system-setting mutations are excluded from the first slice.

## Durable store and single writer

J-01: SQLite is the initial store. Use schema versioning, transactions, explicit FULL synchronous durability and bounded busy timeouts. The first implementation may use rollback-journal mode to simplify read/write behavior. SQLite makes its records atomic, not the external OS call. [R4]

J-02: Acquire an exclusive controller lock before recovery or application. The lock is scoped to the per-user store and lives as an open OS handle, not merely the presence of a PID file. A stale lock filename after a crash is not proof an agent remains alive. No two controllers may mutate from the same store concurrently.

J-03: Enforce at most one unfinished session with a database constraint as well as controller ownership. Repeated Start of an existing plan ID is rejected/idempotent, never another close attempt. Repeated Restore reconciles the existing record. Unresolved records block new mutation sessions until recovery or explicit acknowledgement.

J-04: Restrict the state directory and files to the user/system as needed. Do not use an elevated process to follow user-controlled journal paths. Reject reparse/link/remote-path ambiguity where inspected. Bounded request size is 256 KiB; maximum 32 action entries. Logs are local and contain only necessary identity/settings, not command lines, window titles or document content.

J-05: Disk full, read-only store, unknown schema, corrupt JSON or corrupt database prevents new mutations. Validate recovery targets and enabled properties against the immutable original approval before native inspection or writes. Reject duplicate actions/properties, invalid native values, rewritten original values, removed recovery evidence and false completed states. Structural validation does not make a same-user local log tamper-proof. Preserve problematic records; never delete them or guess defaults to make the UI look clean. No schema downgrade. Normal retention may prune old completed records, never unresolved records.

## Session state machine

UI-only inspection/review precedes the durable machine below:

```text
Prepared -> Applying -> Active -> Restoring -> Completed
                 \---------> Restoring -> RecoveryRequired
Prepared ------------------> Restoring
RecoveryRequired ----------> Restoring
RecoveryRequired ----------> Acknowledged (explicit user review only)
```

S-01: Attach mode requires a verified running game before accepting the plan and before each new action. If the game is already gone, do not apply anything. The first version does not guess launcher successors. Real game termination or explicit Restore stops new optimization and initiates restoration.

S-02: Cancel during application is checked between bounded actions. A delivered close request cannot be retracted; report that limitation. No additional targets are introduced after review. New processes are not automatically acted on.

S-03: A second Start is rejected while another session is unfinished. A second game is never automatically killed or optimized; the original selected game remains the lifetime authority. Future multi-game support needs explicit shared ownership, not accidental overlapping sessions.

S-04: Focus change, minimize, screen lock and sleep do not equal exit. Before resumed actions or recovery, revalidate live identities and readable state. If observation cannot establish safety, stop new mutations and retain a recoverable/unknown state.

S-05: Closing/crashing the UI does not kill the controller. If the controller dies, immediate recovery is NOT guaranteed in the unsupervised first implementation. Next controller startup reconciles unfinished records before accepting anything new. A supervisor/service claim requires separate crash tests.

## Action phases and write-ahead application

Logical action phases are Planned, Intent, Applied, NoChange, NotApplied, NotRequested, Closed, ClosePending, Failed, Indeterminate, RestoreIntent, Restored, Gone, Conflict and ManualReview. Readable messages explain the exact outcome. Close/force and settings have different recovery categories.

W-01: Preflight the entire finite plan: game identity, target identities/protection, consent, valid operations, duplicate/conflicting actions and size bounds. A malformed or unauthorized plan is rejected before recording external mutation intent.

W-02: Persist the session and planned actions before applying. Immediately before each setting mutation, read original state and compute a non-escalating desired value. An unavailable getter produces unsupported/failed-without-mutation, not a guessed original.

W-03: Durably persist Intent with original and desired values before the external call. If persistence fails here, no external call is allowed. Do not hold a SQLite transaction while waiting for an app or OS operation.

W-04: Revalidate held target/protection and relevant state, perform one bounded operation, then verify observable state. Persist completion separately. A setter error may still leave an uncertain state; use the already durable intent as the recovery anchor.

W-05: If a setter, its verification or its completion write fails, stop further application and preserve/reconcile from the last durable record. A close call returning an error may have delivered a request: retain an uncertain close intent, stop new optimization and never replay the close. Never erase the prior intent. Failure isolation during recovery must not falsely mark still-pending work completed.

W-06: NoChange requires no restoration write. When cancellation, revoked authorization or changed original state is detected after persisting intent but before our external call, durably record NotApplied (setting) or NotRequested (close). Those records never confer restoration ownership, even if another tool later independently chooses our intended value. If recording this fact itself fails or the controller crashes first, retain the preceding uncertain intent; do not invent certainty. Lowering policies must not raise an already lower value. A Close request is recorded before sending; only signaled process exit permits Closed. Otherwise ClosePending/Failed/Indeterminate accurately describes the evidence.

## Restoration table

Stop optimization first. Restore actions in reverse dependency/order, continuing independent eligible restorations when one fails. Every restoration write has its own durable intent and readback.

| Observation for a reversible action | Required outcome |
| --- | --- |
| Durable NotApplied or NotRequested | No native recovery write or close request; we did not perform that action |
| Same lifetime; current equals original | Already original/restored; no write |
| Same lifetime; current equals our verified applied value, with no earlier external conflict | Save restore intent, recheck current value, then restore exact original; verify and record |
| An external ownership conflict was previously observed | Never regain write permission merely because current value again matches our old value; retain conflict unless already original or target gone |
| Only apply intent survived; current equals recorded desired value | Reconcile conservatively from durable intent, restore original if all identity/value checks hold; record inferred outcome |
| Current differs from original and our applied/desired value | Conflict; preserve external change |
| PID disappeared or another lifetime now occupies it | Gone; no write to replacement |
| Getter/identity inspection fails | Unresolved; retain retry/manual-review path |
| Setter succeeds but restoration readback differs/fails | Unresolved, not restored |
| Action is close/force | Never repeat the operation; optional reopening is separate |

R-01: Comparisons are best-effort, not a Windows-wide atomic compare-and-swap. Another actor can race or change a value away and back. Do not advertise perfect ownership detection and do not continuously fight another optimizer.

R-02: Pending irreversible intents must not be blindly replayed. An app absent after an interrupted close is not enough evidence that our action caused it; ambiguous cases only offer manual reopening/review.

R-03: Restore original representable state, including nondefault priorities and EcoQoS control/state masks. Do not restore an invented Normal/default. Memory-priority restoration does not restore page residency; closing/reopening does not restore process/GPU memory.

R-04: Automatic reopening is allowed only after verified closure by this session, still-valid separate consent, unchanged executable identity, no existing replacement and an active unlocked interactive desktop. Launch normally, without captured arguments or elevation. Otherwise record deferred/skipped; no background startup on a locked desktop.

R-05: Reopen intent must be persisted before launching. If the controller dies across a launch, startup does not blindly launch again; detect existing instance where possible or leave manual review. Reopening an app never transfers old per-process settings to it.

R-06: Manual acknowledgement requires displaying unresolved actions and an explicit warning that acknowledgement is not restoration. Keep the original record as Acknowledged. It only releases the new-session gate; it must never execute an unreviewed repair or delete evidence.

## Application-workflow records

Session schema 3 adds optional per-action executable reopening approval (file ID/size/last-write time) and separate launch records. Schema 2 can be read and recovered only without these capabilities; new sessions use schema 3. No in-place upgrade fabricates consent. Finished records remain read-only. The SQLite storage schema remains version 1 with an additive profiles table; old binaries cannot parse schema-3 active sessions and must not be used to recover them.

A profile uses its own typed schema and stores executable recipes, not process lifetimes or consent. Profile writes have size/count validation and a transaction protecting the 32-profile limit. Replacing/deleting recipes does not alter sessions or protection. Loading yields only an unapproved review.

Launch outcomes are IntentRecorded, Started (verified new lifetime, not GUI-readiness proof), AlreadyRunning, Deferred, Skipped, Indeterminate and explicitly UserKept. Recovery persists launch intent before calling the native adapter. On interruption an existing launch intent becomes Indeterminate and is never replayed. Deferred is a known no-launch outcome that requires manual reopening, not an automatic retry. Indeterminate remains unresolved until explicit acknowledgement. Reopening failure cannot erase settings-recovery conflicts. Reopening authorization/records are checked against the immutable close action and its verified closure.

## Security, privacy and maintenance

No arbitrary privileged shell execution or raw command-line replay. The agent revalidates typed inputs even when the UI generated them. Same-user code is not treated as a security boundary stronger than the user's existing authority; the initial product refuses elevation and remote operation.

No diagnostic uploads by default. Exports require explicit action and redaction of paths/SIDs. No real machine inventory or trace is committed to this public repository. Installer updates/uninstall, when delivered, must detect active/pending sessions and preserve unresolved recovery data rather than deleting it.

## Primary evidence

- [R1 Rust for Windows](https://learn.microsoft.com/en-us/windows/dev-environment/rust/rust-for-windows)
- [R2 Named Pipe security](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights)
- [R3 GetProcessTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes)
- [R4 SQLite atomic commit](https://www.sqlite.org/atomiccommit.html) and [synchronous pragma](https://www.sqlite.org/pragma.html#pragma_synchronous)

State-machine choices are project contracts, not additional guarantees supplied by these APIs.
