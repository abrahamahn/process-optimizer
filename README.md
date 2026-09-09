# Process Optimizer

A native Windows game-session application focused on competing **GPU work**, consented background-process closure, and recoverable scheduling changes. Rust + Win32; no browser runtime, overclocking, permanent debloat or cloud service.

**Development alpha. Code is implemented; Windows build/test results and remaining hardware validation are tracked in GitHub Actions and the specifications. No FPS improvement or universal GPU restriction is claimed.**

## What this version does

- Samples Windows GPU Engine / GPU Process Memory counters, retaining adapter LUID and engine detail. Missing data is shown as unavailable, never invented as zero.
- Lets you select exact background process lifetimes. Keep is the default. Graceful close and separately approved force termination are distinct actions; no recursive or name-based kill sweep.
- Attaches to an actual running game or attempts to start its selected executable. A separate worker restores recorded settings after game exit or a Restore request, even if the UI window is closed.
- Offers individually selected GPU scheduling priority, CPU priority, EcoQoS and memory-priority controls where native getters/setters work. Read/apply/verify/restore is journaled; GPU priority is not a usage cap, memory reservation or GPU ban.
- Writes a local SQLite write-ahead recovery journal before every mutation. Reused PIDs are not touched; visibly changed values are retained as conflicts rather than overwritten.
- Protects Windows components, critical/other-user processes, known game-support and voice infrastructure, the selected game's family, and your protected applications. Unknown dependencies still require user review.

## Use

Build on Windows with the Rust MSVC toolchain and Visual Studio C++ Build Tools:

```powershell
.\scripts\build.ps1
.\target\release\process-optimizer.exe
```

A successful **Windows** GitHub Actions run also produces a portable x64 ZIP and SHA-256 checksum under that run's **Artifacts**. The executable is not code-signed; the artifact's source commit is included in the ZIP. Do not disable Windows security to run it.

Run normally, **not as administrator**. Start the game through Steam or its normal launcher first; refresh the process list, select the actual game process, and choose **Use selected as game**. Select optional background processes with Ctrl/Shift and assign **Lower priorities**, **Close gracefully**, or **Force-close allowed**. Inspect the plan and choose **START GAME SESSION**. Force termination has an additional confirmation. Save documents first.

**Restore now** asks the running worker to stop and restore. If the worker died, it starts recovery from the durable journal. **Show last report** explains outcomes. **Keep current / clear warning** explicitly acknowledges unresolved values; it does not pretend they were restored.

Closing an app cannot be undone. This version does **not** capture application memory or automatically reopen applications. A settings restore does not bring back unsaved documents, tabs, RAM/VRAM residency or closed workloads.

Settings and recovery records stay under `%LOCALAPPDATA%\ProcessOptimizer`. Do not delete them during an active/unresolved session. The current build has no installer, automatic startup service or kernel driver.

## Validation and limits

```powershell
cargo test --lib
cargo test --all-targets
cargo clippy --all-targets
cargo build --release --bin process-optimizer
```

Native mutation tests use their own disposable fixture processes, never arbitrary user apps. Hosted Windows tests cannot establish gaming GPU/HAGS/driver compatibility, actual FPS gains, input latency, controller/voice compatibility or sustained overhead on a physical machine.

Some counters or scheduling APIs may be unavailable. GPU allocations are not a promise of reclaimable physical VRAM. Required compositor/display work remains active. Advanced app-group resolution, supported cooperative GPU pause adapters, optional app reopening, automatic per-game profiles, CPU Sets, service controls and physical-GPU benchmarks are not complete in this alpha.

## Specification owners

| Document | Authority |
| --- | --- |
| [Product](specs/01-product.md) | Scope, priorities, consent, native UI, non-goals |
| [GPU and process policy](specs/02-gpu-and-process-policy.md) | GPU attribution, close/pause policies, low-level API boundaries |
| [Session recovery and architecture](specs/03-session-recovery-and-architecture.md) | Journal, identity, failures, recovery, native components |
| [Validation and delivery](specs/04-validation-and-delivery.md) | Acceptance scenarios, measured evidence, delivery gates |
