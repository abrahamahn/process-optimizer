# Process Optimizer

A native Windows game-session optimizer that reduces competing background GPU work and resource pressure, with explicit user consent and recoverable settings changes.

**Status: specification only. No optimizer implementation, executable, or performance claim exists yet.**

## Product direction

GPU contention is the first priority. Identify which applications use the game's GPU; close user-approved unnecessary applications, pause supported background GPU workloads, and evaluate reversible scheduling policies for applications that must remain open. CPU, memory, and I/O management support this goal rather than replace it.

Application shutdown is a core feature, not an excluded operation. Graceful closure and force termination have separate consent. Reopening an application is not restoration of its unsaved work. A session snapshot records our changes; it is not a Windows restore point or a whole-machine checkpoint.

Network, Bluetooth, audio, input, display, security, game authentication, anti-cheat, and user-protected applications must remain functional. No overclocking, permanent debloating, security disabling, or promise of exclusive GPU ownership.

## Specifications

| Document | Authority |
| --- | --- |
| [Product](specs/01-product.md) | Scope, priorities, consent, native UI, non-goals |
| [GPU and process policy](specs/02-gpu-and-process-policy.md) | GPU attribution, close/pause policies, low-level API feasibility and boundaries |
| [Session recovery and architecture](specs/03-session-recovery-and-architecture.md) | Journal, failure handling, process identity, recovery, native components |
| [Validation and delivery](specs/04-validation-and-delivery.md) | Acceptance scenarios, measurement, implementation slices and open evidence |

Start with the product spec. Windows API statements are linked to primary documentation in the relevant specification. Capability availability and actual performance effects still require tests on Windows hardware.

## Implementation direction

Rust with native Win32 UI and Windows API bindings. Keep the policy engine testable without Windows and isolate platform calls behind narrow adapters. No Electron, WebView runtime, in-game injection, or always-on LLM is planned.

The first implementation slice must include GPU visibility and a consented application-close path, together with the recovery journal and native controls; CPU-only optimization is not sufficient.
