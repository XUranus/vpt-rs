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
`vptcli snapshot` is available with:

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
`vptcli backup` and `vptcli restore` are available.

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

- `vptcli backup --snapshot-source`
- `vptcli backup --parent-snapshot <id>`
- `vptcli backup --snapshot-kind crash|application`
- `vptcli backup --snapshot-label <name>`
- `vptcli backup --snapshot-read-write`
- `vptcli restore --base-snapshot <id>`
- `vptcli restore --force` for destructive block-level restore backends such as LVM

## Validation And Testing
The project currently has unit tests for:

- platform/backend descriptor behavior
- snapshot kind parsing
- Linux provider registry behavior
- Btrfs snapshot planning and output parsing
- Btrfs send/receive planning
- LVM backup/restore planning

Validated commands so far:

- `cargo fmt`
- `cargo test`
- `cargo run --bin vptcli -- snapshot backend list`
- `cargo run --bin vptcli -- snapshot capabilities --provider btrfs`
- `cargo run --bin vptcli -- backup --help`
- `cargo run --bin vptcli -- restore --help`
- `sudo bash scripts/integration/lvm-snapshot.sh`
