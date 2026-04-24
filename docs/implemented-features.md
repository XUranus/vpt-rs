# Implemented Features

## Current Scope
The crate is now structured as a library-first volume backup project with platform backends and demo CLI tools. Public traits are exposed for:

- snapshot creation and enumeration
- backup/export flows
- restore/import flows
- snapshot mounting (still mostly stubbed)

The core shared surface lives in `src/lib.rs` and `src/types.rs`.

## Platform Architecture
The current platform layer exposes:

- backend descriptors and capability reporting
- platform-specific backend selection
- Linux provider selection by name (`btrfs`, `lvm`, `zfs`)
- a Windows VSS-oriented module tree prepared for future FFI work

The shared backup model now distinguishes:

- live volume sources
- explicit snapshot sources
- temporary snapshot policy for backup flows
- optional parent snapshot references for incremental-capable providers

## Implemented Snapshot Features
`vb-snapshot` is available as a demo CLI with:

- `backend` and `backend list`
- `capabilities`
- `create`
- `list`

The Btrfs provider is the first provider with real snapshot logic. It currently implements:

- request validation
- snapshot path derivation under `.vb-snapshots/`
- `btrfs subvolume snapshot`
- `btrfs subvolume list -s`
- `btrfs subvolume delete`

The LVM provider now implements snapshot planning and execution through the LVM CLI for:

- `lvcreate --snapshot`
- `lvchange --permission r` for read-only snapshots
- `lvs`-based snapshot enumeration
- `lvremove`-based snapshot deletion
- `dd`-based image export from logical volumes
- `dd`-based image restore into logical volumes
- snapshot mount/unmount through the Linux mount tools

The ZFS provider now implements snapshot planning and execution through the ZFS CLI for:

- `zfs snapshot`
- `zfs list -t snapshot`
- `zfs destroy`
- `zfs send`
- `zfs receive`

The current ZFS backup/restore flow is file-based and intentionally narrow:

- backup requires an explicit snapshot source such as `pool/fs@snap`
- restore expects a dataset destination such as `pool/restore`

Mount-oriented ZFS workflows remain stubbed.

## Implemented Backup And Restore Features
`vb-backup` and `vb-restore` are available as demo CLIs.

The Btrfs provider currently implements:

- backup planning and execution through `btrfs send`
- restore planning and execution through `btrfs receive`
- file-based stream export/import using `BackupTarget::ImageFile`
- optional temporary snapshot creation for backup flows
- optional parent snapshot references for incremental send planning

The ZFS provider currently implements:

- backup planning and execution through `zfs send`
- restore planning and execution through `zfs receive`
- explicit snapshot-source backup flows
- optional parent snapshot references for incremental send planning

The current CLI surface includes:

- `vb-backup --snapshot-source`
- `vb-backup --parent-snapshot <id>`
- `vb-backup --snapshot-kind crash|application`
- `vb-backup --snapshot-label <name>`
- `vb-backup --snapshot-read-write`
- `vb-restore --base-snapshot <id>`
- `vb-restore --force` for destructive block-level restore backends such as LVM
- `vb-mount mount|unmount` for snapshot browsing flows

## Validation And Testing
The project currently has unit tests for:

- platform/backend descriptor behavior
- snapshot kind parsing
- Linux provider registry behavior
- Btrfs snapshot planning and output parsing
- Btrfs send/receive planning
- LVM backup/restore planning
- LVM mount/unmount planning

Validated commands so far:

- `cargo fmt`
- `cargo test`
- `cargo run --bin vb-snapshot -- backend list`
- `cargo run --bin vb-snapshot -- capabilities --provider btrfs`
- `cargo run --bin vb-backup -- --help`
- `cargo run --bin vb-restore -- --help`
- `cargo run --bin vb-mount -- --help`
- `sudo bash scripts/integration/lvm-snapshot.sh`
