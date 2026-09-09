# Process Optimizer

A **native Windows GPU-first game-session optimizer**: inspect competing GPU work, close explicitly approved background processes, temporarily lower supported scheduling policies, and restore the settings it changed.

Rust + Win32. No web runtime, overclocking, permanent debloat, kernel driver, game injection, cloud service or resident LLM.

**Development alpha, not a proven FPS booster.** Native code and deterministic/Windows fixture tests are implemented. Actual gaming GPU compatibility, game/anti-cheat behavior and performance improvements remain unmeasured. See [validation evidence](specs/04-validation-and-delivery.md#recorded-evidence).

## Current implementation

| Capability | What actually exists |
| --- | --- |
| Native application | Win32 controls, keyboard navigation, DPI-aware layout, process/action preview, protected-app settings and Restore/report controls |
| GPU inspection | PDH GPU Engine and GPU Process Memory collection; adapter/engine detail; unknown/invalid samples are not fabricated as zero |
| Approved closure | Exact selected process lifetimes; bounded normal-close requests; **separate direct force confirmation**, never timeout-to-force escalation |
| Game session | Attach to an explicitly selected already-running game; independent controller process; restore after selected game exit or user request |
| GPU scheduling | Experimental, opt-in D3DKMT process scheduling class read/apply/verify/restore; denied/unsupported operations are skipped, not bypassed |
| Supporting policies | Individually selected CPU priority, queryable EcoQoS and memory priority; never raise an already lower background priority |
| Recovery | Durable SQLite intent before mutation; exact original values; PID/creation time/user/logon/file identity; external-change conflicts are preserved |
| Ownership and protection | One controller per user store; one unfinished database session; private recovery directory; system/game/launcher/security/device/accessibility and user protections |

There is no automatic whole-process-tree kill, future-process kill rule or claimed GPU usage cap. Selected-process closure is not necessarily whole-application closure: helpers may remain. Multi-adapter memory values are not added into an unlabeled reclaimable-VRAM number.

## Run a portable build

A successful **Windows** GitHub Actions run produces `process-optimizer-windows-x64.zip` and a SHA-256 checksum under its **Artifacts**. The ZIP contains the EXE, this guide, source commit and executable checksum. Extract the ZIP before opening the EXE. The binary is not code-signed; do not disable Windows security.

**Run normally, not as administrator. Save open work first.** Starting the application only observes; no background processes are preselected for closure and experimental GPU scheduling is off by default.

1. Start the game through Steam or its normal launcher. Open Process Optimizer and choose **Refresh GPU / processes**.
2. Select the actual game process and choose **Use selected as game**. A launcher or a browsed executable path is not a substitute for the running game lifetime.
3. Select optional background processes with Ctrl/Shift. Assign **Close gracefully**, **Lower priorities**, or the separately confirmed **Force terminate** action. Use **Keep** or the protection control for anything needed by the game, voice, accessibility or device operation.
4. Inspect the exact plan and choose **START GAME SESSION**. Lower-priority actions require an explicitly selected policy. GPU scheduling prompts for additional experimental consent; force termination has its own target-specific confirmation.
5. **Restore now** stops further optimization and restores eligible settings. If the controller died, it starts journal recovery. **Show last report** distinguishes restoration, exited targets, pending close requests and conflicts.

Closing the UI does not abandon a running session: the independent controller remains. Alt-Tab/minimization is not game exit. If the controller itself crashes, immediate unattended recovery is not guaranteed; reopen the UI and choose **Restore now**. **Keep current / clear warning** explicitly acknowledges unresolved items; it is not a successful restoration.

Normal close can display an application's save prompt. The optimizer never answers it. If exit remains unconfirmed, the report keeps that uncertainty; it does not silently force termination or claim the request was undone.

## Recovery limits

A snapshot records our settings and action intent, **not machine/process/GPU memory**. A settings restore does not recover unsaved documents, closed workloads or old RAM/VRAM residency.

**Automatic app reopening is not implemented in this alpha.** Neither are general cooperative GPU-pause adapters, service/CPU-Set policies, automatic Steam launcher handoff, automatic per-game profiles, a recovery supervisor or an installer. Unavailable launch is visibly disabled; no unsupported shortcut is used in its place.

State stays under `%LOCALAPPDATA%\ProcessOptimizer`. Do not delete it during an active/unresolved session. New session records use schema 2 for strengthened identity and consent. An older/corrupt/future record fails closed and is preserved; never remove it merely to suppress a recovery warning. Protection is conservative but cannot infer every third-party dependency: the user must review which optional targets are genuinely nonessential.

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
