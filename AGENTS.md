# Repository Guidelines

## Project Structure & Module Organization
This repository is the Rust rewrite of the legacy `VolumeBackup` and `Win32VSSWrapper` C++ projects. The crate is structured as a library-first layout with domain modules under `src/` and platform backends under `src/platform/`. CLI tools live in `src/bin/`.

The public API is organized around cross-platform traits:
- `Backend` — common interface (`backend_name()`, `capabilities()`, `supports()`)
- `SnapshotProvider` — create, list, delete snapshots
- `BackupExecutor` — execute backup plans (stream-based or block-level)
- `RestorePlanner` — execute restore plans
- `MountManager` — mount/unmount snapshots (future)

Platform adapters implement these traits behind compile-time dispatch (`cfg(target_os)`).

## Architecture Notes
Snapshot support is backend-specific. Windows targets VSS requestor/provider flows with CLI-primary and COM-fallback paths. Linux uses pluggable backends (Btrfs, LVM, ZFS) selected at runtime. macOS and generic Unix are stubbed for future APFS and other implementations.

The `StubBackend` base provides default `UnsupportedOperation` for all trait methods — concrete backends override only what they implement. Capability discovery is explicit so unsupported operations fail clearly instead of degrading silently.

Each backend uses a plan-then-execute pattern: plan structs (e.g., `BtrfsSendPlan`, `LvmBackupPlan`) capture commands before execution, enabling unit testing without privileged operations.

## Build, Test, and Development Commands
- `cargo build` builds the library and CLI binaries.
- `cargo test` runs unit tests and doctests.
- `cargo fmt --all` formats the workspace.
- `cargo clippy --all-targets -D warnings` enforces lint-clean code (without `--all-features` on Linux, since `windows-vss` is Windows-only).
- `cargo run --bin vptcli` runs the CLI.

Run commands from the repository root. For snapshot, mount, or raw-volume tests, expect elevated privileges and platform-specific fixtures.

## Coding Style & Naming Conventions
Use idiomatic Rust: 4-space indentation, `snake_case` modules/functions, `PascalCase` types, and `SCREAMING_SNAKE_CASE` constants. Prefer traits plus small structs over inheritance-style designs. Model fallible OS operations with typed errors (`thiserror` is preferred) and avoid panics in library code.

Keep unsafe code isolated, documented (`// SAFETY:` comments), and covered by tests. Any FFI boundary should live in a narrow module with safe wrappers.

## Testing Guidelines
Keep unit tests next to modules and cross-module or privileged scenarios in `tests/`. Name tests by behavior, for example `creates_read_only_snapshot_on_supported_backend`. Mock provider traits where possible; gate destructive or privilege-requiring cases behind integration tests.

Integration tests are Python-based under `tests/`:
- `test_smoke.py` — CLI smoke tests (no root required)
- `test_btrfs.py`, `test_lvm.py`, `test_zfs.py` — provider roundtrip tests (root required)
- `test_vss.py` — Windows VSS roundtrip test (admin required)
- `run_all.py` — test runner with `--providers`, `--build`, `--timeout`, `--keep` flags

## Commit & Pull Request Guidelines
Use short imperative commits such as `Add snapshot provider trait` or `Implement Windows VSS adapter skeleton`. PRs should describe platform impact, required privileges, test coverage, and any changes to on-disk formats or restore semantics.
