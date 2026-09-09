# Validation and delivery

Status: authoritative validation plan and implementation evidence; claims below distinguish implementation, CI checks and physical-device evidence.

Owners: [product](01-product.md), [policy](02-gpu-and-process-policy.md), [recovery](03-session-recovery-and-architecture.md). Do not duplicate their contract here.

## Acceptance scenarios

| Scenario | Required result |
| --- | --- |
| First-run scan | GPU visibility and preview without mutation; all processes default to Keep |
| No consent | No close, termination, pause or setting write |
| Graceful-close app with unsaved work | Preserve the app's save/cancel flow; do not automate its response |
| App ignores close | Show still running; do not silently force-kill |
| Explicit force approval | Only the exact approved process lifetimes are eligible; warn that work cannot be recovered |
| Multi-process browser/Electron app | Distinguish app shutdown from a GPU helper exit; unresolved membership is not blanket-approved |
| New or respawned GPU process | Notify/review only; never inherit a destructive rule automatically |
| GPU counters unavailable | Unknown/unavailable state, not fake 0% or success |
| Different GPU adapters | Keep per-adapter/engine attribution; do not blindly sum utilization or shared allocations |
| Priority getter unavailable/denied | Skip without a setter; report the unsupported capability |
| Setter or post-operation verification fails | Stop further application; retain durable intent and reconcile possible effects |
| User-protected voice/game-support app | Protection wins over rules and force-close requests |
| Networking/Bluetooth/audio/controller smoke | Online play, voice, input and reconnect behavior remain functional |
| Game exits during preparation | Stop remaining mutations; restore owned changes |
| Game crashes | Restore according to the same rules as normal exit |
| Alt-Tab/minimize | Session remains active; not treated as game exit |
| UI closes | Worker continues; user can reopen UI and inspect/stop the active session |
| Worker interruption after durable intent | Reconcile getter state; no PID-only replay or blind overwrite |
| Journal is corrupt or not durable | Fail closed; expose recovery-needed status |
| PID reused / original process ended | Never touch replacement; mark original-process-gone |
| Application changed a setting mid-game | Detect observable drift, preserve it and report a conflict |
| External change immediately around restore | Recheck after restore intent; acknowledge lack of atomic OS CAS and never claim perfect isolation |
| Setting intent saved but setter never called | Persist NotApplied when possible; do not take ownership of a later coincidentally matching value |
| Close intent saved but cancelled before request | Persist NotRequested when possible; do not create a fictitious completed or unconfirmed shutdown |
| Previously observed recovery conflict | Do not regain write permission merely because the value later matches our old setting |
| Modified approval/original-value records | Reject structural inconsistency or immutable-record changes before native actions |
| Session with unresolved settings/close outcomes | Cannot be persisted as fully restored |
| Application closed | Do not claim settings restoration recovered unsaved work |
| Optional reopen | Separately approved typed launch only; never elevate, replay commands or clear unresolved settings failures |
| GPU-driver reset / sleep / display switch | Invalidate capability/telemetry, stop unsafe pending actions, reconcile remaining owned settings |
| Multi-user / RDP session | Reject another user's targets; scopes do not merge |
| Duplicate session request | Exactly one active controller/journal ownership; return existing status or reject |
| Executable group selection | Finite current file/user/logon/session identities only; no implicit future-process rule |
| Target helper exits after preflight | Skip the exact exited target without cancelling the game session or sending another close |
| Profile load | New unapproved preview; no retained force, reopening or experimental permission |
| Profile expands beyond the limit | Reject entirely, never silently truncate the reviewed plan |
| Profile executable was replaced | Omit/reject changed file IDs; same-file updates still need fresh user review |
| Protected profile group | Exclude the protected group and explain it |
| GUI reopening without verified graceful closure | No launch |
| GUI reopening with changed stamp, existing instance or unavailable desktop | Explicit skipped/already-running/deferred result; no guessed startup |
| Interruption across GUI launch | Preserve uncertainty and never blindly repeat the launch |
| Upgrade with a stored schema-2 session | Read and recover the existing record, without granting new reopening permission |

Native mutation tests MUST use child fixtures created and owned by the test harness. They MUST NOT search for or close arbitrary desktop programs. Force-kill fixtures require an explicit opt-in test command, separate from the default suite. Read-only probe and native-window smoke checks may run in hosted Windows CI, but do not constitute a real-GPU performance test.

## Measurement protocol

Use controlled paired baseline/optimized runs of the same game build, scene/benchmark, resolution and quality settings. Match GPU mode, driver/HAGS state, display setup, power source, thermal warmup, refresh/sync settings and background workload. Use repeated alternating pairs; retain raw samples/conditions and report variability rather than selecting the best run. No hard-coded FPS uplift target.

Primary outcomes: frame-time distribution and low-frame performance using a pinned, documented PresentMon-based analysis; report exactly how percentiles/low-FPS values are computed and exclude warmup/loading only by a declared rule. Secondary outcomes: per-adapter background engine busy time, process GPU-memory accounting, CPU load, input/voice behavior, power/thermal behavior and optimizer overhead. A lower process count or memory number alone is not a successful optimization.

Overhead targets are budgets to validate, not measurements already achieved: no resident LLM/network dependency, no always-on overlay, no active UI animation during a minimized/hidden session, no continuous full-system trace. Initial idle/active-controller CPU target is below 0.5% of one logical processor averaged over 60 seconds and total resident private-memory target below 64 MiB during a quiet gaming session; record sampling rate and machine before claiming compliance. Tighten or revise only from evidence, never silently relabel regressions.

Compatibility matrix includes supported Windows 11 builds; single dGPU and hybrid iGPU/dGPU; NVIDIA/AMD/Intel where available; HAGS on/off observed without automatic toggling; full-screen/borderless; multi-monitor and external display; launchers/anti-cheat; Bluetooth reconnect; voice chat; sleep/resume; battery/AC transitions. Model/driver/API denial and unknown measurements must be normal represented states.

## Implementation slices and gates

| Slice | Current code scope | Exit evidence |
| --- | --- | --- |
| 1. First full session | Native UI, GPU inspection, exact process selection, close/force distinction, consent, journal and restore controls | Deterministic consent/recovery tests, native owned-fixture integration tests, native-window smoke |
| 2. Game lifecycle and resilience | Already-running game attachment, independent controller, durable cancellation, identity-safe reverse recovery, single controller/store and private state | Fault-injection and duplicate/cancellation tests plus non-admin production-worker lifecycle tests; physical sleep/reboot/logoff/driver-reset matrix remains open |
| 3. Low-level resource policies | GPU scheduling is experimental opt-in; CPU, EcoQoS and memory policies individually optional where query/apply/verify are supported | Per-adapter/driver real-machine capability and rollback evidence before recommending GPU policy; no guaranteed benefit from API success |
| 4. Application-aware management | Explicit executable groups, per-game recipe save/load/delete, separately approved GUI reopening and readable reports; whole-app/cooperative adapters remain absent | Product-workflow regression tests and native fixture/desktop checks; real applications and physical desktop transitions remain separate gates |
| 5. Verified performance | Measurement protocol specified; capture/report tooling and gaming measurements remain outstanding | Repeated paired gaming measurements and physical hardware compatibility evidence |

The native worker and portable executable are a development alpha, not completion of every acceptance gate. Driver development, security disabling, power/overclock control and permanent debloating remain outside scope.

## Recorded evidence

### Version 0.2.0 application workflows, 2026-09-09

[Validation run 34340104981](https://github.com/abrahamahn/process-optimizer/actions/runs/34340104981) tested the complete reviewed application changes and committed the exact formatted source as `2383a72878cd3f0f83a8771c53df3bc80ba14808`. This includes the database upgrade and expected-helper-exit corrections, not just the initial feature patch.

Environment: GitHub-hosted Windows Server 2025 **10.0.26100**, x64 MSVC, Rust **1.90.0**. This was not the user's gaming PC.

| Executed check | Observed result |
| --- | --- |
| Library and database unit tests | 36 passed |
| Existing behavioral-contract tests | 17 passed |
| Application-group, profile, reopening and report tests | 29 passed |
| Recovery ownership and persistence-boundary tests | 15 passed |
| Native Windows contract tests | 8 passed; force case ignored in default invocation |
| Separately opted-in force fixture | 1 passed |
| Native provenance, storage and locking tests | 4 passed |
| Production-worker lifecycle under disposable standard account | 8 passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed without Rust/Clippy warnings |
| Release build and portable-runtime import inspection | Passed; no external Visual C++ runtime DLL dependency |
| Native-window smoke and read-only PDH collector | Passed |

**118 executed passing test cases** across the separate Windows invocations; ignored cases are counted only when actually executed by their opt-in invocation, not twice.

The GUI-reopening worker case **actually created the new test GUI process and verified its new identity** on this run. It was not a Deferred-only success. The test first rejected an altered executable stamp, verified graceful closure, ended the owned game fixture, then verified the reopened process and cleaned up that exact child. A separate worker test rejects generic reopening for a windowless fixture. On other hosted desktops the same test may report Deferred; those runs must not be described as successful native launches.

The other worker cases cover game exit with no UI, manual restore while the game remains alive, recovery after forcible interruption of the owned worker, durable cancellation before apply, duplicate-controller rejection and replacement processes not receiving old settings. Fixtures establish a known CPU-priority baseline so a no-op cannot falsely satisfy the round trip.

The 29 product cases cover finite grouping, owner/logon/file identity and bounds; fresh profile expansion, protected groups, replaced executables, invalid or mixed recipes, storage replacement/deletion/count limits and absence of remembered approvals; optional reopening consent, verified closure, desktop/existing-instance/changed-file outcomes, persistence failure before or after launch, no duplicate launch after interruption, immutable evidence, complete reporting and expected helper exits versus real cancellation.

The database upgrade test reads an actual stored schema-2 JSON body with the new field absent through `get`, `latest` and `active`, then saves its recovered state without rewriting its schema. It verifies that a new reviewed schema-3 session can follow. An in-memory validator alone is not substituted for this database read path.

`tests/worker_e2e.rs` is ignored in ordinary local runs. `scripts/ci-normal-user.ps1` is guarded for isolated GitHub Windows CI, creates and removes its own standard account and explicitly opts in to all eight cases. No production privilege bypass, existing-account change or arbitrary user application termination is used.

### Earlier evidence

The [native lifecycle baseline](https://github.com/abrahamahn/process-optimizer/actions/runs/34334379794) and [recovery ownership audit](https://github.com/abrahamahn/process-optimizer/actions/runs/34334785243) remain available in their original runs and Git history. The prior [integrated 0.1.x run](https://github.com/abrahamahn/process-optimizer/actions/runs/34335461323) checked the earlier 86-case source at `85676f2cd9cac8b9cfef75a40d315d0cbf08f3dc`; it does not establish the new 0.2.0 features.

### Capability and delivery boundaries

The native GPU scheduling query returned unavailable (`NTSTATUS 0xc0000022`) on the application-workflow validation runner; its safe fallback passed. **This does not establish GPU priority application or gaming benefit.** PDH provider availability is not physical gaming-GPU validation.

Normal committed-source CI repeats formatting checks, locked tests, warning-free Clippy, all eight standard-user worker tests, release compilation, runtime import inspection, native-window creation and read-only GPU collection before packaging. Each produced ZIP identifies its own source commit and contains an executable checksum; an outer package checksum is also published. A preparation run is not substituted for the final packaged commit's CI result.

Windows x64 uses a statically linked C runtime, configured in `.cargo/config.toml`; `scripts/check-portable.ps1` checks the actual release import table. The [Rust linkage reference](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes) documents this mechanism. Runtime-linkage flags alone are not proof of a portable binary.

Only normal read-only CI is retained in the delivered tree. Temporary encoded source transport, correction scripts and write-enabled preparation workflows are removed. Reports read from the local recovery database rather than automatically duplicating machine inventories into JSON exports. Structural checks do not make a same-user local journal tamper-proof. Profile file IDs are not content hashes; every loaded recipe still requires fresh review.

Actual third-party app reopening, visual/accessibility interaction review, sleep/logoff/reboot/power-loss faults, hybrid GPU/HAGS/driver behavior, anti-cheat, input/voice reconnect, sustained optimizer overhead and game frame-time benefits remain unverified. General cooperative pause adapters, whole-app dependency discovery, unattended profile activation, supervised recovery and a signed installer/update path are not implemented. None of these are implied by the passing fixture suite.
