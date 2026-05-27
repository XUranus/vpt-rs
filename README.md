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
  --snapshot-read-write>        Make snapshot read-write (default is read-only)
  --no-snapshot                 Skip snapshot creation
```

### `vptcli restore`

Restore a volume from a stream or image file.

```
vptcli restore <destination> --input <file> [options]

Options:
  --provider <name>             Select backend (btrfs, lvm, zfs)
  --force                       Force destructive restore (e.g. LVM dd overwrite)
  --base-snapshot <id>          Base snapshot for incremental restore
```

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| Linux (btrfs) | Implemented | `btrfs send`/`btrfs receive` |
| Linux (LVM) | Implemented | `dd` block-level copy |
| Linux (ZFS) | Implemented | `zfs send`/`zfs receive` |
| macOS | Stubbed | Architecture prepared for APFS |
| Windows | Stubbed | VSS module scaffolded (feature: `windows-vss`) |

## Library Usage

```rust
use vpt_rs::platform;
use vpt_rs::{SnapshotProvider, BackupPlan, BackupSource, BackupTarget, VolumeRef};

// Get the current platform backend
let backend = platform::current_backend();

// List snapshots
let snapshots = backend.list_snapshots(&VolumeRef::new("/mnt/data"))?;
```

### Core Traits

| Trait | Purpose |
|-------|---------|
| `SnapshotProvider` | Create, list, delete snapshots |
| `BlockDeviceCopier` | Backup and restore volumes |
| `RestorePlanner` | Plan restore operations |
| `MountManager` | Mount/unmount snapshots and volumes |

### Key Types

`VolumeRef`, `SnapshotRef`, `SnapshotHandle`, `SnapshotInfo`, `SnapshotRequest`,
`SnapshotKind`, `SnapshotPolicy`, `BackupPlan`, `BackupSource`, `BackupTarget`,
`RestorePlan`, `Capability`, `BackendDescriptor`

## Project Structure

```
src/
  lib.rs              Public API re-exports
  types.rs            Shared types (VolumeRef, BackupPlan, etc.)
  snapshot.rs         SnapshotProvider trait
  backup.rs           BlockDeviceCopier trait
  restore.rs          RestorePlanner trait
  mount.rs            MountManager trait
  error.rs            Error types
  logging.rs          Tracing initialization
  process.rs          External command helpers
  bin/
    vptcli.rs         CLI binary
  platform/
    mod.rs            Platform abstraction layer
    linux/
      mod.rs          Linux backend selector (btrfs/lvm/zfs)
      btrfs.rs        Btrfs implementation
      lvm.rs          LVM implementation
      zfs.rs          ZFS implementation
    windows.rs        Windows stub + VSS scaffold
    macos.rs          macOS stub
    unix.rs           Generic Unix stub
tests/
  env.py              Test infrastructure
  test_btrfs.py       Btrfs roundtrip integration test
  test_lvm.py         LVM roundtrip integration test
  test_zfs.py         ZFS roundtrip integration test
  test_smoke.py       CLI smoke tests
  run_all.py          Test runner
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
