# 02 - GPU and process policy

Status: initial specification, 2026-09-09. This is an implementation contract and feasibility plan, not a tested Windows implementation.

## GPU-first objective

Reduce unnecessary competing work on the game's active GPU and investigate memory pressure separately. Do not redefine success as a low total GPU utilization number: a game can legitimately use the GPU heavily after interference is reduced.

WDDM schedules GPU engines and manages video memory. Engines can operate concurrently; process memory accounting may include shared allocations. These are reasons to preserve adapter/engine identity and avoid naive summation. [S1]

All actions remain subject to the [product consent contract](01-product.md) and [recovery protocol](03-session-recovery-and-architecture.md).

## Observation model

Collect `ProcessIdentity`, application/group identity, owner/session, adapter identity, engine identifier/type, observation interval, GPU activity, dedicated/shared memory where available, CPU/I/O evidence, protection reason and telemetry quality.

A process identity is tied to its observed lifetime, not just its PID. An adapter uses a session-valid LUID plus descriptive hardware identity; never assume GPU 0 is the game's GPU. Multi-adapter activity remains separated. Device removal, driver restart or adapter changes invalidate observations and require re-probing.

The proposed collector uses runtime-discovered Windows GPU performance counters through PDH, including GPU Engine and GPU Process Memory where available. Locale-aware enumeration or the documented English-counter API must avoid hard-coded localized names. Missing counters, access failures and invalid sample status produce `unknown`, not zero. PDH's English-counter API is documented; presence and correctness of a GPU counter set still require host tests. [S2]

Detailed ETW/WPR/WPA or PresentMon capture is an explicit diagnostic path, not an unrestricted always-on trace. Identify PID reuse and stale samples before joining resource observations to an action target.

### Accounting requirements

- Keep 3D, compute, copy, video encode and decode work distinct when the provider exposes them. Do not add unrelated engine percentages into a fictitious total; missing engine types remain unknown.
- Rank optional candidates using sustained activity on the game's adapter and separately observed memory pressure. One instantaneous spike or a large allocation alone is not authorization to close an app.
- Do not add per-process memory values and claim the result is adapter-wide usage. Cross-process shared allocations can be double-counted. Shared GPU memory is also not equivalent to dedicated VRAM. [S1]
- `IDXGIAdapter3::QueryVideoMemoryInfo` describes the calling process's budget and usage; it is not an arbitrary game-PID memory-budget query. Do not show the optimizer's budget as the game's. [S3]
- Microsoft documents incorrect GPU Process Memory values on affected Windows 10 systems. Treat per-process values as estimates until cross-checked on the tested configuration; do not assume that historical issue exists on every Windows 11 system. [S4]
- Attribute DWM/shared-surface memory cautiously. Do not call it a leak or reclaimable waste solely from a process counter.

## Action selection

Protection and consent are hard constraints, not score penalties. A policy produces a finite, reviewable plan using these alternatives:

| Action | Intended mechanism | Product treatment |
| --- | --- | --- |
| Keep | Required or user-protected functionality | No mutation |
| Graceful close | End an unnecessary application's work | Core feature; explicit app-group consent |
| Force terminate | End an approved app that will not exit normally | Separate destructive consent; bounded and verified |
| Cooperative pause | Stop a specific supported background GPU workload | App-specific; CPU-thread suspension is not an equivalent substitute |
| Lower GPU scheduling class | Reduce scheduling preference of an eligible retained background app | Experimental until read/apply/verify/restore and performance tests pass |
| Supporting CPU/memory policies | Reduce other measured competition | Secondary; never described as a GPU ban |

There is no assumed supported, universal commodity-Windows API that gives arbitrary third-party apps a reliable GPU usage cap of zero while leaving all their other functions intact. Any future quota mechanism needs its own documented scope and evidence. Scheduling preference must never be advertised as such a cap.

## Application closure

Use an application-specific normal quit path when available. A generic GUI fallback may request `WM_CLOSE` on verified top-level windows using bounded messaging. A close message is a request and may invoke a save prompt; it is not proof the process or its GPU workers exited. [S5]

Do not close by wildcard image name, recursively kill an unreviewed process tree, or terminate only a browser's GPU helper while pretending the application has stopped. Establish a finite group of eligible processes using lifetime identity and verified application relationships. Re-check every target immediately before acting; parent PID or common filename alone is insufficient.

Record the request and wait for process-exit evidence. If helpers remain, report partial closure and keep observing their resource use. New helpers are new targets, not automatically included in old force consent. Reclaimed GPU resources are measured after exit; no immediate or exact byte release is promised.

On a save prompt, cancellation, access denial or timeout, keep the app open unless the user separately confirms force termination. Never auto-dismiss save dialogs. If a chosen closure cannot be completed, show the degraded plan and allow continuing or cancelling; do not silently escalate.

`TerminateProcess` is an optional implementation path with the necessary access rights. It initiates termination; pending I/O may delay completion and DLL cleanup is not normal application shutdown. Verify exit using a process handle and bounded waits, and report timeout/unknown explicitly. It can lose unsaved work. [S6]

No user process is actually terminated by writing these specifications. Future integration tests must use controlled fixture applications.

### Relaunches and new GPU consumers

Observe new process lifetimes during a session. Default behavior is notify/defer, not automatic killing. A previously authorized automatic graceful-close rule may be used only within its explicit scope, after identity and protection checks. A deliberate user reopen overrides automation for that session; ambiguous launch origin is treated conservatively.

Do not fight a launcher, service recovery loop or updater with repeated kills. The initial product makes at most one automated closure attempt per approved application per session; persistent respawn is reported for review. A future keep-closed policy needs separate approval, bounded retries, cancellation and tests.

## Cooperative GPU pause

An app adapter must specify: workload discovery, readable pause/running state, a supported pause operation, verification, resume and conflict detection. Pause only the workload selected by the user; do not claim generic support for all CUDA, Vulkan, browser or rendering applications.

Thread suspension is excluded as a general GPU-pause implementation. It can stop CPU-side execution without a verified release of GPU allocations or an application-safe pause. Windows documents deadlock risk when suspending threads that own synchronization objects. [S7]

## Low-level GPU scheduling spike

Investigate the documented `D3DKMTGetProcessSchedulingPriorityClass` and `D3DKMTSetProcessSchedulingPriorityClass` functions. They accept a process handle and expose a read/write scheduling class through Gdi32; this route does not require inventing a new kernel driver. [S8]

Enable an action only when the exact host/process combination permits querying the original class, setting an allowed lower class, verifying it and restoring it. Probe required rights, OS build, WDDM/driver behavior, HAGS state and API workload coverage on a controlled helper before enabling a profile. An API success code is not a benchmark result.

Never increase an already lower-priority background app's class. Preserve the original value. Below-normal is the initial candidate; idle/starving and realtime classes are not default optimization policies. Do not mutate games, DWM, protected processes or required low-latency companion apps in this first spike.

`IDXGIDevice::SetGPUThreadPriority` operates on a DXGI device interface; it is not a general way for our process to change another application's device. Do not inject into games to obtain such an interface. Microsoft warns that inappropriate GPU-priority changes can reduce rendering performance. [S9]

Scheduling changes do not promise a GPU percentage ceiling, immediate cancellation of queued work, memory reclamation or exclusive ownership. Unsupported or unreadable configurations fall back to approved closure/pause or observation, not undocumented privileged methods.

## Supporting low-level policies

| Policy | Capability and boundary | Rollback prerequisite |
| --- | --- | --- |
| CPU process priority | `GetPriorityClass` / `SetPriorityClass`; lower selected competitors, no Realtime. CPU priority alone does not control disk/memory or GPU work. [S10] | Read and restore the original class |
| EcoQoS | `GetProcessInformation` / `SetProcessInformation` with power-throttling information, where working on the target build. Not a GPU access control. [S11] | Query and round-trip Version, ControlMask and StateMask; skip if original management state cannot be read |
| Memory priority | Process memory-priority APIs where supported; no forced cache flush or claim of restoring page residency. [S11] | Read and restore the original priority |
| CPU Sets | Tested topology-aware process-default assignment; not exclusive core reservation, and thread/affinity constraints still matter. [S12] | Preserve the original set including no-assignment state; skip incompatible cases |
| Service/workload pause | App adapter or approved SCM stop; later scope, with dependency inspection. Never stop a shared service host or cascade through protected dependencies. [S13] | Verified original state, bounded completion and supported restart/resume |

CPU Sets, memory-priority mutation and service policies follow the first GPU/closure path. A documented API with a failed getter is not eligible for automatic reversible use.

Do not attach arbitrary running user apps to new Job Objects just to throttle them: the association cannot be undone for the existing process. Do not base I/O limiting on `SetIoRateControlInformationJobObject`, which Microsoft marks unsupported on Windows 10 1607 and newer. [S14]

## Display and hybrid-GPU boundaries

Required compositor/display/driver work is not waste to eliminate. Never terminate DWM, reset the display driver, disable an adapter, change monitor routing or force a MUX transition during a session.

Moving an already-running third-party app's GPU context to an iGPU is not an assumed feature. A future launch-time GPU preference feature must use a separately verified mechanism, disclose restart requirements and measure shared thermal/memory-bandwidth effects; it is not in the first implementation.

HAGS, MPO, fullscreen optimizations, refresh rate, HDR, VRR, graphics quality, driver profiles and frame generation remain unchanged in automatic sessions. No registry folklore is a substitute for an API contract.

## Primary evidence

Reviewed 2026-09-09. Documentation establishes API semantics, not product compatibility or measured gains.

- [S1 - Microsoft: GPUs in Task Manager](https://devblogs.microsoft.com/directx/gpus-in-the-task-manager/)
- [S2 - PdhAddEnglishCounterW](https://learn.microsoft.com/en-us/windows/win32/api/pdh/nf-pdh-pdhaddenglishcounterw)
- [S3 - QueryVideoMemoryInfo](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiadapter3-queryvideomemoryinfo)
- [S4 - GPU process memory counter issue](https://learn.microsoft.com/en-us/troubleshoot/windows-client/performance/gpu-process-memory-counters-report-wrong-value)
- [S5 - WM_CLOSE](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-close) and [bounded messaging](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendmessagetimeoutw)
- [S6 - TerminateProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess)
- [S7 - SuspendThread](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-suspendthread)
- [S8 - Get GPU scheduling class](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/d3dkmthk/nf-d3dkmthk-d3dkmtgetprocessschedulingpriorityclass) and [set GPU scheduling class](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/d3dkmthk/nf-d3dkmthk-d3dkmtsetprocessschedulingpriorityclass)
- [S9 - IDXGIDevice::SetGPUThreadPriority](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgidevice-setgputhreadpriority)
- [S10 - SetPriorityClass](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setpriorityclass)
- [S11 - GetProcessInformation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocessinformation) and [SetProcessInformation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setprocessinformation)
- [S12 - CPU Sets](https://learn.microsoft.com/en-us/windows/win32/procthread/cpu-sets)
- [S13 - Stopping a service](https://learn.microsoft.com/en-us/windows/win32/services/stopping-a-service)
- [S14 - Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) and [unsupported job I/O rate control](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-setioratecontrolinformationjobobject)
