# alani-platform

Architecture-specific HAL for x86_64, riscv64, timers, interrupt controllers, CPU features, and page-table primitives.

| Field | Value |
|---|---|
| Tier | MVK required |
| Owner | Platform team |
| Aliases | `alani-arch` |
| Architectural dependencies | `alani-abi` |

## Quick start

```bash
cargo fmt -- --check
cargo test --all-features
cargo test --no-default-features
cargo check --no-default-features
cargo clippy --all-features -- -D warnings
```

## API surface

This crate is a dependency-free Rust 2021 skeleton that remains `no_std` compatible when the default `std` feature is disabled. `alani-abi` is recorded in Cargo metadata as an architectural dependency; no private cross-repository modules are imported.

- `arch` defines architecture families, CPU vendors, feature masks, privilege/execution modes, CPU topology, and architecture profile validation for host simulation, x86_64, and riscv64.
- `hal` defines HAL capabilities, hardware profiles, deterministic boot phases, boot steps, MMIO region metadata, and cache-policy safety checks.
- `interrupts` defines interrupt controller descriptors, vectors, binding flags, top-half event records, and a fixed-capacity binding table with rights and audit checks.
- `timers` defines timer descriptors, capability masks, modes, configurations, lifecycle state, devices, and diagnostic snapshots.
- `paging` defines physical/virtual address wrappers, paging modes, page sizes, page flags, mapping policy, page mapping records, and a fixed-capacity page-table plan with overlap and sealing checks.

Security-sensitive paths fail closed on reserved bits, unsupported CPU features, missing authority, unsafe MMIO execution, unacknowledged interrupt events, write-execute mappings, user-device mappings, overlapping page ranges, invalid redaction, and sealed mutable state. Public records carry data classification and trace context so boot, kernel, device, and simulator layers can propagate diagnostics consistently.

Keep public API changes synchronized with `docs/repositories/alani-platform.md`, Doc 42, Doc 43, and the kernel boot/memory/device documents in `alani-spec`.
