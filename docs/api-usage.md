# API Usage

## Backup Planning
The shared model now distinguishes live volumes from explicit snapshots.

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
};
```

## Restore Planning
Restore plans remain target-oriented and can optionally carry a base snapshot reference for future incremental restore work.

```rust
use std::path::PathBuf;
use vpt_rs::{BackupTarget, RestorePlan, SnapshotRef, VolumeRef};

let plan = RestorePlan {
    source: BackupTarget::ImageFile(PathBuf::from("/tmp/backup.stream")),
    destination: VolumeRef::new("tank/restore"),
    force: true,
    base_snapshot: Some(SnapshotRef::new("tank/restore@base")),
};
```

## Snapshot References
Use `SnapshotRef` when the backup source is already a snapshot or when an incremental-capable provider needs a parent snapshot.

Use `SnapshotHandle` for snapshots returned directly by a provider operation such as `create_snapshot`.
