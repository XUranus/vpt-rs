# Repository Guidelines

## Project Structure & Module Organization
This repository is the Rust rewrite of the legacy `VolumeBackup` and `Win32VSSWrapper` C++ projects. The crate currently starts at `src/lib.rs`; grow it into a library-first layout with domain modules such as `src/backup/`, `src/restore/`, `src/mount/`, and `src/snapshot/`. Put OS backends under `src/platform/{windows,linux,macos,unix}/` and keep CLI demos in `src/bin/`.

Design the public API around cross-platform traits, then implement platform adapters behind them. Example split: `SnapshotProvider`, `BlockDeviceCopier`, and `MountManager`.

## Architecture Notes
Snapshot support is backend-specific. Windows should target VSS requestor/provider flows; macOS should treat APFS snapshots as the primary native snapshot primitive; Linux/Unix should use pluggable backends such as LVM, Btrfs, or ZFS rather than assuming one universal snapshot API. Keep capability discovery explicit so unsupported operations fail clearly instead of degrading silently.

## Build, Test, and Development Commands
- `cargo build` builds the library and CLI binaries.
- `cargo test` runs unit tests and doctests.
- `cargo fmt --all` formats the workspace.
- `cargo clippy --all-targets --all-features -D warnings` enforces lint-clean code.
- `cargo run --bin <tool>` runs a demo CLI once binaries exist.

Run commands from the repository root. For snapshot, mount, or raw-volume tests, expect elevated privileges and platform-specific fixtures.

## Coding Style & Naming Conventions
Use idiomatic Rust: 4-space indentation, `snake_case` modules/functions, `PascalCase` types, and `SCREAMING_SNAKE_CASE` constants. Prefer traits plus small structs over inheritance-style designs. Model fallible OS operations with typed errors (`thiserror` is preferred) and avoid panics in library code.

Keep unsafe code isolated, documented, and covered by tests. Any FFI boundary should live in a narrow module with safe wrappers.

## Testing Guidelines
Keep unit tests next to modules and cross-module or privileged scenarios in `tests/`. Name tests by behavior, for example `creates_read_only_snapshot_on_supported_backend`. Mock provider traits where possible; gate destructive or privilege-requiring cases behind ignored integration tests.

## Commit & Pull Request Guidelines
This repository has no established git history yet, so use short imperative commits such as `Add snapshot provider trait` or `Implement Windows VSS adapter skeleton`. PRs should describe platform impact, required privileges, test coverage, and any changes to on-disk formats or restore semantics.
