# vpt-rs

`vpt-rs` is a Rust rewrite of the existing `VolumeBackup` and `Win32VSSWrapper` C++ projects. The goal is a modern, library-first volume backup system that combines snapshot creation, backup/restore, and snapshot mount workflows behind a cross-platform API.

## Current Status

The crate already exposes:

- snapshot lifecycle traits and provider selection
- backup and restore traits for file-backed export/import flows
- mount traits for snapshot browsing workflows
- demo CLIs: `vb-snapshot`, `vb-backup`, `vb-restore`, `vb-mount`

Linux providers currently implemented:

- `btrfs`: snapshot create/list/delete, `send` backup, `receive` restore, snapshot mount/unmount
- `lvm`: snapshot create/list/delete, image backup/restore, snapshot mount/unmount
- `zfs`: snapshot create/list/delete, `send` backup, `receive` restore

Windows VSS scaffolding exists, but the real COM-backed implementation is still pending.

## Project Layout

- `src/lib.rs`: public crate surface
- `src/types.rs`: shared domain model
- `src/platform/`: platform backends
- `src/bin/`: demo CLIs
- `scripts/integration/`: privileged filesystem-backed integration scripts
- `docs/`: provider status, API usage, and integration notes

## Build And Test

```bash
cargo build
cargo test
cargo build --release
```

Provider-specific demos:

```bash
./target/release/vb-snapshot backend list
./target/release/vb-backup --provider btrfs --output /tmp/backup.stream /path/to/subvol
./target/release/vb-mount mount --provider btrfs /path/to/.vb-snapshots/snap0
./target/release/vb-restore --provider lvm --force --input /tmp/volume.img /dev/vg0/restore
./target/release/vb-mount mount --provider lvm /dev/vg0/snap0
```

## Integration Scripts

On Linux, privileged integration checks are available under `scripts/integration/`.

```bash
sudo IMAGE_DIR=/opt/volumeset COPY_DIR=/opt/volumeset/copy MOUNT_ROOT=/mnt/volmnt \
  bash scripts/integration/run-all.sh
```

Useful flags:

- `ASSERT_RESTORE_CONTENTS=0`
- `ASSERT_SNAPSHOT_CLEANUP=0`

## Roadmap

Near-term priorities:

- real Windows VSS bindings
- richer mount workflows
- incremental send support for snapshot-capable providers
- broader cross-platform snapshot abstractions for macOS and generic Unix
