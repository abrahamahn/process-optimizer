# 02 - GPU and process behavior

Behavioral baseline v1, 2026-09-09. This owns observation semantics, protection and action mechanisms. [Product](01-product.md) owns consent; [recovery](03-session-recovery-and-architecture.md) owns writes and restoration.

## GPU observation

G-01: A sample identifies source, quality, monotonic interval, process lifetime, adapter LUID, physical adapter index where exposed, engine ID/type and units. Dedicated memory and shared memory are independent optional values. Adapter LUID is session-valid, not a persistent hardware key. Never assume GPU 0.

G-02: The initial collector uses Windows PDH with language-neutral English counter registration for GPU Engine / Utilization Percentage and GPU Process Memory / Dedicated Usage / Shared Usage when present. Two collection points are required for rate counters. English wildcard registration and formatted-array retrieval must be exercised on non-English Windows too. [S1][S2]

G-03: Collect an identity inventory before and after the measured interval. Only attribute a PID counter to a process when both inventories identify the same lifetime. New processes, exited/reused PIDs, invalid item status and missing sources become unknown/unattributed, not zero.

G-04: Validate returned buffer sizes, item counts, string boundaries, numeric finiteness and ranges. Bound allocation/retry attempts. Counter growth must cause a bounded re-query, never an unbounded allocation or pointer walk. Preserve unknown engine names rather than guessing their meaning.

G-05: Keep 3D, compute, copy, encode and decode engines separate. A 'busiest observed engine' summary is the maximum of known comparable engine observations and is labeled as such; it is not total GPU use and incomplete samples remain labeled partial. Never sum unrelated percentages.

G-06: Do not sum per-process memory to infer physical adapter usage or recoverable VRAM. Cross-process surfaces can be counted multiple times; shared GPU memory is not dedicated VRAM. `QueryVideoMemoryInfo` concerns the calling process, not an arbitrary game's PID. Historical GPU Process Memory counter issues require configuration-specific validation, not a claim all modern hosts are broken. [S3][S4][S5]

G-07: Runtime missing counters, unsupported driver paths, access denial, adapter change and stale data are explicit unknown/degraded statuses. Observation failure does not permit privileged fallback or make an app automatically disposable. Driver/device changes invalidate old attribution and capability evidence.

G-08: In normal mode use approximately two-second observation intervals when the inventory is explicitly visible/refreshed. The hidden controller must not continuously render or run detailed tracing. New consumers are never automatically terminated. Detailed ETW/WPR/WPA/PresentMon capture is separate, time-bounded and opt-in.

G-09: A candidate's resource evidence is advisory. For any automatic recommendation added later, require sustained same-adapter evidence from at least three valid intervals, and retain protection/consent as hard constraints. A one-off spike or a large memory allocation alone never authorizes an action.

## Target inspection and protection

T-01: Open a process with only the required rights. Validate PID, creation FILETIME, owning SID, interactive session, logon identity when available, executable path and file identity. Hold the handle across revalidation and action; never reopen a PID and assume it is the same lifetime.

T-02: Initial actions are same-user, same interactive-session, unelevated operations. Reject elevated optimizer operation rather than turning all actions into administrator operations. No SeDebugPrivilege, protected-process access bypass, game injection or own kernel driver.

T-03: Inspect critical/protected state and executable provenance. An inspection error means skip. Hard-protect Windows paths, system/session services, the game, optimizer executables, known required game/security/audio/input/network/thermal components and user-protected paths. Role-name heuristics are additional conservative protection, not sufficient proof of eligibility.

T-04: A game selection is protected before validating other plan targets. Known launchers cannot substitute for a real game lifetime. A plan involving the game or a hard-protected process is rejected before any mutation, even when force confirmation is present.

T-05: Generic application grouping is an explicit finite set of verified identities. Common filename, signer or parent PID alone does not authorize recursion. A newly spawned helper is not part of previous consent. Unknown membership means no group expansion.

T-06: User protection wins over a prior optimization plan. Restoration of an already-applied setting is a separate operation and does not require new optimization consent; identity and restoration safety checks still apply.

## Graceful close and optional force

A-01: Prefer a documented application quit adapter where implemented. Generic fallback enumerates verified top-level windows for each selected lifetime and sends bounded `WM_CLOSE` requests. Verify window ownership immediately before each request. Do not inspect window titles or answer application dialogs. Limit requests to 16 windows per target. [S6]

A-02: Each window request has at most a one-second message timeout. Overall close observation is bounded (initial default five seconds after requests). Failure/timeout is a recorded outcome, not force permission. Holding a process handle does not make HWND ownership changes atomic; minimize and disclose that residual race.

A-03: `WM_CLOSE` accepted is not exit. Verify process-handle signaling; if it remains live, report close requested/refused-or-pending rather than 'closed'. Some applications can later honor an already delivered close; cancellation cannot retract the message. Uncertain closure attribution cannot trigger automatic reopening.

A-04: Direct Force is allowed only with fresh, separate consent for the exact current identities. Call `TerminateProcess` with the required access and bounded exit verification. The call starts termination; pending I/O can delay completion. Unverified exit is indeterminate, not successful. No automatic normal-to-force escalation. [S7]

A-05: No normal/force request is automatically repeated on recovery or on a replacement lifetime. At most one automated close attempt per selected application per session. Respawn or a deliberate/ambiguous user reopen is reported, not fought with a kill loop.

A-06: Reclaimed processing/memory is measured after closure when telemetry is available. Helper survival/shared allocations prohibit whole-application or exact-byte claims. Exited selected processes are reported separately from observed GPU change.

## GPU scheduling

L-01: Use the documented process-handle APIs `D3DKMTGetProcessSchedulingPriorityClass` and `D3DKMTSetProcessSchedulingPriorityClass`. These expose scheduling class through Gdi32. Getter, setter, verification and exact restoration must all be possible for a target before treating a policy as reversible. [S8]

L-02: The candidate class is BelowNormal. If the original class is already BelowNormal or lower, record no change; never raise it to the configured nominal target. No game priority boost, Realtime or starving/Idle policy is applied by default.

L-03: Query original, durably journal intent, apply, query again and classify the result. API success is not GPU-workload coverage or performance evidence. Unsupported/denied read or write skips/fails this action without undocumented alternatives.

L-04: This is an experimental, explicitly opted-in capability until driver/HAGS/API-workload and hardware performance validation is recorded. A controlled-fixture read/set/read/restore probe establishes only API round-trip behavior. No experimental operation is silently promoted to a default because it compiles or works on a hosted runner.

L-05: Scheduling preference is not a hard GPU quota, allocation release, cancellation of submitted work, exclusivity or a guaranteed preemption policy. `IDXGIDevice::SetGPUThreadPriority` requires an application's own device interface and is not our cross-process mechanism. Do not inject to obtain it. [S9]

## Supporting policies

| Action | Desired value | Required readback/restoration |
| --- | --- | --- |
| CPU BelowNormal | Lower only if the existing class is above BelowNormal; preserve Idle and BelowNormal | Exact original process priority class; never Realtime [S10] |
| EcoQoS execution speed | Set execution-speed control/state bits only when the getter supports the target | Preserve complete Version/ControlMask/StateMask; unreadable OS-managed state means unsupported [S11] |
| Low memory priority | At most level 2; preserve an existing level 1 or 2 | Exact original process memory-priority value, not page residency [S11] |
| CPU Sets | Not a universal default; only a separately validated topology-specific policy | Original assignment including empty/unassigned state; thread affinity may override [S12] |
| Cooperative workload pause | Only a specific implemented app adapter | Prior running/paused state, supported pause/resume and conflict check |
| Service stop | Only a separately reviewed named-service adapter, not generic process termination | Original service state, dependency inspection, stop/start verification; never disable startup type [S13] |

L-06: CPU policies are secondary and MUST NOT be labeled GPU blocking. A successful setter with unreadable original state is not eligible for normal reversible use.

L-07: No arbitrary `SuspendThread`/process suspension as a GPU-pause substitute: synchronization owners can deadlock, and it is not a verified GPU-memory-release operation. No arbitrary Job Object attachment for a temporary policy, because an existing process cannot be detached. Do not use job I/O rate control marked unsupported on Windows 10 1607 and later. [S14][S15]

## Persistent startup cleanup

Persistent cleanup is **not** a gaming-session process mutation. Version 0.4 may enumerate and edit only `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` string values. An entry is eligible only when its first executable token can be conservatively resolved to the reviewed canonical executable. Environment-variable or shell-command ambiguity is unsupported.

Before deletion, store the exact value name, command string and value kind. Re-read immediately before deleting it; observable drift cancels the operation. Restoration never overwrites another value using the same name. No process is terminated by this action.

Do not fall back to disabling machine-wide startup, services, drivers, scheduled tasks, security software, device/audio/Bluetooth components, shell components or vendor utilities that cannot be attributed through the supported source. Future service/task cleanup requires its own owner contract, privilege design, dependency checks and native tests.

## Unsupported features and future adapters

Missing pause/service/CPU-set/launcher adapters return explicit unsupported/not implemented results. They never fall back to process suspension, cascading service stops or registry edits. An app adapter must define discovery, identity, readable prior state, supported operation, bounded verification, recovery and effect tests before registration.

Do not move live third-party GPU contexts to an iGPU, force MUX/display-route changes, terminate DWM or reset a driver. HAGS, MPO, fullscreen optimizations and graphics/driver profiles stay unchanged. A future launch-time GPU preference is a separate measured capability, not part of the generic session policy.

## Primary API evidence

Reviewed for contract semantics; real-host tests remain required.

- [S1 PdhAddEnglishCounterW](https://learn.microsoft.com/en-us/windows/win32/api/pdh/nf-pdh-pdhaddenglishcounterw)
- [S2 PdhGetFormattedCounterArrayW](https://learn.microsoft.com/en-us/windows/win32/api/pdh/nf-pdh-pdhgetformattedcounterarrayw)
- [S3 Microsoft: GPUs in Task Manager](https://devblogs.microsoft.com/directx/gpus-in-the-task-manager/)
- [S4 QueryVideoMemoryInfo](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_4/nf-dxgi1_4-idxgiadapter3-queryvideomemoryinfo)
- [S5 GPU Process Memory counter issue](https://learn.microsoft.com/en-us/troubleshoot/windows-client/performance/gpu-process-memory-counters-report-wrong-value)
- [S6 WM_CLOSE](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-close) and [SendMessageTimeoutW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendmessagetimeoutw)
- [S7 TerminateProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess)
- [S8 Get GPU class](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/d3dkmthk/nf-d3dkmthk-d3dkmtgetprocessschedulingpriorityclass) and [set GPU class](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/d3dkmthk/nf-d3dkmthk-d3dkmtsetprocessschedulingpriorityclass)
- [S9 SetGPUThreadPriority](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgidevice-setgputhreadpriority)
- [S10 SetPriorityClass](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setpriorityclass)
- [S11 GetProcessInformation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocessinformation) and [SetProcessInformation](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setprocessinformation)
- [S12 CPU Sets](https://learn.microsoft.com/en-us/windows/win32/procthread/cpu-sets)
- [S13 Stopping a service](https://learn.microsoft.com/en-us/windows/win32/services/stopping-a-service)
- [S14 SuspendThread](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-suspendthread)
- [S15 Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) and [unsupported job I/O rate control](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-setioratecontrolinformationjobobject)

## Native GUI reopening adapter

The initial adapter requires an ordinary local Windows-GUI PE image and an observed top-level window at opt-in/preflight. It excludes shells/interpreters/job hosts and reparse-path ambiguity. Before launch it checks the same owner/logon/session, current protection, original image file ID/size/write time and possible existing instances. A no-write/no-delete-sharing executable handle stays open through process creation. Launch uses the absolute reviewed path with no arguments and normal priority/privileges; old process policies are not transferred. This is not a content signature or perfect filesystem-wide race isolation.

Both WTS extended session state (Active and Unlocked) and the input desktop (Default) must be readable immediately before launch. Missing/locked/disconnected evidence produces Deferred; the adapter never unlocks the desktop or changes privileges. Windows 7/Server 2008 R2 reversed lock-flag behavior is outside the supported Windows 11 product target. OS checks are snapshots, not a promise that another actor cannot lock the desktop immediately afterward.

Primary API references: [WTSINFOEX_LEVEL1_W](https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/ns-wtsapi32-wtsinfoex_level1_w), [OpenInputDesktop](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openinputdesktop), [CreateFile sharing](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew), [CreateProcessW](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw).


## Manual Game Mode rules (0.3.0)

The default no-game-selection flow applies only explicit recurring background-app permissions. Before each native operation, re-read the permission, validate action kind, owner, file ID/size/write time and expiry, and perform existing critical/system/session/protection checks. Current foreground application evidence is an additional veto, never positive authorization or automatic game detection. Permission checks do not weaken the existing exact process-handle checks.

Normal-close permissions never allow ForceClose or generic suspension. Reduce load permits lower-only CPU/EcoQoS/memory policies and, only with the activation's fresh experimental consent, GPU scheduling. Required graphics work remains. Unknown apps are never altered. File updates, denied observations and protected groups are skipped/rejected rather than guessed.

Primary references for the foreground veto: [GetForegroundWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getforegroundwindow), [GetWindowThreadProcessId](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowthreadprocessid). Neither API establishes that an application is a game.
