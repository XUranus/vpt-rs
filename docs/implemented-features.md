# Implemented Features

## Current Scope

The crate is structured as a library-first volume backup project with platform backends and a CLI tool. The public API is organized around:

- **`Backend`** trait for capability discovery
- **`SnapshotProvider`** for snapshot lifecycle operations
- **`BackupExecutor`** for backup/export flows (stream-based or block-level)
- **`RestorePlanner`** for restore/import flows
- **`MountManager`** for future mount/unmount workflows

The core shared surface lives in `src/lib.rs`, `src/backend.rs`, and `src/types.rs`.

## Platform Architecture

The platform layer exposes:

- backend descriptors and capability reporting
- platform-specific backend selection
- Linux provider selection by name (`btrfs`, `lvm`, `zfs`)
- a Windows VSS module with CLI-primary and COM-fallback paths

The shared backup model distinguishes:

- live volume sources
- explicit snapshot sources
- temporary snapshot policy for backup flows
- optional parent snapshot references for incremental-capable providers

## Implemented Snapshot Features

`vptcli snapshot` is available with:

- `backend` and `backend list`
- `capabilities`
- `create`
- `list`
- `delete`

### Btrfs

- request validation (rejects application-consistent)
- snapshot path derivation under `.vb-snapshots/`
- `btrfs subvolume snapshot [-r]`
- `btrfs subvolume list -s`
- `btrfs subvolume delete`

### LVM

- `lvcreate --snapshot --extents 20%ORIGIN`
- `lvchange --permission r` for read-only snapshots
- `lvs`-based snapshot enumeration (filtered by origin)
- `lvremove`-based snapshot deletion

### ZFS

- `zfs snapshot [-r]`
- `zfs list -t snapshot -H -o name,mountpoint`
- `zfs destroy`

### Windows VSS

- `wmic shadowcopy call create` for snapshot creation
- `wmic shadowcopy where ID get DeviceObject` for device path retrieval
- `vssadmin list shadows` for listing (locale-independent parsing)
- `wmic shadowcopy delete` / `vssadmin delete shadows` for deletion
- `IVssCoordinator::DeleteSnapshots` COM fallback for deletion
- GUID format validation for snapshot IDs (injection prevention)

## Implemented Backup And Restore Features

`vptcli backup` and `vptcli restore` are available with:

- `--provider <name>` for backend selection
- `--snapshot-source` for explicit snapshot sources
- `--parent-snapshot <id>` for incremental backups
- `--snapshot-kind crash|application`
- `--snapshot-label <name>`
- `--snapshot-read-write`
- `--no-snapshot`
- `--block-size <N[K|M|G]>` for I/O chunk size
- `--force` for destructive restore backends
- `--base-snapshot <id>` for incremental restore

### Btrfs

- backup via `btrfs send [-p parent]` with stdout to stream file
- restore via `btrfs receive` with stdin from stream file
- temporary snapshot creation and cleanup for backup flows
- incremental send with parent snapshot reference

### LVM

- backup via block-level copy (`copy_blocks`, default 4 MiB) from LV to image file
- restore via block-level copy from image file to LV (requires `--force`)
- temporary snapshot creation and cleanup for backup flows

### ZFS

- backup via `zfs send [-i parent]` with stdout to stream file
- restore via `zfs receive [-F]` with stdin from stream file
- requires explicit snapshot source or temporary snapshot policy
- incremental send with parent snapshot reference

### Windows VSS

- backup via VSS snapshot + block-level copy to image file
- fallback to direct volume copy when VSS is unavailable (e.g. VHD volumes)
- restore via block-level copy from image file to volume (requires `--force`)

## Unit Test Coverage

Unit tests cover:

- platform/backend descriptor behavior
- snapshot kind parsing
- Linux provider registry behavior
- Btrfs snapshot planning, output parsing, send/receive planning
- LVM volume parsing, snapshot planning, backup/restore planning
- ZFS dataset parsing, snapshot planning, send/receive planning
- block-level copy (file contents, empty file, zero block size rejection)
- command timeout behavior
- VSS CLI parser (GUID extraction, wmic field parsing, vssadmin output parsing, volume matching)
- VSS COM helpers (GUID parsing, volume path normalization, wide string conversion)
