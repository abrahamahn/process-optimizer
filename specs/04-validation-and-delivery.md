# 04 - Validation and delivery

Status: planned work, 2026-09-09. No tests, Windows execution or gaming benchmarks have been performed for this repository.

## Acceptance principle

Demonstrate that approved interference is reduced, required functionality survives and eligible settings are restored. Do not equate a smaller process count, more free memory or an API success return with a better game experience.

The [product](01-product.md), [policy](02-gpu-and-process-policy.md) and [recovery](03-session-recovery-and-architecture.md) documents own behavior; this document defines how to check it.

## Implementation slices

| Slice | Deliverable | Gate |
| --- | --- | --- |
| 0 - GPU visibility and dry run | Native application inventory, game/adapter selection, GPU evidence quality and action preview; no mutations | Controlled GPU fixture is attributed correctly; unknown counters and protected roles never become destructive recommendations |
| 1 - First usable session | Durable journal, consented graceful app-group closure, game lifetime tracking, native Start/Restore/report, optional separately approved reopening | Closure, refusal, partial exit, cancellation and interrupted recovery scenarios pass; result still does not claim FPS improvement |
| 2 - GPU policy and advanced close | Get/set/verify/restore scheduling-class spike; explicit force-termination path for eligible disposable fixtures; limited cooperative pause adapter where supported | Per-configuration compatibility recorded, performance compared, no automatic force escalation or protected target mutation |
| 3 - Supporting depth | Reversible CPU priority/EcoQoS; measured CPU Sets/memory policies; later selected service/workload adapters and supervised recovery | Each policy independently passes capability, restoration, functionality and overhead gates before default enablement |

Build vertical slices end-to-end. The first useful release is GPU-aware and can actually close approved applications; do not spend the first releases making a CPU-only priority tool.

No new kernel driver, generic plugin system, permanent debloat engine, cloud control plane or alternative Windows boot shell is a prerequisite.

## Behavioral acceptance scenarios

| ID | Scenario | Required result |
| --- | --- | --- |
| GPU-01 | One fixture performs 3D work, another video/compute work where available | Correct adapter/engine attribution; no misleading summed percentage |
| GPU-02 | Per-process memory includes shared allocations | No sum presented as physical adapter usage or guaranteed reclaimable VRAM |
| GPU-03 | Telemetry is missing, stale, denied or inconsistent | Unknown/degraded status, not zero use or unconditional optimization advice |
| GPU-04 | Game uses a different GPU from a background application | Keep the adapters separate; do not claim same-engine contention without evidence |
| GPU-05 | GPU scheduling getter/setter fails or the original class is unreadable | Skip mutation and report unsupported/denied; no privileged bypass |
| GPU-06 | GPU priority is lowered successfully but frametimes do not improve | Report no demonstrated gaming benefit; do not promote the policy on API success alone |
| GPU-07 | A fixture is lower priority already | Never raise its priority to the profile's nominal background setting |
| GPU-08 | An approved app exits but shared memory remains or helpers survive | Report observed delta/partial closure, not a fabricated exact VRAM release |
| APP-01 | Approved disposable application exits normally | Verify target exit, record closure, and separately offer/perform approved reopening |
| APP-02 | Application has a save prompt, refuses close or hangs | Leave it open after the bounded request; no automatic force termination |
| APP-03 | User explicitly confirms force termination of eligible fixture targets | Re-check exact targets and permissions, request termination, verify exit and warn that state is not restorable |
| APP-04 | A process exits and its PID is reused between preview and apply | Never mutate the replacement lifetime |
| APP-05 | Browser/helper relationship is ambiguous or changes after approval | Do not silently expand the target set or repeatedly kill the GPU helper |
| APP-06 | App respawns or user opens it during a session | Respect the bounded policy; user reopen/ambiguity prevents automated repeat closure |
| APP-07 | Background app is required for voice, capture, accessibility or device cooling | Protect it despite resource cost; changing optional app policy needs review |
| APP-08 | Protected process shares a name or appears in an approved tree | Protection wins; a name/tree match never authorizes the action |
| REC-01 | Inject a crash before and after every journal write and OS operation | Reconcile persisted intent; no blind irreversible replay or false completed status |
| REC-02 | User/another tool changes a setting during the game | Visible divergence produces conflict instead of overwriting |
| REC-03 | Original policy was non-default or OS-managed | Restore that exact representable original state, not a guessed Normal/default value |
| REC-04 | UI exits while a game runs | Independent controller retains ownership and restoration capability |
| REC-05 | Controller dies or machine reboots | Startup reconciliation honors process lifetime and documented supervisor limits |
| REC-06 | Disk full, access denied, journal corrupt or future schema | Fail before new mutations where possible, preserve records, show unresolved recovery |
| REC-07 | User already reopened a closed app | No duplicate launch; never replay old process settings into the new instance |
| REC-08 | User cancels after some applications closed | Restore eligible settings and offer reopening; explicitly report closure as non-reversible |
| SES-01 | Steam launcher exits but the actual game continues | Do not restore prematurely or treat launcher exit as game exit |
| SES-02 | Alt-Tab, minimize, lock/unlock or sleep/resume | No false exit; revalidate before further mutations |
| SES-03 | Game fails to launch or crashes immediately | Stop new actions and reconcile all changes already attempted |
| SES-04 | Second game starts or Start/Restore is clicked repeatedly | Protect both games, enforce single-session ownership and idempotence |
| SEC-01 | Unprivileged client forges a broker command or changes a target | Reject request; do not trust raw PID, path or unvalidated log content |
| SEC-02 | Required rights are absent or anti-cheat denies observation | Skip/degrade; do not inject, enable bypasses or disable protection |
| SYS-01 | Online play with Bluetooth controller and voice audio | Connectivity, input, voice and protected dependencies remain functional in the tested game |
| SYS-02 | Display adapter/driver state changes mid-session | Invalidate affected capabilities, stop stale actions and re-probe safely |
| SELF-01 | Optimizer window is hidden throughout a game | No persistent optimizer-owned GPU rendering workload; measure controller overhead |

These tests do not establish compatibility with all games or anti-cheat systems. Publish only the combinations actually tested.

## Test layers

Pure Rust tests cover plan decisions, consent precedence, identity matching, state transitions and simulated failure reconciliation without Windows.

Windows integration tests use purpose-built normal-close, save-prompt, hung-window, respawning-helper and controlled GPU-work fixtures. Never point destructive tests at arbitrary user apps. Force tests require an explicit test opt-in and a fixture identity check.

Fault injection must include the gap between an external mutation and its completion record, not just database transaction failures. Verify that one failed rollback does not block unrelated restoration. Test user-session separation, duplicated IPC requests and restart authorization.

Hardware tests record OS build, architecture, CPU/GPU identity, driver versions, WDDM, hybrid/dedicated topology, display configuration, game build and relevant graphics policies. Record HAGS and other existing settings; do not change them invisibly to improve a result.

## Performance experiment

Use a pinned, verified PresentMon build or another documented capture route with known metric definitions. PresentMon provides Windows presentation/performance capture; it does not make every game's physical input latency directly measurable. [V1]

Compare at least these conditions using the same instrumentation:

- Normal environment without optimizer mutations.
- Observer-only optimizer, to measure its own cost.
- Approved application closure/pause only.
- Closure/pause plus each candidate low-level policy individually, then the selected combination.

Run a reproducible scene or benchmark with fixed resolution, quality, frame cap, VSync/VRR/HDR, frame-generation mode, power/fan profile, driver and game version. Separate warm-up/shader-compilation effects from the measured segment without clearing user caches. Balance test order and record thermal state. Use at least five valid repeats per condition as an initial protocol choice, not a guarantee of statistical certainty.

Include a GPU-contended workload, CPU contention, memory-pressure case and an already-clean baseline. Do not manufacture contention in marketing and present it as a typical user's baseline. Controlled stress fixtures are explicitly labeled synthetic.

### Metrics and reporting

For valid comparable frame intervals, report per-run average FPS, median/P95/P99 frametime, long-frame counts using a declared threshold, and run-to-run variation. Keep loading screens and interactive play segments distinct according to a predeclared rule.

Define '1% low' explicitly if used: for this project, take the slowest `ceil(0.01 * N)` frame intervals and report `1000 / mean(those intervals in ms)`. Do not silently substitute the reciprocal P99 frametime or mix captured, displayed and generated-frame series.

Report background per-engine GPU activity and adapter-wide memory observations separately from game's activity. Also record CPU, disk, thermal/power indicators when reliably available, connectivity failures, crashes, display issues and recovery outcomes.

A measured memory delta is not automatically memory made available to the game. A lower non-game GPU counter without a frametime benefit is resource relief, not demonstrated gaming improvement. Without comparable captures, state 'not measured'. No fixed percentage performance promise.

Do not promote a default policy unless its intended benefit is repeatable in its target scenario and clean-baseline regressions are investigated. Negative, neutral and inconclusive outcomes must be retained, not averaged away or hidden.

## Initial overhead targets

Provisional reference-machine targets, not achieved results: hidden UI/controller combined average CPU below 0.5% of total machine capacity over a 60-second steady observation window, combined private committed memory below 64 MiB, and no persistent optimizer-created GPU command workload while hidden.

Start with roughly two-second coarse telemetry intervals and event-driven process-exit observation. Higher-resolution diagnostic capture is time-bounded and explicitly enabled. Measure provider/trace overhead, not just our process counters. No sub-millisecond polling or timer-resolution change is permitted merely to animate a dashboard.

Targets are revised only with recorded evidence and rationale. If they are missed, report the miss. Frame-impact evaluation must include normal run-to-run noise and the observer-only baseline; low optimizer CPU usage alone is not a pass.

## Open evidence before implementation claims

Exact GPU counter availability/semantics, safe app-group resolution, D3DKMT rights and driver/HAGS behavior, EcoQoS original-state round-trip support, normal quit/pause adapters, restart fidelity, supervision behavior and compatibility with individual games remain to be tested.

No application-specific integration is implemented merely because its name appears in a design example. No supported/recommended policy is declared until its tests, recovery limitations and performance evidence are recorded.

## Primary evidence

- [V1 - Intel/GameTechDev PresentMon source and documentation](https://github.com/GameTechDev/PresentMon)

Windows API evidence is maintained in [GPU and process policy](02-gpu-and-process-policy.md). Benchmark methodology, thresholds and delivery slices here are proposed project choices, not vendor guarantees.
