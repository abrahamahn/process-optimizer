# Process Optimizer

A **native Windows GPU-first game-session optimizer**: inspect competing GPU work, close explicitly approved background processes, temporarily lower supported scheduling policies, and restore the settings it changed.

Rust + Win32. No web runtime, overclocking, permanent debloat, kernel driver, game injection, cloud service or resident LLM.

**Development alpha, not a proven FPS booster.** Native code and deterministic/Windows fixture tests are implemented. Actual gaming GPU compatibility, game/anti-cheat behavior and performance improvements remain unmeasured. See [validation evidence](specs/04-validation-and-delivery.md#recorded-evidence).

## New in the application-workflow alpha

**Select same app** expands a finite group with the same executable file, user, logon and Windows session. Review the rows and choose an action; different executables and future processes are not automatically included. This is executable grouping, not a claim that every helper belongs to the same app.

**Save / Load game profile** stores a local recipe for the selected game. Loading explicitly rebuilds a preview from current process identities. Replaced executable file identities and protected groups are omitted or rejected with an explanation. Profiles bind file IDs, not cryptographic content hashes; in-place updates still require your fresh review. No profile is auto-activated. Force permission, app-reopening approval and experimental GPU approval are never saved. A profile may expand to more current processes than the original selection; review every row. Profiles can be replaced or deleted without deleting recovery records.

**Reopen after close...** separately approves one main GUI process already marked **Close gracefully**. After a verified graceful closure, restoration may start the unchanged executable once, normally and without arguments. Console jobs, interpreters, helpers without a top-level window, changed files and possible existing instances are excluded. Locked, disconnected or unverifiable desktops defer reopening to the user. A crash around launch leaves an explicit uncertain result, never a blind retry. Reopening does not recover tabs, documents, unsaved work or memory. Toggle the same control again to remove permission before Start.

**Show last report** now separates settings, close requests and reopening outcomes in readable text. It does not present app reopening as undoing a shutdown.

## Current implementation

| Capability | What actually exists |
| --- | --- |
| Native application | Win32 controls, keyboard navigation, DPI-aware layout, process/action preview, protected-app settings and Restore/report controls |
| GPU inspection | PDH GPU Engine and GPU Process Memory collection; adapter/engine detail; unknown/invalid samples are not fabricated as zero |
| Approved closure | Exact selected process lifetimes; bounded normal-close requests; **separate direct force confirmation**, never timeout-to-force escalation |
| Game session | Attach to an explicitly selected already-running game; independent controller process; restore after selected game exit or user request |
| GPU scheduling | Experimental, opt-in D3DKMT process scheduling class read/apply/verify/restore; denied/unsupported operations are skipped, not bypassed |
| Supporting policies | Individually selected CPU priority, queryable EcoQoS and memory priority; never raise an already lower background priority |
| Profiles and app groups | Explicit executable-group selection and per-game recipe save/load/delete; no stored consent or unattended activation |
| Optional reopening | Separately approved GUI executable after confirmed graceful close, with file/version and desktop checks plus durable no-retry launch intent |
| Recovery | Durable SQLite intent before mutation; exact original values; PID/creation time/user/logon/file identity; external-change conflicts are preserved |
| Ownership and protection | One controller per user store; one unfinished database session; private recovery directory; system/game/launcher/security/device/accessibility and user protections |

There is no automatic whole-process-tree kill, future-process kill rule or claimed GPU usage cap. Selected-process closure is not necessarily whole-application closure: helpers may remain. Multi-adapter memory values are not added into an unlabeled reclaimable-VRAM number.

## Run a portable build

A successful **Windows** GitHub Actions run produces `process-optimizer-windows-x64.zip` and a SHA-256 checksum under its **Artifacts**. The ZIP contains the EXE, this guide, source commit and executable checksum. Extract the ZIP before opening the EXE. The binary is not code-signed; do not disable Windows security.

**Run normally, not as administrator. Save open work first.** Starting the application only observes; no background processes are preselected for closure and experimental GPU scheduling is off by default.

1. Start the game through Steam or its normal launcher. Open Process Optimizer and choose **Refresh GPU / processes**.
2. Select the actual game process and choose **Use selected as game**. A launcher or a browsed executable path is not a substitute for the running game lifetime.
3. Select optional background processes with Ctrl/Shift. Assign **Close gracefully**, **Lower priorities**, or the separately confirmed **Force terminate** action. Use **Keep** or the protection control for anything needed by the game, voice, accessibility or device operation.
4. Before Start, optionally expand a selection with **Select same app**, save/load a game profile, or separately approve **Reopen after close...** for one main GUI process. These controls only edit the unsent preview.
5. Inspect the exact plan and choose **START GAME SESSION**. Lower-priority actions require an explicitly selected policy. GPU scheduling prompts for additional experimental consent; force termination has its own target-specific confirmation.
6. **Restore now** stops further optimization and restores eligible settings. If the controller died, it starts journal recovery. **Show last report** distinguishes restoration, exited targets, pending close requests and conflicts.

Closing the UI does not abandon a running session: the independent controller remains. Alt-Tab/minimization is not game exit. If the controller itself crashes, immediate unattended recovery is not guaranteed; reopen the UI and choose **Restore now**. **Keep current / clear warning** explicitly acknowledges unresolved items; it is not a successful restoration.

Normal close can display an application's save prompt. The optimizer never answers it. If exit remains unconfirmed, the report keeps that uncertainty; it does not silently force termination or claim the request was undone.

## Recovery limits

A snapshot records our settings and action intent, **not machine/process/GPU memory**. A settings restore does not recover unsaved documents, closed workloads or old RAM/VRAM residency.

**GUI reopening is optional and narrowly gated, not general application-state restoration.** General cooperative GPU-pause adapters, service/CPU-Set policies, automatic Steam launcher handoff, unattended per-game activation, a recovery supervisor and an installer remain unavailable. Unavailable launch is visibly disabled; no unsupported shortcut is used in its place.

State stays under `%LOCALAPPDATA%\ProcessOptimizer`. Do not delete it during an active/unresolved session. New session records use schema 3 for separate reopening approval and outcomes. Schema-2 records remain recoverable without acquiring the new launch capability. Other unsupported/corrupt/future records fail closed and are preserved; never remove it merely to suppress a recovery warning. Protection is conservative but cannot infer every third-party dependency: the user must review which optional targets are genuinely nonessential.

## Build from source

Windows x64, the Rust MSVC toolchain and Visual Studio C++ Build Tools are required. `rust-toolchain.toml` pins Rust; `Cargo.lock` pins resolved dependencies.

```powershell
.\scripts\build.ps1
.\target\release\process-optimizer.exe
```

The script runs formatting, tests, static checks and a release build. It does not start an optimizer session. Native tests change only their own disposable fixtures. The force test is ignored by default and can be requested separately:

```powershell
cargo test --locked --test windows_contract -- --ignored --nocapture
```

Cross-platform logic tests use `cargo test --locked --all-targets`; Windows-specific tests are conditionally compiled. A Linux test pass does not establish Windows behavior. Hosted Windows tests are not physical-GPU gaming benchmarks.

## Scope and evidence

GPU priority means scheduling preference, **not zero-GPU enforcement, reservation, memory eviction or guaranteed FPS gain**. Network, Bluetooth, audio/input, display, security and game infrastructure are protected. No HAGS/MPO/registry folklore, game quality changes, device resets or security disabling are performed.

[Product behavior](specs/01-product.md) defines the user contract. [GPU/process behavior](specs/02-gpu-and-process-policy.md) defines mechanisms. [Session/recovery](specs/03-session-recovery-and-architecture.md) defines durability and failures. [Validation](specs/04-validation-and-delivery.md) separates executed tests from remaining release gates.
