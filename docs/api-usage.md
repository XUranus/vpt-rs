# API Usage

## Backend Discovery

All backends implement the `Backend` trait, which provides capability querying:

```rust
use vpt_rs::{Backend, Capability};
use vpt_rs::platform;

let backend = platform::current_backend();
println!("backend: {}", backend.backend_name());
assert!(backend.supports(Capability::CrashConsistentSnapshot));

// List all available backends (Linux has btrfs, lvm, zfs)
for descriptor in platform::available_backend_descriptors() {
    println!("{}: {} capabilities", descriptor.backend_name, descriptor.capabilities.len());
}
```

## Backup Planning

The shared model distinguishes live volumes from explicit snapshots.

Example: backup a live Btrfs subvolume using a temporary read-only snapshot.

```rust
use std::path::PathBuf;
use vpt_rs::{BackupPlan, BackupSource, BackupTarget, SnapshotKind, SnapshotPolicy, VolumeRef};

let plan = BackupPlan {
    source: BackupSource::Volume(VolumeRef::new("/mnt/data/subvol")),
    target: BackupTarget::ImageFile(PathBuf::from("/tmp/backup.stream")),
    snapshot_policy: SnapshotPolicy::temporary(
        SnapshotKind::CrashConsistent,
        Some("nightly".to_string()),
        true,
    ),
    parent_snapshot: None,
    block_size: None,  // use provider default (4 MiB)
};
```

Example: incremental ZFS send from an explicit snapshot with a parent snapshot.

```rust
use std::path::PathBuf;
use vpt_rs::{BackupPlan, BackupSource, BackupTarget, SnapshotPolicy, SnapshotRef};

let plan = BackupPlan {
    source: BackupSource::Snapshot(SnapshotRef::new("tank/data@snap2")),
    target: BackupTarget::ImageFile(PathBuf::from("/tmp/incremental.zfs")),
    snapshot_policy: SnapshotPolicy::disabled(),
    parent_snapshot: Some(SnapshotRef::new("tank/data@snap1")),
    block_size: None,
};
```

## Restore Planning

Restore plans carry source, destination, and optional flags for destructive backends.

```rust
use std::path::PathBuf;
use vpt_rs::{BackupTarget, RestorePlan, SnapshotRef, VolumeRef};

let plan = RestorePlan {
    source: BackupTarget::ImageFile(PathBuf::from("/tmp/backup.stream")),
    destination: VolumeRef::new("tank/restore"),
    force: true,  // required for LVM and VSS
    base_snapshot: None,
    block_size: None,
};
```

## Executing Plans

Use the `BackupExecutor` and `RestorePlanner` traits to execute plans:

```rust
use vpt_rs::{BackupExecutor, RestorePlanner};
use vpt_rs::platform;

let backend = platform::current_backend();

// Execute a backup
backend.backup_volume(&backup_plan)?;

// Execute a restore
backend.restore_volume(&restore_plan)?;
```

## Snapshot References

Use `SnapshotRef` when the backup source is already a snapshot or when an incremental-capable provider needs a parent snapshot.

Use `SnapshotHandle` for snapshots returned directly by a provider operation such as `create_snapshot`.

```rust
use vpt_rs::{SnapshotRef, VolumeRef};

// Reference with origin (useful for ZFS dataset resolution)
let snap_ref = SnapshotRef::new("tank/data@snap1")
    .with_origin(VolumeRef::new("tank/data"));

// Simple reference (origin is optional)
let snap_ref = SnapshotRef::new("/mnt/data/.vb-snapshots/base");
```
