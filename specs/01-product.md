# 01 - Product and user-visible behavior

Behavioral baseline v1, 2026-09-09. MUST/MUST NOT are requirements, not statements that an implementation or performance result exists. Implementation evidence is maintained in [validation](04-validation-and-delivery.md). This document owns user-visible behavior; [policy](02-gpu-and-process-policy.md) owns resource actions and [recovery](03-session-recovery-and-architecture.md) owns durable state.

## Purpose and priorities

Temporarily reduce unnecessary non-game work on the GPU used by a game, preserve required functionality, and restore settings changed by this application. GPU processing contention and video-memory pressure are separate concerns. A smaller process count or a lower counter does not prove better frame times.

P-01: GPU visibility and user-approved application closure are required in the first usable version. A CPU-priority-only utility is not this product.

P-02: Native Windows application, Rust core and Win32 UI. No Electron, WebView, account, cloud backend, embedded LLM or compulsory overlay. First build/validation target is Windows x64; Windows 11 gaming compatibility must be established separately from hosted Windows CI.

P-03: Overclocking, undervolting, fan/firmware control, permanent debloating, security disabling, kernel patching, driver reset, game injection, anti-cheat bypass and a stripped boot shell are excluded. Do not change game quality, resolution, frame cap, HDR, VRR or frame-generation settings.

P-04: Protect networking, Bluetooth, input, audio, display, security, accessibility, game authentication/anti-cheat and required thermal/device software. No process-name kill list, service-host termination, suspension of arbitrary threads, shader-cache deletion, pagefile disabling or RAM/VRAM purge.

## Simple On/Off workflow (0.3.0, latest user decision)

The default native window MUST expose only Game Mode status, Turn ON/OFF, a small summary and Settings. No game executable browser, game-process selection, per-game profile or process grid appears on this screen. Settings is a separate compact view, not an always-expanded dashboard. The older explicit game-attached console remains available only under Settings > Advanced tools.

M-01: The default session is manual. A user may turn it on before launching any game, switch games normally, and turn it off when done. Game exit, Alt-Tab, minimizing or closing the settings window MUST NOT end this mode. The independent worker lasts until explicit Off or a recovery-triggering failure. No foreground heuristic is presented as reliable automatic game recognition.

M-02: One-time Settings chooses optional background apps and explicitly approves recurring normal-close or Reduce load actions. Each approval covers the named local executable and its current matching processes at each user-initiated On for no more than 30 days. File ID/size/last-write evidence and user ownership are bound; a detected file change or expiry requires renewed approval. These are metadata checks, not content signatures. Keep removes permission. Nothing is preapproved, and no unknown game/app is classified disposable automatically.

M-03: Normal mode never asks to select a game. First On with no valid permission opens Settings rather than claiming optimization. Later On rebuilds an exact finite plan from valid permissions and current process lifetimes, then starts it without a redundant approval dialog. All unapproved apps are untouched. An empty current match may stay On, but MUST state that nothing was changed.

M-04: Cleanup runs once at each On. New or manually reopened processes are not chased. The mode does not promise GPU exclusivity or a blanket GPU ban. Reduce load bundles supported CPU/EcoQoS/memory lowering; GPU scheduling is opt-in for the next activation only and requires its distinct experimental confirmation. No remembered force or reopening permission exists in this simple path.

M-05: Off stops new actions and restores eligible settings via the existing journal. It does not relaunch closed apps or recover unsaved work; disclose this in the app-approval dialog and Settings. Existing advanced reopening behavior remains separate. Turning Off a legacy attached session uses its existing approved recovery behavior, including already approved reopening.

M-06: A dead worker, unresolved operation or corrupt store must not be displayed as an ordinary successful On/Off transition. Main offers restoration; Settings exposes details and explicit acknowledgement. Recurring rule edits are disabled during active/preparing sessions. System/user protection and current native authorization override saved permission, including foreground-app checks before mutations.

## UI state and first-run defaults

| State | Visible behavior | Allowed operations |
| --- | --- | --- |
| Idle | Game Mode OFF, On button, Settings | Turn On using existing permissions or perform one-time setup |
| Inspecting | Read-only resource collection; unknown values are labeled | Cancel observation; no mutations |
| Reviewing | Exact target lifetimes, actions, risks and recovery categories | Edit/remove actions, approve or cancel |
| Starting | Independent controller validates and records the request | Restore/cancel; no second Start |
| Active | Game Mode ON, Off button and outcome summary | Turn Off; close UI without abandoning worker/recovery |
| Restoring | New optimization stops; per-action outcomes displayed | Idempotent Restore retry |
| RecoveryRequired | Unfinished/conflicting changes and their reasons | Retry recovery or explicitly acknowledge unresolved records |
| Completed | Restored settings, exited processes, reopen outcomes and unresolved limits are distinguished | New inspection/session |

U-01: First launch is read-only. No selected termination targets, no default recurring approval, no automatic activation, no force-close fallback, no reopen permission and no experimental policy enabled. M-02 is the only recurring permission route in the simple UI.

U-02: The advanced game-attached console attaches to an explicitly selected already-running game. The default manual flow instead follows M-01 through M-06. The selection identifies its actual process lifetime, not merely a launcher or foreground window. The UI MUST reject known launchers as the game target. Automatic launcher tracking and saved auto-activation are unavailable until their adapters and lifetime handoff tests exist; unavailable functionality is shown honestly, not simulated.

U-03: Select the game's observed adapter when unambiguous. If no adapter or multiple adapters can be identified, show unknown/ambiguous and allow explicit inspection; never assume GPU 0. A manually approved close can still run without GPU evidence, but MUST NOT be presented as a measured GPU recommendation.

U-04: Inventory presents application/executable, PID, protection/eligibility, engine/adapter evidence and dedicated/shared memory estimates separately. No browser URLs, document contents, window titles or command lines are collected for display. Sort by a named per-engine observation, not a fictitious sum across engines.

U-05: The user marks individual verified process lifetimes or an explicitly enumerated group as nonessential. A group is a finite set, not recursive authorization over future children. Generic closure reports selected-process outcomes and possible remaining helpers; only a tested app adapter may claim whole-app closure.

U-06: A plan has at most 32 action entries and one action of each kind per target. Closure/force closure conflicts with other actions for that target. The preview is invalidated if a selected identity, game, protection rule or action changes. Refreshing counters does not silently replace an approved target with a new PID lifetime.

U-07: Closing/minimizing the native window stops its rendering/refresh work; the independent controller retains the session. Reopening the UI reads controller/journal state. The UI MUST NOT interpret its own exit, Alt-Tab, game minimization or screen lock as game exit. No always-on dashboard animation is required.

U-08: Settings consist of protected apps and visible capability/experimental-policy controls. Protection is conservative across app updates at the same canonical path. A protection removal does not itself authorize an action. Invalid settings fail closed; do not silently reset them to a less protective empty list.

U-09: Native controls must be keyboard reachable, have textual labels, follow system font/contrast conventions, resize without hiding Restore and support high-DPI text. Accessibility and visual review are a release gate, not implied by successful compilation.

## Explicit application workflows

U-10: Same-app selection groups only matching observed canonical executable path/file ID, owner SID, logon and Windows session. It selects a finite maximum of 32 rows without granting permission. Different executable helpers, unobserved members and future children are excluded. Whole-app closure is not inferred from these groups.

U-11: A per-game profile is a local recipe, not authorization. Save/load/delete are explicit UI actions. The profile is keyed by canonical game path and binds executable file ID. A changed game file ID requires renewed review/save; missing, replaced or protected target groups are reported instead of silently selected. A file ID is not a content hash: same-file in-place updates may retain the recipe match, never the prior consent. Loading re-enumerates and constructs fresh exact identities, never old PID approval. At most 32 recipes and 32 targets per recipe; expansion above 32 processes fails without a truncated plan. Force actions cannot be saved; reopening and experimental GPU permissions are discarded. Profile capture retains no process command lines, document data or session tokens. Every loaded plan still requires Start review.

U-12: Reopen after close is an independent per-main-GUI-process opt-in, removable before Start and absent from saved profiles. Only verified graceful closure by this session qualifies. The ordinary local GUI executable must match the recorded file ID, size and last-write time, with a held read-only/no-write-or-delete-share file handle through launch. No shell or captured arguments. Existing or unverifiable replacement instances prevent duplicate startup. An unavailable/locked/disconnected interactive desktop yields a terminal Deferred outcome requiring manual reopening, not an unattended later retry. An uncertain launch blocks automatic retries and remains visible for review. API checks are conservative snapshots, not atomic guarantees against all desktop/filesystem races.

## Consent

C-01: Advanced actions require session-scoped review of exact target identity, operation, reason, save-state uncertainty and recovery category. Simple manual actions may use the separate explicit recurring grant in M-02; the worker still validates the exact operation, live identity and unchanged permission before a mutation. Explicit confirmation that optional targets are nonessential is necessary; high resource usage is never permission. Hard protection still overrides consent.

| Operation | Approval | Recovery disclosure |
| --- | --- | --- |
| Graceful close | Approve the listed lifetimes | May display a save prompt or refuse; shutdown cannot be undone |
| Force terminate | A separate explicit confirmation listing the current exact force targets; never remembered as a default | Unsaved work can be lost; normal shutdown handlers may not run |
| GPU/CPU/memory policy | Approve each policy; experimental policy requires an additional opt-in | Restore the original readable value if the same target and ownership checks remain valid |
| Cooperative pause | Approve a tested adapter's specific workload | Resume only work paused by this session |
| Reopen app | Separate opt-in for each eligible application launch identity | New process, not restored RAM/VRAM/documents or old execution state |

C-02: Start cannot imply consent to unlisted force termination. Normal-close timeout, missing windows, refusal, access denial or save prompts MUST NOT escalate to force. A separately chosen Force action is a new reviewed action, not a hidden fallback.

C-03: Unknown save state stays unknown. Never dismiss a save dialog, confirm a destructive application prompt or synthesize an answer on the user's behalf.

C-04: Cancellation before the durable session record causes no mutations. Cancellation after partial application stops further actions and restores eligible settings; apps already closed stay closed unless separate reopen permission applies.

C-05: Reopening is never enabled by default. Generic reopening is limited to a verified original executable with no replayed command line, normal user privileges, unchanged file identity and no existing replacement instance. Headless jobs, migrations and shell commands are not generic reopen candidates.

C-06: Recurring simple-mode rules are only the M-02 opt-in: user-initiated On, no more than 30 days, exact executable evidence, revocable with Keep, no future-process enforcement. Existing per-game profiles remain recipes, not recurring permission. No filename-only match, remembered force or remembered experimental approval is allowed.

## Protected and optional applications

Hard protection includes the game, verified game infrastructure, the optimizer/controller, critical/protected/system or other-user processes, session-0 services, Windows graphics/session components and paths, known security/launcher/anti-cheat/voice/input/thermal infrastructure, and user-protected paths. Failed identity or protection inspection is not eligibility.

Generic role discovery cannot prove every arbitrary third-party dependency. Require a reviewed nonessential designation, retain conservative hard protections, do not infer disposable status from a digital signature, and state this limitation. Existing accessibility/voice/capture/device dependencies must be reviewed on the actual gaming host before use.

Optional examples, not defaults: animated wallpapers, inactive browsers, unused GUI development tools, optional capture/overlay applications and explicit local GPU workloads. Active voice, capture, remote play or accessibility changes the role. Steam helpers are protected by default, even when they consume GPU resources.

A protection added during a session prevents future optimization actions. It does not prevent attempting to undo that session's own prior reversible changes. No repeated fight with user changes or respawn loops.

## Honest results

R-01: Distinguish settings restored/already original, target exited/replaced, close refused/timed out, unsupported capability, recovery conflict, application reopened, reopen deferred and manual acknowledgement. Never collapse them into 'everything restored'.

R-02: Snapshot means our original values plus a durable change journal, not a Windows restore point or machine checkpoint. Reopening cannot restore unsaved work.

R-03: GPU scheduling preference is not a GPU percentage cap or exclusive reservation. No promise of zero non-game GPU usage, a fixed amount of reclaimed VRAM or a fixed FPS gain.

R-04: Required rendering and OS work may remain. Dedicated allocations, shared system memory, GPU engine activity and frame-time improvement must not be conflated.

R-05: Without a comparable benchmark, report 'gaming benefit not measured'. No fabricated saved-VRAM estimate from summing process counters.

## Feature availability and scope boundaries

A feature has one explicit availability state: implemented-and-tested for the recorded environment, experimental/opt-in, unsupported on this host, or not implemented. Missing adapters do not fall back to undocumented operations.

The complete behavioral baseline specifies failure as well as success. It does not promise that every Windows driver or third-party application supplies a pause API. CPU Sets, service/app-specific pause, launch-time GPU preference, automatic launcher handoff and supervised recovery require their own implementation evidence before becoming available. The first native session path must remain useful without those optional capabilities.
