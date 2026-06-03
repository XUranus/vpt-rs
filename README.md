# vpt-rs

Cross-platform volume backup library and CLI tool written in Rust. Provides a unified trait-based architecture for snapshot creation, block-level backup, and restore across multiple storage backends.

## Quick Start

```bash
cargo build --release

# List available backends
./target/release/vptcli snapshot backend list

# Create a snapshot
vptcli snapshot create --provider btrfs /mnt/data/my-volume

# Back up to a stream file
vptcli backup --provider btrfs --output backup.stream /mnt/data/my-volume

# Restore from a stream file
vptcli restore --provider btrfs --input backup.stream /mnt/data/restore-dir
```

## Installation

```bash
cargo install --path .
```

Or build manually:

```bash
cargo build --release
# Binary at target/release/vptcli
```

## CLI Reference

### `vptcli snapshot`

Manage snapshots and query backend capabilities.

```
vptcli snapshot backend                              # Show current backend
vptcli snapshot backend list                         # List all available backends
vptcli snapshot capabilities --provider <name>       # List backend capabilities
vptcli snapshot create [--provider <name>] <volume> [--label <name>] [--kind crash|application] [--read-write]
vptcli snapshot list [--provider <name>] <volume>
vptcli snapshot delete [--provider <name>] <snapshot-id>
```

### `vptcli backup`

Back up a volume to a stream or image file.

```
vptcli backup <source> --output <file> [options]

Options:
  --provider <name>             Select backend (btrfs, lvm, zfs)
  --snapshot-source             Treat source as an existing snapshot
  --parent-snapshot <id>        Parent snapshot for incremental backup
  --snapshot-kind <type>        Snapshot consistency: crash (default) or application
  --snapshot-label <name>       Label for the snapshot
  --snapshot-read-write         Make snapshot read-write (default is read-only)
  --no-snapshot                 Skip snapshot creation
  --block-size <N[K|M|G]>      I/O block size (default: 4M)
```

### `vptcli restore`

Restore a volume from a stream or image file.

```
vptcli restore <destination> --input <file> [options]

Options:
  --provider <name>             Select backend (btrfs, lvm, zfs)
  --force                       Force destructive restore (e.g. LVM or VSS overwrite)
  --base-snapshot <id>          Base snapshot for incremental restore
  --block-size <N[K|M|G]>      I/O block size (default: 4M)
```

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| Linux (btrfs) | Implemented | `btrfs send`/`btrfs receive`, incremental send |
| Linux (LVM) | Implemented | Block-level copy via `copy_blocks` |
| Linux (ZFS) | Implemented | `zfs send`/`zfs receive`, incremental send |
| Windows | Implemented | VSS snapshots via CLI (wmic/vssadmin), COM fallback for delete |
| macOS | Stubbed | Architecture prepared for APFS |
| Generic Unix | Stubbed | Architecture prepared for future backends |

## Library Usage

```rust
use vpt_rs::platform;
use vpt_rs::{Backend, SnapshotProvider, BackupPlan, BackupSource, BackupTarget, VolumeRef};

// Get the current platform backend
let backend = platform::current_backend();

// Query capabilities
println!("backend: {}", backend.backend_name());
assert!(backend.supports(vpt_rs::Capability::CrashConsistentSnapshot));

// List snapshots
let snapshots = backend.list_snapshots(&VolumeRef::new("/mnt/data"))?;
```

### Core Traits

| Trait | Purpose |
|-------|---------|
| `Backend` | Common interface: `backend_name()`, `capabilities()`, `supports()` |
| `SnapshotProvider` | Create, list, delete snapshots |
| `BackupExecutor` | Execute backup plans (stream-based or block-level) |
| `RestorePlanner` | Execute restore plans |
| `MountManager` | Mount/unmount snapshots (future) |

### Key Types

`VolumeRef`, `SnapshotRef`, `SnapshotHandle`, `SnapshotInfo`, `SnapshotRequest`,
`SnapshotKind`, `SnapshotPolicy`, `BackupPlan`, `BackupSource`, `BackupTarget`,
`RestorePlan`, `Capability`, `BackendDescriptor`

## Project Structure

```
src/
  lib.rs              Public API re-exports
  backend.rs          Backend supertrait
  types.rs            Shared types (VolumeRef, BackupPlan, etc.)
  snapshot.rs         SnapshotProvider trait
  backup.rs           BackupExecutor trait
  restore.rs          RestorePlanner trait
  mount.rs            MountManager trait
  error.rs            Error types (thiserror)
  copy.rs             Block-level copy utility
  logging.rs          Tracing initialization
  process.rs          External command helpers (timeout, I/O redirection)
  bin/
    vptcli.rs         CLI binary
  platform/
    mod.rs            Platform abstraction + StubBackend
    linux/
      mod.rs          Linux backend selector (btrfs/lvm/zfs)
      btrfs.rs        Btrfs implementation (send/receive)
      lvm.rs          LVM implementation (block-level copy)
      zfs.rs          ZFS implementation (send/receive)
    windows/
      vss.rs          VSS snapshot provider orchestration
      vss/
        ffi.rs        FFI routing (CLI primary, COM fallback)
        ffi/cli.rs    wmic/vssadmin CLI wrappers
        ffi/com.rs    Native COM API (raw vtable FFI)
        requestor.rs  VSS requestor (init, sessions)
        session.rs    VSS session (commit/abort)
    windows.rs        Windows backend (feature-gated VSS)
    macos.rs          macOS stub (macos-apfs)
    unix.rs           Generic Unix stub
tests/
  env.py              Test infrastructure (UUID isolation, CLI wrappers)
  test_smoke.py       CLI smoke tests (no root required)
  test_btrfs.py       Btrfs roundtrip integration test
  test_lvm.py         LVM roundtrip integration test
  test_zfs.py         ZFS roundtrip integration test
  test_vss.py         Windows VSS roundtrip integration test
  run_all.py          Test runner with per-provider selection
  README.md           Test documentation
```

## Integration Tests

Python-based tests that exercise the full lifecycle: volume init, mount, write data, snapshot create/list, backup, restore, verify files, snapshot delete, teardown.

```bash
# Build first
cargo build --release

# Run all tests (requires root for provider tests)
sudo python3 tests/run_all.py

# Run specific providers
sudo python3 tests/run_all.py --providers btrfs,lvm

# Smoke tests (no root required)
python3 tests/test_smoke.py
```

See [tests/README.md](tests/README.md) for full documentation.

## Logging

The CLI uses Rust `tracing` with `EnvFilter`. Set the `RUST_LOG` environment variable to control verbosity:

```bash
RUST_LOG=vpt_rs=debug vptcli snapshot backend list
RUST_LOG=trace vptcli backup --provider btrfs --output out.stream /mnt/data
```

## License

TBD
