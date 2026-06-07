---
sidebar_position: 1
---

# Coding Style & Conventions

This page documents the coding conventions used in the vpt-rs project. Following these conventions keeps the codebase consistent and reviewable.

## Rust Style

vpt-rs follows idiomatic Rust conventions:

| Element | Convention | Example |
|---------|-----------|---------|
| Modules | `snake_case` | `src/platform/linux/btrfs.rs` |
| Functions | `snake_case` | `plan_create_snapshot()`, `copy_blocks()` |
| Types (structs, enums) | `PascalCase` | `BtrfsBackend`, `SnapshotRequest`, `BackupPlan` |
| Constants | `SCREAMING_SNAKE_CASE` | `DEFAULT_BLOCK_SIZE`, `CAPABILITIES`, `BTRFS_BIN` |
| Traits | `PascalCase` | `Backend`, `SnapshotProvider`, `BackupExecutor` |
| Enum variants | `PascalCase` | `CrashConsistent`, `ImageFile`, `Disabled` |

## Formatting

- **4-space indentation** (no tabs)
- Run `cargo fmt --all` before committing
- Run `cargo clippy --all-targets -D warnings` to catch common mistakes

:::tip
On Linux, do NOT use `--all-features` for clippy — the `windows-vss` feature requires Windows.
:::

## Error Handling

vpt-rs uses `thiserror` for typed errors. The `Error` enum in `src/error.rs` carries structured context:

```rust title="src/error.rs"
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("operation `{operation}` is not supported by backend `{backend}`")]
    UnsupportedOperation {
        operation: &'static str,
        backend: &'static str,
    },
    // ...
}
```

Guidelines:
- **No panics in library code** — always return `Result<T, Error>`
- **Use `?` for error propagation** — don't unwrap in library code
- **Carry context** — include backend name, operation, and path in errors
- **Log errors at the call site** — use `tracing::error!` before returning

## Unsafe Code

:::caution
Keep `unsafe` code isolated, documented, and covered by tests.
:::

All `unsafe` blocks must have `// SAFETY:` comments explaining why the invariants hold:

```rust
// SAFETY: COM objects with COINIT_MULTITHREADED are thread-safe.
unsafe impl Send for ComPtr {}
unsafe impl Sync for ComPtr {}
```

The VSS COM module (`src/platform/windows/vss/ffi/com.rs`) is the only file with `unsafe` code. It's isolated behind the `windows-vss` feature gate.

## Trait Design

- **All operational traits extend `Backend`** — provides `backend_name()`, `capabilities()`, `supports()`
- **Use `&'static str` for backend names** — enables zero-cost logging
- **Use `&'static [Capability]` for capability sets** — enables compile-time allocation
- **Prefer `&self` over `&mut self`** — backends are `Send + Sync` and stateless

## Testing

- **Unit tests live next to the code** — in `#[cfg(test)] mod tests` blocks
- **Name tests by behavior** — e.g. `application_consistent_requests_are_rejected`
- **Test plan generation, not execution** — plans can be tested without privileged operations
- **Integration tests are Python-based** — in `tests/` directory, require root

## Commits

Use short imperative commits:

```
Add snapshot provider trait
Implement Windows VSS adapter skeleton
Fix LVM snapshot cleanup after backup failure
```

PRs should describe:
- Platform impact (which backends are affected)
- Required privileges (root, admin, none)
- Test coverage (unit tests added? integration tests?)
- Changes to on-disk formats or restore semantics

## Project Structure

```
src/
  lib.rs              # Public API re-exports
  backend.rs          # Backend supertrait
  types.rs            # Shared domain types
  error.rs            # Error enum (thiserror)
  snapshot.rs         # SnapshotProvider trait
  backup.rs           # BackupExecutor trait
  restore.rs          # RestorePlanner trait
  mount.rs            # MountManager trait
  copy.rs             # Block-level copy utility
  process.rs          # External command execution
  logging.rs          # Tracing initialization
  bin/vptcli.rs       # CLI binary
  platform/
    mod.rs            # Platform abstraction + StubBackend
    linux/
      mod.rs          # LinuxBackend enum + delegate! macro
      btrfs.rs        # Btrfs provider
      lvm.rs          # LVM provider
      zfs.rs          # ZFS provider
    windows.rs        # Windows backend (feature-gated)
    windows/vss/      # VSS module tree
    macos.rs          # macOS stub
    unix.rs           # Generic Unix stub
```
