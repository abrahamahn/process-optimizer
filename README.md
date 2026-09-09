# Process Optimizer

A **native Windows GPU-first game-session optimizer**: inspect competing GPU work, close explicitly approved background processes, temporarily lower supported scheduling policies, and restore the settings it changed.

Rust + Win32. No web runtime, overclocking, permanent debloat, kernel driver, game injection, cloud service or resident LLM.

**Development alpha, not a proven FPS booster.** Native code and deterministic/Windows fixture tests are implemented. Actual gaming GPU compatibility, game/anti-cheat behavior and performance improvements remain unmeasured. See [validation evidence](specs/04-validation-and-delivery.md#recorded-evidence).

## New in 0.4.1 — explicit per-app controls and safety labels

Settings now treats every visible app as an explicit state choice instead of a one-way cleanup action.

- Each row shows both **Current** state and a conservative **Recommended** action. Game Mode choices are **Keep**, **Lower**, or **Close**. Supported Windows startup entries independently show **On** or **Disabled**.
- Selecting an app immediately updates the controls. A previously lowered or close-approved app can be changed back with **Keep / undo Game Mode rule**. A startup entry disabled by Process Optimizer can be changed back with **Restore Windows startup**.
- Known protected processes remain visible instead of disappearing from the list. They are placed first and rendered in red as **ESSENTIAL — DO NOT CHANGE**. Lower, Close and Disable startup are disabled for those rows. If an old rule or prior startup cleanup needs to be undone, the safe removal/restore control remains available.
- Optional apps are labeled **OPTIONAL** and receive guidance such as **KEEP**, **LOWER DURING GAME MODE**, **CLOSE DURING GAME MODE**, or **DISABLE STARTUP**. Recommendations are hints only; they do not grant permission or execute automatically.
- Game Mode and Windows startup are independent. For example, an app can remain installed and start normally while only being lowered during Game Mode, or its supported startup entry can be disabled while its Game Mode action remains Keep.

## New in 0.4.0 — two cleanup choices

Settings separates **Game Mode only** from **Permanent cleanup**.

- **Game Mode only** is temporary: approved apps may be normally closed or have supported CPU/EcoQoS/memory priorities reduced when you press **Turn ON**. **Turn OFF** restores settings changed by the optimizer; it does not resurrect closed apps or unsaved work.
- **Permanent cleanup** is deliberately narrow and reversible in 0.4.0: it can disable an exact matching **current-user Windows `Run` startup entry** after backing up the original value. It does **not** uninstall applications, disable Windows services/drivers/scheduled tasks, change security features, or force-close a running process. Unsupported startup sources are left untouched.
- **Copy / paste process list** opens a read-only text view. Drag-select text, or press **Ctrl+A → Ctrl+C**, and paste it directly into ChatGPT. The copied report omits executable paths, command lines, document/window content, SIDs, and account identifiers.

The Settings list also adds plain-language hints such as **Web browser**, **Overlay / monitoring**, **Developer tool**, **Apple sync / discovery**, **Audio hardware software**, and **ASUS utility**. These hints are guidance, not automatic permission: unknown and unapproved apps remain untouched.

## New in 0.3.0 — just Game Mode ON / OFF

The default window now has **Turn ON / Turn OFF** and **Settings**. There is no game browser or game-process selection. You can switch it on before opening Steam, start or switch games normally, then switch it off when finished. Game exit does not switch off a manual session.

**Initial setup:** open Settings, choose optional background apps, and approve **Allow normal close** or **Reduce background load**. A separate confirmation explains that each future user-initiated On may apply that action to current processes of the unchanged executable for 30 days. Keep removes the permission. Detected file updates and expired approvals require review. No app is preapproved and no unknown process is killed automatically.

**After setup:** Turn ON → play → Turn OFF. Settings remembers permissions, not game paths. Cleanup runs once at On; apps you reopen or launch later are not repeatedly closed. With no running matches, the screen explicitly says nothing changed. Off restores our eligible settings, not closed apps or unsaved documents.

Reduce load uses supported CPU, EcoQoS and memory priorities. GPU scheduling remains an experimental, separately approved option for the next activation only. Actual GPU contention reduction can come from normal-closing your approved GPU-using apps; scheduling preference is not a GPU ban.

The old process grid, per-game recipes and optional reopening remain available under **Settings > Advanced tools**. They are no longer the default workflow. Advanced attached sessions still restore when their selected game exits; manual sessions run until Off. The two modes share the same single-session and recovery protections.

## Current implementation

| Capability | What actually exists |
| --- | --- |
| Native application | Compact native On/Off window and Settings; detailed process tools remain in the optional Advanced window |
| GPU inspection | PDH GPU Engine and GPU Process Memory collection; adapter/engine detail; unknown/invalid samples are not fabricated as zero |
| Approved closure | Exact process lifetimes; bounded normal-close requests; separate direct force confirmation only in Advanced, never timeout-to-force escalation |
| Game session | Default manual mode lasts until Off, without a selected game. Optional Advanced sessions can still follow an exact game lifetime |
| GPU scheduling | Experimental, opt-in D3DKMT process scheduling class read/apply/verify/restore; denied/unsupported operations are skipped, not bypassed |
| Supporting policies | CPU priority, queryable EcoQoS and memory priority; never raise an already lower background priority |
| Per-app Settings control | Visible Current + Recommended state; explicit Keep/Lower/Close and supported startup Disable/Restore; protected rows stay visible and locked |
| Profiles and app groups | Default saved background-app permissions; optional Advanced per-game recipes and explicit executable grouping |
| Optional reopening | Advanced only: separately approved GUI executable after confirmed graceful close, with file/desktop checks and durable no-retry launch intent |
| Permanent cleanup | Reversible exact current-user `Run` startup disable/restore with backup-before-delete and conflict-safe restore; no service/task/driver fallback |
| Recovery | Durable SQLite intent before mutation; exact original values; PID/creation time/user/logon/file identity; external-change conflicts are preserved |
| Ownership and protection | One controller per user store; one unfinished database session; private recovery directory; known essential and user-protected apps are visible but mutation-locked |

There is no automatic whole-process-tree kill, future-process kill rule or claimed GPU usage cap. Selected-process closure is not necessarily whole-application closure: helpers may remain. Multi-adapter memory values are not added into an unlabeled reclaimable-VRAM number.

## Run a portable build

A successful **Windows** GitHub Actions run produces `process-optimizer-windows-x64.zip` and a SHA-256 checksum under its **Artifacts**. The ZIP contains the EXE, this guide, source commit and executable checksum. Extract the ZIP before opening the EXE. The binary is not code-signed; do not disable Windows security.

**Run normally, not as administrator. Save open work first.** Starting the application does not activate cleanup; no background apps are preapproved and experimental GPU scheduling is off by default.

1. Extract the ZIP and open `process-optimizer.exe` normally, not as administrator.
2. Open **Settings** and select an app. Check its **Current**, **Recommended**, and **SAFETY** information. Choose **Keep**, **Lower during Game Mode**, or **Close during Game Mode** for temporary behavior. For a supported unused current-user startup entry, choose **Disable Windows startup**; choose **Restore Windows startup** to undo it.
3. Press **Turn ON**. No game selection is needed. Start your game normally.
4. When finished, press **Turn OFF**. **Settings > Session details** contains exact outcomes; **Advanced tools** preserves the earlier explicit session controls.

Closing the UI does not abandon a running session: the independent controller remains. Alt-Tab/minimization does not end manual mode. If the controller itself crashes, immediate unattended recovery is not guaranteed; reopen the UI and choose **Restore settings**. **Settings > Recovery options** shows details before offering explicit acknowledgement; acknowledgement is not successful restoration.

Normal close can display an application's save prompt. The optimizer never answers it. If exit remains unconfirmed, the report keeps that uncertainty; it does not silently force termination or claim the request was undone.

## Recovery limits

A snapshot records our settings and action intent, **not machine/process/GPU memory**. A settings restore does not recover unsaved documents, closed workloads or old RAM/VRAM residency.

**GUI reopening is available only in Advanced and is narrowly gated, not general application-state restoration.** General cooperative GPU-pause adapters, service/CPU-Set policies, automatic Steam launcher handoff, unattended per-game activation, a recovery supervisor and an installer remain unavailable. Unavailable launch is visibly disabled in Advanced; no unsupported shortcut is used in its place.

State stays under `%LOCALAPPDATA%\ProcessOptimizer`. Do not delete it during an active/unresolved session. New records use schema 4 to distinguish manual and game-attached sessions. Schema-2/3 records remain recoverable without acquiring the new manual-mode behavior. Other unsupported/corrupt/future records fail closed and are preserved; never remove them merely to suppress a recovery warning. Protection is conservative but cannot infer every third-party dependency: the user must review which optional targets are genuinely nonessential.

## Upgrade from 0.1.x / 0.2.x

Finish the old session with **Restore now**, inspect the report, and close the old UI before replacing the executable. Do not delete `%LOCALAPPDATA%\ProcessOptimizer`: it contains recovery evidence. Version 0.3.0 reads and restores schema-2/3 sessions without granting them new permissions; new sessions use schema 4. Corrupt or unsupported records remain preserved rather than silently reset. App reopening starts a new process and cannot recover unsaved work.

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

GPU priority means scheduling preference, **not zero-GPU enforcement, reservation, memory eviction or guaranteed FPS gain**. Known essential networking, Bluetooth, audio/input, display, security and game infrastructure are protected. No HAGS/MPO/registry folklore, game quality changes, device resets or security disabling are performed.

[Product behavior](specs/01-product.md) defines the user contract. [GPU/process behavior](specs/02-gpu-and-process-policy.md) defines mechanisms. [Session/recovery](specs/03-session-recovery-and-architecture.md) defines durability and failures. [Validation](specs/04-validation-and-delivery.md) separates executed tests from remaining release gates.
