# 04 - Validation, acceptance and delivery evidence

Behavioral baseline v1, 2026-09-09. This document owns validation. A complete specification does not mean implementation, real Windows gaming compatibility or performance gain is complete. Evidence must identify the environment and exact executed scope.

## Acceptance matrix

Scenarios map to [product](01-product.md), [policy](02-gpu-and-process-policy.md) and [recovery](03-session-recovery-and-architecture.md). Tests may combine scenarios but must not claim hardware coverage from a mock.

| Scenario | Required result | Contract |
| --- | --- | --- |
| Initial launch | Read-only, empty destructive plan, no remembered force/reopen/experimental consent | U-01 |
| Malformed/future/oversized request | Reject before mutation | U-06, J-04/J-05, W-01 |
| Missing review/nonessential acknowledgement | Reject before mutation | C-01, W-01 |
| Force without its distinct confirmation | Reject regardless of normal-close permission | C-02 |
| Experimental action without opt-in | Reject before mutation | L-04 |
| Game/protected/system/other-user target | Reject even with force consent | T-01..T-04 |
| Duplicate/conflicting actions | Reject instead of silently selecting a destructive one | U-06 |
| PID reused between preview/apply | Never mutate replacement | T-01, D-01 |
| Original below desired priority | No change; never raise it | L-02/L-06, W-06 |
| Missing original/unsupported getter | Skip without setter | W-02 |
| Save prompt/refusal/hung app | Bound request; no force fallback | A-01..A-04 |
| Normal close accepted but process alive | ClosePending, not closed/restored | A-03 |
| Explicit force on disposable fixture | Revalidate exact lifetime and verify exit | A-04 |
| Helpers survive or respawn | Report selected lifetimes only; no recursive repeat killing | T-05, A-05/A-06 |
| Cancel before apply | No mutation | C-04 |
| Cancel/game exit during apply | Stop new actions and restore earlier eligible settings | S-01/S-02 |
| Launcher exit/Alt-Tab/minimize | Never guessed as selected game exit | U-02, S-04 |
| Two controllers or duplicate Start | Exactly one mutation owner/unfinished session | J-02/J-03 |
| UI exits | Agent ownership and durable record remain | S-05 |
| Controller crash/restart | Startup reconciliation, not an instant-supervisor claim | S-05 |
| Failure before durable intent | Setter/close never called | W-03 |
| Failure after OS call before completion record | Reconcile durable intent; no irreversible replay | W-04/W-05, R-02 |
| Failure before/after restore write | Retry idempotently; retain pending intent | R-01 |
| User changes value during game | Conflict; do not overwrite | Restoration table |
| Process disappears/replaced at restore | Gone; no old-value replay | D-02 |
| Restore readback unavailable/different | RecoveryRequired, not completed | Restoration table |
| One rollback fails | Attempt independent recovery; retain failures | W-05 |
| Original OS-managed/nondefault state | Restore exact representable original | R-03 |
| Corrupt store/disk full/unknown schema | Fail closed, preserve evidence | J-05 |
| Reopen not approved/already open/identity changed/desktop locked | No launch; explicit skipped/deferred result | C-05, R-04 |
| Crash around reopening | No blind duplicate launch | R-05 |
| Acknowledge unresolved | Preserve record; label not restored; explicit review | R-06 |
| Missing/invalid/stale GPU sample | Unknown, not zero | G-02..G-04 |
| Multiple adapters/engines | Preserve keys; no naive sum or GPU-0 assumption | G-01/G-05 |
| Shared GPU allocations | No summed recoverable-VRAM claim | G-06 |
| Device/driver state changes | Invalidate capability/telemetry evidence | G-07 |
| Online + Bluetooth + voice + anti-cheat | Retain functionality on the actually tested host/game | P-04 |
| Hidden optimizer | No persistent rendering; measure overhead | U-07, G-08 |

## Implementation sequence and current boundaries

The behavioral baseline defines failure handling before release. The implementation includes pure policy/recovery, durable storage, exact native identity/protection, GPU-aware UI, approved close/force paths, independent controller and experimental GPU plus supporting scheduling policies.

Optional app reopening, cooperative workload/service adapters, CPU Sets, automatic launcher handoff, automatic profiles and supervised recovery remain unavailable. Their specified success/failure contracts are future acceptance gates, not implemented features. The current controller is a separate process using the same portable EXE's worker mode; the UI does not own that process's lifetime. Transport uses typed session records in the restricted SQLite store rather than a privileged command socket.

## Test layers

Pure tests cover consent/protection, identities, lower-only policy choice, telemetry parsing, states and reconciliation. Database tests exercise schema/serialization, unique unfinished-session ownership, duplicate IDs, failed saves and unresolved retention in temporary databases.

Fault-injection coverage includes saved-intent boundaries, simulated post-setter interruption, setter errors with uncertain effects, cancellation between actions/properties, external value changes, idempotent restoration and prevention of close replay. Exhaustive real process termination at every persistence boundary remains a separate release gate; passing mock tests must not be described as exhaustive machine-crash coverage.

Native tests read their own process or change only purpose-built disposable child fixtures. Force termination is an ignored-by-default test invoked separately in CI. File-lock/ACL tests use fresh test-owned temporary directories. No arbitrary user app is used as a destructive test target.

Hosted Windows CI validates exercised native APIs and the build. It does not establish consumer-GPU, Windows 11 gaming, localized counter, native UI usability, hybrid GPU, thermal or anti-cheat compatibility. A GPU test that handles an unavailable API is a capability-fallback test, not a successful GPU-priority mutation test.

## Recorded evidence

### Behavioral/native hardening, 2026-09-09

[Validation run 34332463606](https://github.com/abrahamahn/process-optimizer/actions/runs/34332463606) applied reviewed source corrections to the concurrent native implementation and committed the formatted/tested source as `bbd75f57a286b57b45d40244a98402e36dfe73dc`.

Environment: GitHub-hosted Windows Server 2025, build 10.0.26100, Rust 1.90.0, x64 MSVC. This was not the user's gaming PC.

| Executed check | Observed result |
| --- | --- |
| Library unit tests | 35 passed |
| Additional behavioral-contract tests | 17 passed |
| Windows contract suite | 7 passed; force test ignored in the default invocation |
| Separate explicit force-fixture test | 1 passed |
| Native provenance, controller lock and storage tests | 4 passed |
| `cargo clippy --locked --all-targets` | Passed without Rust/Clippy warnings |
| GPU scheduling query on disposable fixture | Unavailable: NTSTATUS `0xc0000022`; safe fallback exercised, GPU mutation not established |

There were 64 executed passing test cases across the default and separate force invocations. The GPU capability case is included in that count but did not demonstrate usable GPU scheduling on this runner.

The normal committed-source CI workflow additionally gates packaging on formatting, locked tests, warning-free Clippy, a release build, native window creation smoke test and read-only GPU probe. Its run and artifacts identify the exact packaged commit. At this evidence entry, those normal packaging results are not substituted for the completed hardening run or for physical gaming measurements.

### Remaining release gates

Actual Windows 11/GPU/HAGS/driver API round trips; localized counters; real game/anti-cheat/controller/voice behavior; visual/accessibility interaction review; exhaustive process-crash and power-loss faults; optional adapters; signed installer/update behavior; and performance/overhead measurements below remain unverified or not implemented. No FPS uplift, VRAM recovery amount, physical input-latency improvement or overhead budget is claimed achieved.

## Recovery ownership regression gate

The recovery audit adds deterministic cases for: cancellation or external drift before a setter; another actor independently choosing our intended value; cancellation before any close request; journal-write failures at each apply/restore boundary; a competing writer between restore intent and the native call; persistent ownership conflicts; ambiguous setter/close results stopping later actions; invalid property values; changed approvals/original values; missing game/provenance; duplicate or out-of-plan recovery properties; and a falsely completed record.

These cases use in-memory effects, not actual GPU performance. A separate Windows fixture test verifies cancellation inside the native setter without modifying any unrelated process. Results must be recorded from the completed run; adding a test file is not a pass.

## Benchmark protocol

Use a pinned PresentMon build or another documented capture path and declare metric definitions. PresentMon presentation data is not automatically physical end-to-end input latency. [V1]

Compare normal environment, observer-only, approved closure alone and closure plus each policy separately. Include clean baseline, realistic interference, explicitly labeled synthetic GPU/CPU contention and memory-pressure cases.

Keep resolution, quality, frame cap, VSync/VRR/HDR, frame generation, game/driver versions and power/fan profiles fixed. Record CPU/GPU identity, OS build, WDDM, HAGS, topology, thermal state and warm-up. Balance order and run at least five repetitions as an initial protocol, not guaranteed statistical certainty. Do not clear user caches to manufacture results.

Report per-run mean FPS, median/P95/P99 frame time, declared long-frame threshold and variation. Keep capture/display/generated-frame series distinct. This project's optional 1% low definition is `1000 / mean(slowest ceil(0.01*N) frame intervals in milliseconds)`, not automatically reciprocal P99.

GPU-engine changes and memory observations are distinct from gameplay benefits. Lower background usage without repeatable frame-time improvement is resource relief, not proven gaming improvement. Keep negative and inconclusive results. No fixed FPS promise.

## Initial overhead budgets (unachieved targets)

Hidden UI/controller combined mean CPU below 0.5% of total machine capacity over a steady 60-second window, combined private committed memory below 64 MiB and no persistent optimizer-created GPU rendering workload. Measure provider overhead and observer-only frame impact too.

Use bounded process/event waits. Coarse GPU observation uses roughly two-second intervals when explicitly refreshed; detailed tracing is a separate opt-in feature. No timer-resolution hack or submillisecond dashboard polling. Record missed budgets honestly.

## Delivery requirements

Committed source, lockfile, tests and portable build instructions stay together. Normal CI has read-only repository permissions; temporary source-preparation workflows are removed from the delivered tree. Packages include their source commit and checksums without local machine inventories. No installer/service/auto-start permission is granted by extracting a ZIP.

## Primary evidence

- [V1 PresentMon source/documentation](https://github.com/GameTechDev/PresentMon)

API semantics remain in the policy/recovery owners. Benchmark methodology and budgets are project choices.
