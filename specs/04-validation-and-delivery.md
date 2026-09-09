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
| 3. Low-level resource policies | GPU scheduling is experimental opt-in; CPU, EcoQoS and memory policies individually optional where query/apply/verify are supported | Per-adapter/driver real-machine capability + rollback evidence before recommending GPU policy; no guaranteed benefit from API success |
| 4. Application-aware management | Explicit executable groups, per-game recipe save/load/delete, separately approved GUI reopening and readable reports; whole-app/cooperative adapters remain absent | Product-workflow regression tests and native fixture/desktop checks; real applications and physical desktop transitions remain separate gates |
| 5. Verified performance | Paired game benchmark capture/reporting and hardware matrix | Not yet performed on physical gaming hardware |

The native worker and portable executable are a development alpha, not completion of every acceptance gate. Driver development, security disabling, power/overclock control and permanent debloating remain outside scope.

## Recorded evidence

### Native lifecycle baseline, 2026-09-09

[Main run 34334379794](https://github.com/abrahamahn/process-optimizer/actions/runs/34334379794) tested source `f5809b598f28912b3b3aec543b6c814ef642b336`. The workflow ran on GitHub-hosted Windows Server 2025 **10.0.26100**, x64 MSVC, Rust **1.90.0**, with a separate Linux logic-check job. This is not the user's gaming PC.

| Executed check in that Windows job | Result |
| --- | --- |
| Library unit suite | 35 passed |
| Behavioral contract suite | 17 passed |
| Native Windows contract suite | 7 passed; force test excluded from the default invocation |
| Explicit fixture-force invocation | 1 passed, only on the child created by that test |
| Native provenance/storage/lock suite | 4 passed |
| Production-worker lifecycle under a disposable non-admin account | 6 passed with no privilege bypass in the application |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed, no warnings |
| Release build, native-window smoke, read-only GPU probe and portable packaging | Passed |

**70 executed, passing test cases** across those Windows test invocations. The six end-to-end cases exercise actual game-fixture exit, manual restoration while the game stays alive, recovery after the owned worker is forcibly interrupted, cancellation before preparation, rejection of a duplicate worker, and a restarted background process not inheriting the original lifetime's settings. Fixtures establish and verify a known CPU-priority baseline so an unchanged already-low setting cannot falsely satisfy a round-trip test.

The end-to-end suite is ignored in ordinary local runs. `scripts/ci-normal-user.ps1` is guarded for isolated GitHub Windows CI; it creates and removes its own standard account, uses isolated stores and owned disposable binaries, and explicitly opts in to those six cases. It never changes an existing user account or searches for real user applications to terminate.

### Recovery ownership audit, 2026-09-09

[Audit run 34334785243](https://github.com/abrahamahn/process-optimizer/actions/runs/34334785243) applied the checksum-verified recovery patch to the reviewed `a78ae1dce066d1a3e5d8b816c98373f62225bee8` baseline and committed the exact formatted/tested source as `84c712bc3917ad3fd33f8388663774f5e625b926`. Environment: GitHub-hosted Windows Server 2025 **10.0.26100**, x64 MSVC, Rust **1.90.0**.

| Executed check in the audit job | Result |
| --- | --- |
| Library unit suite | 35 passed |
| Existing behavioral-contract suite | 17 passed |
| New recovery-ownership and journal-integrity suite | 15 passed |
| Native Windows contract suite | 8 passed; force test excluded from default invocation |
| Separately opted-in force fixture | 1 passed |
| Native provenance/storage/lock suite | 4 passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed without warnings |
| Release build, native-window creation and read-only GPU probe | Passed |

**80 executed, passing test cases** in that audit. Its 15 new deterministic scenarios cover cancellation or external drift before a setter, another actor independently choosing our intended value, cancellation before any close request, persistence failures at each apply/restore boundary, a competing writer after restore intent, persistent ownership conflicts, ambiguous native effects stopping later actions, invalid values, changed approvals/originals, missing game/provenance, duplicate or out-of-plan properties, and falsely completed records. A new native fixture test checks cancellation again inside the setter.

The audit did not include the concurrently added six production-worker lifecycle tests or static-runtime packaging change. Integration preserves those newer files and runs the normal full CI on the resulting main commit; do not count a planned integration as a passing run. Deterministic effect simulation is not exhaustive hardware power-loss testing.

### Capability and package boundaries

The native GPU scheduling query returned unavailable (`NTSTATUS 0xc0000022`) on these hosted runs, and its safe-fallback test passed. **This does not establish GPU priority application or gaming benefit.** The read-only PDH probe produced parseable measurements/provider status; hosted-provider availability is not physical gaming-GPU validation.

Subsequent main runs repeat all committed tests and emit a source-identified ZIP, executable checksum and package checksum. Windows x64 packaging uses a statically linked C runtime, configured in `.cargo/config.toml`; `scripts/check-portable.ps1` checks the actual release import table for external Visual C++ runtime DLL dependencies before packaging. The [Rust linkage reference](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes) documents the mechanism. Runtime-linkage changes still require the normal Windows checks to pass; flags alone are not proof of a portable binary.

Only normal read-only CI remains in the integrated tree. Temporary patch-transport and write-enabled audit workflow files are excluded from delivery. Session integrity validation detects structural inconsistency and prevents rewriting original approved records through the application; it does not claim to defeat arbitrary code already running as the same user. Reports are read from the local recovery database rather than automatically duplicating full inventories into a second JSON file.

No real-game FPS/latency improvement, VRAM reclaim amount, native visual-layout inspection, hybrid-GPU/HAGS behavior, anti-cheat acceptance, input/voice reconnect result or optimizer overhead budget is claimed. Keep these as explicit release gates, not hidden assumptions. The existing README and owner specs remain authoritative; do not create duplicate architecture/status documents merely to restate this table.

## Application-workflow validation

The application-workflow change adds regression coverage for finite group identity, group limits, recipe permissions, changed executables, fresh process resolution, protection, storage bounds, no-launch without verified closure, launch-intent/completion persistence faults, no duplicate reopening and schema-2 recovery without new privileges. Native worker tests exercise GUI executable approval, modified-stamp rejection and restoration-time reopening or explicit desktop deferral. A hosted result that is Deferred does not establish successful GUI launch. Record the committed-source run and the actual native outcome before making completion claims.
