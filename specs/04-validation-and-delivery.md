# Validation and delivery

Status: authoritative validation plan and implementation evidence. Distinguish implemented behavior, CI checks and physical-device evidence.

Owners: [product](01-product.md), [policy](02-gpu-and-process-policy.md), [recovery](03-session-recovery-and-architecture.md). This document tests their contract; it does not replace it.

## Acceptance scenarios

| Scenario | Required result |
| --- | --- |
| First-run launch | Compact On/Off and Settings; no game selection; no preapproved cleanup |
| No app permission | No close, termination, pause or setting write; first-use On opens Settings |
| Manual On | Resolve a finite approved set of current process lifetimes without a game path or game PID |
| Manual game exit/switch | Mode stays active until Off; do not infer session end from an unrelated game |
| Manual Off | Stop new actions and restore eligible original settings; not automatic app reopening |
| Advanced attached game exit/crash | Restore under the same identity/ownership rules as before |
| Graceful-close app with unsaved work | Preserve save/cancel flow; never automate its response |
| App ignores close | Report still running or pending; never silently force-kill |
| Explicit Advanced force approval | Exact approved lifetimes only; warn that unsaved work cannot be recovered |
| Manual force/reopening request | Reject; saved normal-close consent cannot acquire either capability |
| Multi-process browser/Electron app | Distinguish app shutdown from helper exit; unknown membership is not blanket approval |
| New or respawned GPU process | No automatic destructive rule inheritance or repeated kill loop |
| GPU counters unavailable | Unknown/unavailable, not fabricated 0% or success |
| Different GPU adapters | Preserve adapter/engine attribution; do not sum unrelated utilization/shared allocations |
| Priority getter unavailable/denied | Skip without a setter and report unsupported capability |
| Setter or post-operation verification fails | Stop further application; retain durable intent and reconcile possible effects |
| User-protected/essential app | Protection wins over present or remembered cleanup permission |
| Missing, expired, revoked or future-dated permission | No new mutation under that permission |
| Updated/replaced file or wrong owner/session | Saved permission does not match; ask for review rather than widening scope |
| Networking/Bluetooth/audio/controller smoke | Retained operation and reconnect behavior on the tested host/game |
| Attached game exits during preparation | Stop remaining mutations and restore owned changes |
| Alt-Tab/minimize | Not a session-exit signal |
| UI closes | Worker continues; reopened UI can inspect/stop session |
| Manual worker interrupted | Recover from journal without requiring a live game; do not replay cleanup |
| Worker interrupted after durable intent | Reconcile getter state; no PID-only replay or blind overwrite |
| Corrupt or non-durable journal | Fail closed and expose recovery-needed status |
| PID reused/original ended | Never touch replacement; mark original process gone |
| App/another tool changes a setting | Preserve observable drift and report conflict |
| External change around restore | Recheck after restore intent; acknowledge no atomic Windows-wide CAS |
| Intent saved but setter never called | Persist NotApplied when possible; do not own a later coincidentally matching value |
| Close intent cancelled before request | Persist NotRequested when possible; no fictitious shutdown |
| Prior recovery conflict | Matching our old value later does not renew write permission |
| Modified approvals/original values | Reject structural inconsistency or immutable-record changes before native action |
| Unresolved settings/close outcomes | Cannot persist as fully restored |
| Closed app | Do not claim restored settings recovered unsaved work |
| Optional Advanced reopen | Separately approved typed launch only; no elevation/command replay or clearing unrelated recovery failures |
| GPU reset/sleep/display change | Invalidate stale evidence and stop unsafe pending actions; physical transition tests required |
| Multi-user/RDP | Reject other-user targets; scopes do not merge |
| Duplicate Start/controller | Exactly one active controller/store owner; reject or return existing state |
| Executable group selection | Finite current file/user/logon/session identities, not future-process authorization |
| Helper exits after preflight | Skip exact exited target without cancelling manual/game session or sending another close |
| Advanced profile load | New unapproved preview; no remembered force/reopening/experimental approval |
| Expansion beyond limit | Reject entirely; never silently truncate |
| Protected profile group | Exclude protected group and explain |
| Empty current matches in manual mode | Honest no-change state, never fallback to wildcard cleanup |
| Advanced GUI reopen without confirmed graceful exit | No launch |
| Reopen file changed/existing instance/desktop unavailable | Explicit skipped/already-running/deferred result |
| Interruption around launch | Preserve uncertainty; never blind duplicate launch |
| Schema-2/3 upgrade | Read and recover older records without granting new manual-mode or reopening permissions |

Mutation tests MUST operate on fixture children created and owned by the harness, never arbitrary user applications. Force fixtures require a separate explicit invocation. Native smoke/read-only collection on hosted Windows is not real-GPU performance evidence.

## Simple-mode UI acceptance

The default screen exposes only On/Off and Settings as actions. There is no game-path field, browser or game-process picker. Settings is hidden until opened and returns to compact mode through Done. Native smoke exercises both views and the retained Advanced screen. Synthetic screenshot capture contains no real process inventory.

Settings must make remembered app consent explicit: permitted action, file binding, 30-day expiry and removal through Keep. Experimental GPU permission is separate for the next activation; it is not permanently saved by an app rule. Modifying the default UI must not change legacy attached sessions into manual ones.

Initial approval is required but subsequent On/Off use does not require selecting a game or rebuilding process selections. Newly opened applications are not chased. Closing the UI leaves the independent worker running; no system-tray icon or immediate supervisor recovery is implied.

## Measurement protocol

Compare paired baseline/optimized runs of the same game build, scene, resolution and quality. Match GPU mode, driver/HAGS, displays, power source, thermal warmup, refresh/sync and background workload. Repeat alternating pairs, retain conditions/raw samples, and report variation rather than choosing the best run. No fixed FPS-uplift target.

Primary outcomes are frame-time distribution and low-frame performance using a pinned documented PresentMon-based analysis. Declare percentile/low-FPS definitions and any warmup/loading exclusions. Secondary outcomes are per-adapter background engine activity, memory accounting, CPU, input/voice operation, thermal/power behavior and optimizer overhead. Fewer processes or lower memory counts alone are not gaming improvement.

Overhead budgets remain unachieved targets: no resident LLM/network dependency, always-on overlay or continuous full-system tracing; no hidden/minimized UI animation. Initial controller CPU target is below 0.5% of one logical processor averaged over 60 seconds, total private memory below 64 MiB in a quiet session. Record provider sampling cost and observer-only frame impact. Revise targets only with evidence, not relabeling regressions.

The hardware matrix includes consumer Windows 11 builds; single/hybrid GPU; NVIDIA/AMD/Intel where available; existing HAGS states without automatic toggling; full-screen/borderless; multiple/external monitors; launchers/anti-cheat; Bluetooth reconnect/voice; sleep/resume and AC/battery transitions. Denied APIs and unknown observations must remain ordinary represented states.

## Implementation and release gates

| Area | Current scope | Gate |
| --- | --- | --- |
| Default use | Manual On/Off plus saved background-app permissions in Settings | Native views, exact permission resolution, cancellation and no-game worker tests |
| Advanced use | Process inventory, per-game recipes, force distinction and opt-in GUI reopening | Existing consent, profile, identity and reopening regression tests |
| Resilience | Independent controller, durable journal, exact-lifetime reverse recovery, one session/store | Fault simulation plus actual disposable standard-user worker tests; physical faults remain open |
| Low-level policies | Experimental GPU priority, supported CPU/EcoQoS/memory policies | Read/apply/verify/restore on each supported host; GPU not recommended on compile success alone |
| Performance | Protocol specified, physical capture/report tooling and measurements outstanding | Reproducible paired gaming and observer-overhead measurements |

This is a development alpha, not completion of every release gate. Driver development, overclocking, security disabling and permanent debloating remain excluded. General cooperative pause/service/CPU-Set adapters, automatic game detection, unattended activation, recovery supervision and signed installer/update delivery remain unavailable.

## Recorded evidence

### Version 0.3.0 manual On/Off, 2026-09-09

[Validation run 34348171462](https://github.com/abrahamahn/process-optimizer/actions/runs/34348171462) tested and published formatted source `049d69dd67c262b41dd69502c74b40a4bd259994`. Environment: GitHub-hosted Windows Server 2025 **10.0.26100**, x64 MSVC, Rust **1.90.0**. This was not the user's gaming PC.

| Executed check | Observed result |
| --- | --- |
| Library/database unit tests | 36 passed |
| Existing behavioral contracts | 17 passed |
| New manual-mode/permission tests | 20 passed |
| Application workflow tests | 29 passed |
| Recovery ownership/persistence boundaries | 15 passed |
| Native Windows contracts | 8 passed; force excluded from this invocation |
| Separately opted-in force fixture | 1 passed |
| Native provenance/storage/locking | 4 passed |
| Production worker under disposable standard user | 12 passed |
| Warning-free Clippy, release build and runtime import inspection | Passed |
| Default compact window/Settings and retained Advanced smoke | Passed |
| Read-only PDH collector and synthetic-window capture | Passed |

**142 executed passing cases**, counting ignored tests only when actually executed in their opt-in invocation.

The 20 new deterministic tests cover manual lifetime, no game path, permission ownership/expiry/revocation, changed file identity/stamp, protected groups, bounded current-process resolution, absence of force/reopen inheritance, storage and legacy behavior.

The four added standard-user worker cases exercise actual manual On without a selected game, remaining active after an unrelated game exits, Off restoring original CPU priority, recovery after interrupting the manual worker without a live game, revocation before mutation, and normal close of an approved GUI fixture. The prior eight attached-lifecycle/reopening cases also ran. Test fixtures establish a known original CPU priority, so an already-low no-op cannot pass as a successful round trip.

Native smoke checks verify that only the toggle and Settings actions are visible on the default screen and that Settings opens/closes without exposing the Advanced executable picker. A **616-by-375-pixel capture of the actual synthetic default window was visually inspected** for text visibility, spacing and absence of the game selector. This does not establish comprehensive high-DPI, keyboard, screen-reader or multi-monitor usability.

All native mutations used test-owned disposable processes. No application or device on the user's computer was changed. The GPU scheduling query returned unavailable (`NTSTATUS 0xc0000022`); the fallback passed, not a GPU priority round trip or FPS benchmark. The existing Advanced GUI reopen fixture actually created and verified its new process on this run; simple manual mode intentionally does not reopen closed applications.

Only application source/tests/docs were published by the temporary validation job. Its token was not granted additional workflow permissions; the normal CI file was left unchanged by that job. Temporary transport/correction scripts and write-enabled validation files are removed from the delivered tree. Ordinary main CI builds the resulting committed source and identifies its own package commit. Its result is checked separately before distributing the binary.

### Prior evidence

[Version 0.2.0 validation](https://github.com/abrahamahn/process-optimizer/actions/runs/34340104981) tested 118 cases and published source `2383a72878cd3f0f83a8771c53df3bc80ba14808`; [its integrated main run](https://github.com/abrahamahn/process-optimizer/actions/runs/34340678119) packaged `e72f1d8e4fecd2eba021adc1d51703274e94b2b6`. Those cases included finite executable groups, fresh per-game recipes, separate reopening approval, no duplicate launch after interruption and schema-2 database recovery. Their exact earlier evidence is retained in Git history, not presented as proof of manual mode.

The [0.1.x integrated run](https://github.com/abrahamahn/process-optimizer/actions/runs/34335461323), [native lifecycle baseline](https://github.com/abrahamahn/process-optimizer/actions/runs/34334379794) and [recovery ownership audit](https://github.com/abrahamahn/process-optimizer/actions/runs/34334785243) remain available. Earlier test totals are not added to the current total.

## Delivery and limits

Normal read-only CI gates formatting, locked tests, warning-free Clippy, opted-in fixture tests, release build, native-window smoke, PDH collection and packaging. Windows packages use the static C runtime from `.cargo/config.toml`; `scripts/check-portable.ps1` inspects actual imports for external Visual C++ runtime DLLs. The [Rust linkage reference](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes) documents the mechanism; flags alone are not executable proof.

Packages contain an executable checksum and source commit, with an outer package checksum. Do not distribute real machine inventories or raw user traces. Reports stay in the local recovery database; structural checks do not make a same-user store tamper-proof. Advanced profile file IDs are not content hashes. Default remembered permissions also check file size/modification stamp and expiry; these are conservative binding checks, not cryptographic guarantees against hostile same-user code.

Actual third-party app behavior, broad visual/accessibility interaction, sleep/logoff/reboot/power-loss faults, hybrid GPU/HAGS/driver behavior, anti-cheat, Bluetooth/voice reconnect, sustained overhead and game frame-time gains remain unverified. Do not claim these from passing hosted-fixture tests.
