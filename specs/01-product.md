# 01 - Product

Status: initial specification, 2026-09-09. No implementation or measured performance improvement is claimed.

This document owns product scope. [Policy](02-gpu-and-process-policy.md), [recovery](03-session-recovery-and-architecture.md) and [validation](04-validation-and-delivery.md) own their respective details.

## Purpose

Temporarily turn a normal Windows workstation into a less-contended gaming environment, then recover the settings changed by the optimizer. The user should not have to permanently debloat Windows or manually close and reopen the same approved applications every time.

**Primary objective: minimize unnecessary non-game work on the GPU used by the game, while preserving the operating system and the user's required functions.**

This is not a process-count contest, a RAM-cleaning utility, or an overclocking tool. A reduction in resource counters is useful evidence, but not proof of higher FPS or smoother frames.

## Confirmed requirements

| ID | Requirement |
| --- | --- |
| P-01 | GPU processing contention is the first optimization priority; distinguish it from GPU memory pressure. |
| P-02 | Closing unnecessary background applications with user consent is a core feature, not an excluded action. |
| P-03 | Graceful close and optional force termination are distinct operations with distinct consent. |
| P-04 | Low-level temporary policies are in scope when their effect, compatibility and recovery can be demonstrated. |
| P-05 | A session snapshot plus a durable change journal must support restoration of settings we changed. |
| P-06 | The application must be native Windows software with a small, understandable settings surface. |
| P-07 | Online play, Bluetooth, input, audio, display, security and required game infrastructure must remain functional. |
| P-08 | CPU/GPU overclocking and voltage tuning are excluded. |

Rust with Win32 is the initial implementation direction. Exact compatibility ranges, API permissions and performance budgets below are engineering proposals requiring validation, not facts about a released product.

## Priority and first release

First: identify background GPU consumers and provide an approved application-close or supported workload-pause path.

Second: investigate reversible GPU scheduling policies for eligible applications that remain open. These are best-effort contention controls, not GPU percentage caps.

Third: reduce supporting CPU, memory and I/O contention without breaking retained functionality.

The first usable release must contain GPU visibility, consented closure, a native Start/Restore interface and recovery records. A release containing only CPU priority and EcoQoS changes does not satisfy this scope.

Proposed first validation target: Windows 11 x64 on an explicitly recorded OS build and driver configuration. Other Windows versions, ARM64 and vendor/driver combinations are unverified until tested; do not infer support from compilation alone.

## Session experience

1. Select a game or attach to a verified running game. Show the selected rendering adapter when it can be determined.
2. Inspect the application's resource use and present a concrete action preview. Nothing destructive is pre-approved merely because an application is in the background.
3. Obtain approval, save mutation intent, apply only the approved plan and observe the actual game process rather than the launcher alone.
4. Monitor with bounded overhead. New or changed applications require policy re-evaluation, not a kill-everything loop.
5. When the game exits, or when the user chooses Restore, stop applying policies, restore eligible settings and optionally reopen applications under a separate restart permission.
6. Show settings restored, applications reopened, actions skipped, unresolved conflicts and unsupported capabilities separately.

Game launch from Steam or another launcher may trigger a saved profile only after the user opts into automatic activation. First-run review is mandatory. Alt-Tab, minimized windows and temporary loss of foreground focus are not game exit.

## Consent contract

Every preview entry identifies the application, verified process group, proposed action, reason, observed GPU/other resource evidence, possible interruption and recovery category.

Consent is scoped to one session by default. A saved per-game/per-application rule is separately opt-in, revocable, versioned and invalidated by relevant identity or action changes. It is not blanket permission to terminate new processes with the same filename.

| Action | Consent and outcome |
| --- | --- |
| Keep/protect | No mutation. Protection takes precedence over other rules. |
| Lower approved scheduling policy | Capture the original readable value first; restore that value if ownership checks still hold. |
| Pause supported workload | Requires a tested application adapter and readable prior state; resume only work we paused. |
| Gracefully close application | Explicit approval. A refusal, save dialog or timeout does not authorize force termination. |
| Force terminate application | Separate explicit confirmation for the currently verified target set; warn that unsaved work may be lost. Disabled by default and not silently persisted. |
| Reopen after the game | Separate opt-in. A new process is launched; unsaved documents, runtime state and GPU allocations are not restored by this product. |

The product cannot generally determine whether arbitrary applications have unsaved work. Unknown save state must be shown as unknown. A Start button never constitutes consent to an unlisted force termination.

## Protected functionality

Protect the game and required launcher/authentication/anti-cheat processes, Windows session and graphics infrastructure, networking, Bluetooth, audio, input, security, accessibility and essential thermal/device control. User-marked protected apps, voice chat, streaming and assistive tools remain available unless the user deliberately changes their non-system application policy.

Examples of optional candidates, not a kill list: animated wallpapers, inactive browsers, optional capture/overlay utilities, paused development tools and local GPU compute workloads. Their names or vendor signatures do not prove they are safe to close. Active capture, voice or remote-play roles change the decision.

Do not terminate a launcher helper just because it uses GPU memory. Preserve game launch, authentication, controller routing, cloud saves and required UI dependencies. Unknown dependency means skip and explain.

## Native settings and UI

Keep the primary screen to: selected game, GPU/resource overview, action preview, Start and Restore. Advanced settings expose protected applications, per-app actions, restart consent, tested policy switches and diagnostic capture.

The resource view labels adapter and engine; it separates GPU processing, dedicated memory and shared memory. Measured changes must not be presented as guaranteed recoverable VRAM or predicted FPS.

While gaming, the UI sleeps or closes to the tray. No always-on animations, web runtime, compulsory overlay or persistent optimizer-created rendering workload. Accessibility, high-DPI support, keyboard navigation and visible recovery status are release requirements.

The session controller is independent of the window. Closing the window must not lose recovery records. Behavior if both controller and UI die is specified in the recovery document, not hidden behind an unconditional restoration promise.

## Non-goals and boundaries

No CPU/GPU overclocking, undervolting, fan tuning, firmware control, permanent service disabling, registry debloat, security-feature disabling, kernel patching, game DLL injection or anti-cheat bypass.

No whole-machine snapshot, disk rollback or reboot into a stripped Windows shell in the first product. No clearing shader caches, disabling the pagefile, indiscriminate working-set trimming, forced DWM termination or display-driver reset as a performance feature.

No universal zero-GPU policy for every non-game process, no GPU hard-quota claim, no guaranteed fixed amount of recovered memory and no fixed FPS uplift claim. Required graphics work remains exempt.

No account, cloud backend or LLM is required for the local optimizer. Runtime inventories, consent and recovery logs stay local by default; exported diagnostics require deliberate user action and redaction.
