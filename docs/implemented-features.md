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

LVM and ZFS currently expose capability metadata and selection paths, but still return stubbed operational errors.

## Implemented Backup And Restore Features
`vb-backup` and `vb-restore` are available as demo CLIs.

The Btrfs provider currently implements:

- backup planning and execution through `btrfs send`
- restore planning and execution through `btrfs receive`
- file-based stream export/import using `BackupTarget::ImageFile`

Incremental parent/base snapshot streams are not implemented yet.

## Validation And Testing
The project currently has unit tests for:

- platform/backend descriptor behavior
- snapshot kind parsing
- Linux provider registry behavior
- Btrfs snapshot planning and output parsing
- Btrfs send/receive planning

Validated commands so far:

- `cargo fmt`
- `cargo test`
- `cargo run --bin vb-snapshot -- backend list`
- `cargo run --bin vb-snapshot -- capabilities --provider btrfs`
- `cargo run --bin vb-backup -- --help`
- `cargo run --bin vb-restore -- --help`
