# 04 - Validation, acceptance and delivery evidence

Behavioral baseline v1, 2026-09-09. This document owns validation. A complete specification does not mean the implementation, real Windows gaming compatibility or performance gain is complete. Evidence below must state the executed environment and exact scope.

## Acceptance matrix

Each scenario has a deterministic required result; identifiers map to [product](01-product.md), [policy](02-gpu-and-process-policy.md) and [recovery](03-session-recovery-and-architecture.md). Tests may combine scenarios but must not silently claim hardware coverage from a mock.

| Scenario | Required result | Contract |
| --- | --- | --- |
| Initial launch | Read-only, empty destructive plan, no remembered force/reopen/experimental consent | U-01 |
| Malformed/future/oversized request | Reject before mutation | U-06, J-04/J-05, W-01 |
| Missing review/nonessential acknowledgement | Reject before mutation | C-01, W-01 |
| Force without its distinct confirmation | Reject, regardless of normal-close permission | C-02 |
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
| Controller crash/restart | Startup reconciles; no instant-supervisor claim | S-05 |
| Failure before durable intent | Setter/close never called | W-03 |
| Failure after OS call before completion record | Reconcile prior durable intent; no irreversible replay | W-04/W-05, R-02 |
| Failure before/after restore write | Retry idempotently; do not erase pending intent | R-01 |
| User changes value during game | Conflict; do not overwrite | Restoration table |
| Process disappears/replaced at restore | Gone; no old-value replay | D-02 |
| Restore readback unavailable/different | RecoveryRequired, not completed | Restoration table |
| One rollback fails | Attempt independent remaining recovery, retain failures | W-05 |
| Original OS-managed/nondefault state | Restore exact representable original | R-03 |
| Corrupt store/disk full/unknown schema | Fail closed, preserve evidence | J-05 |
| Reopen not approved/already open/identity changed/desktop locked | No launch; explicit skipped/deferred result | C-05, R-04 |
| Crash around reopening | No blind duplicate launch | R-05 |
| Acknowledge unresolved | Preserve record, label not restored, unlock only by explicit review | R-06 |
| Missing/invalid/stale GPU sample | Unknown, not zero | G-02..G-04 |
| Multiple adapters/engines | Preserve keys; never naive sum or GPU-0 assumption | G-01/G-05 |
| Shared GPU allocations | No summed recoverable-VRAM claim | G-06 |
| Device/driver state changes | Invalidate capability/telemetry evidence | G-07 |
| Online + Bluetooth + voice + anti-cheat | Retained functionality on the actually tested game/host | P-04 |
| Hidden optimizer | No persistent rendering; measure collector/controller overhead | U-07, G-08 |

## Implementation order

1. Behavioral contracts and pure deterministic policy/recovery model.
2. Durable journal and fault-injection tests, then process identity/protection and Windows adapters.
3. Native inventory with GPU evidence and exact consented close path; independent game-session controller and recovery UI.
4. Explicit force path and opt-in experimental GPU scheduling; supporting CPU/EcoQoS/memory operations only when queryable/reversible.
5. Controlled Windows fixture tests, build artifacts, native interaction review and then real gaming benchmarks.

Do not replace steps 2-3 with a CPU-only priority utility. Do not expose missing pause/launcher/service/CPU-set integrations as working. Those optional adapters and a supervised recovery service require separate acceptance before activation.

## Test layers

Pure tests exercise request validation, consent/protection precedence, identities, lower-only policy choice, telemetry parsing, states and reconciliation on non-Windows too.

SQLite tests exercise schema/serialization, one unfinished session, duplicate IDs, failed writes and retention of unresolved records in temporary directories.

Fault injection must cover every externally visible mutation boundary: before intent save, after intent save, after OS effect, before completion save, before restore intent, after restore effect and before restoration completion. Include setting-readback failure and an API error that occurs after an effect. A database-only crash test is insufficient.

Windows integration tests operate only on purpose-built disposable child fixtures. Destructive fixture tests require an explicit environment opt-in and direct child identity validation. Never terminate/modify unrelated user applications to test the optimizer. Cover normal/refusing windows, held process identity, CPU round-trip and GPU-class capability query/round-trip when available. A skipped GPU test is reported as skipped/unsupported, not proof of GPU optimization.

Hosted Windows CI validates the toolchain and supported APIs exercised there. It is not a substitute for a consumer GPU, Windows 11 gaming environment, localized counter availability, native GUI usability, hybrid-GPU laptop, thermal behavior or an anti-cheat compatibility test.

## Benchmark protocol

Use a pinned PresentMon build or another documented capture path and record metric definitions. PresentMon captures presentation/performance data; not every metric is physical end-to-end input latency. [V1]

Compare normal environment, observer-only, approved closure alone, and closure plus each candidate policy separately. Include clean baseline, realistic background interference, explicitly labeled synthetic GPU/CPU contention, and memory-pressure cases.

Keep resolution, quality, frame cap, VSync/VRR/HDR, frame-generation mode, game/driver versions and power/fan profiles fixed. Record CPU/GPU identity, OS build, WDDM, HAGS, display topology, thermal state and warm-up. Balance test order and run at least five valid repetitions per condition as an initial protocol, not guaranteed statistical certainty. Do not clear users' caches to manufacture results.

Report per-run mean FPS, median/P95/P99 frame time, declared long-frame threshold and run-to-run variation. Keep capture/display/generated-frame series distinct. If reporting 1% low, this project defines it as `1000 / mean(slowest ceil(0.01*N) frame intervals in milliseconds)`; it is not automatically reciprocal P99.

GPU-engine changes and memory observations are separate from gameplay benefits. Lower background usage without repeatable frame-time improvement is resource relief, not demonstrated gaming improvement. Keep negative and inconclusive results. No predeclared fixed FPS uplift.

## Initial overhead budgets (unachieved targets)

Hidden UI/controller combined mean CPU below 0.5% of total machine capacity over a steady 60-second window, combined private committed memory below 64 MiB and no persistent optimizer-created GPU rendering commands. Measure provider overhead and observer-only frame impact too, not only our process counters.

Use bounded event waits for game exit and cancellation. Coarse observations are roughly two seconds; detailed tracing is opt-in and time-bounded. No timer-resolution hack or submillisecond polling for dashboard animation. Record any missed budget instead of relabeling it achieved.

## Delivery and truthful status

Source, tests and portable Windows build instructions belong in the same repository. CI should build normal-user executables, run safe fixture tests and retain downloadable artifacts without uploading user state. No installer/service/auto-start permission is silently granted by extracting a build.

Maintain an explicit README implementation matrix and an evidence entry for each delivered increment: commit/build context, commands, pass/fail/skipped scope, platform and remaining unverified behavior. Do not call a Windows app tested because Linux policy tests pass, or call it a performance optimizer proven because an EXE was produced.

Current evidence at this specification revision: behavioral contracts reviewed against the discussed requirements; implementation and runtime tests have not yet been delivered. Subsequent code commits must replace this sentence with actual evidence, not an unconditional completion claim.

## Primary evidence

- [V1 PresentMon source/documentation](https://github.com/GameTechDev/PresentMon)

API semantics are sourced in the policy/recovery owners. Test methodology and budgets are project decisions.
