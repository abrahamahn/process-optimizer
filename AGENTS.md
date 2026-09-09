# Repository instructions

Read `README.md` and the relevant owner specification before implementation. The native implementation is an alpha; distinguish written code, passing Windows integration tests and real-hardware performance evidence.

## Authority

- `specs/01-product.md`: product requirements, priority, consent and UI.
- `specs/02-gpu-and-process-policy.md`: resource policies and Windows API boundaries.
- `specs/03-session-recovery-and-architecture.md`: state, identity, journal and recovery.
- `specs/04-validation-and-delivery.md`: acceptance tests, measurement and delivery gates.

Edit the owner document instead of duplicating its contract in a new roadmap or summary. Keep one implementation repository; do not introduce unrelated application starters or projects.

## Implementation rules

GPU contention reduction is the primary outcome. Consented application shutdown is a required feature. Do not substitute a CPU-priority utility for the specified product.

Use Rust and native Windows UI/API calls; keep unsafe FFI narrow and reviewed. A documented API is not evidence that a particular driver, OS build or process permits it. Read, apply, verify and restore must be tested before enabling a reversible policy.

Preserve protected functionality. No overclocking, security disabling, kernel patching, anti-cheat bypass, game injection, arbitrary shell execution, blanket process killing or undocumented registry tweaks.

Persist recovery intent before mutations. Process identity is not just a PID or executable name. Closing and reopening an app is not restoring its unsaved state. Never claim a GPU hard quota or guaranteed exclusive GPU access without a separately verified mechanism. Recovery never replays a close operation.

Tests distinguish pure policy tests, Windows integration tests, recovery fault injection and real-hardware performance measurements. Never run destructive tests against arbitrary user applications. Mutation tests own fixture process handles; force tests require explicit opt-in.

Run `cargo test --lib` cross-platform and `cargo test --all-targets` on Windows. Build the native application and run read-only/UI smoke probes before packaging. Do not equate a hosted CI runner with a physical gaming GPU.

Read the current remote HEAD before editing; preserve unrelated work and do not force-push. Do not publish machine inventories, raw traces, local paths, process command lines, credentials or user documents to this public repository.
