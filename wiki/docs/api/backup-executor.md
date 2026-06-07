# BackupExecutor Trait

The `BackupExecutor` trait exports a volume to a stream or image file.
Implementations may use stream-based send (Btrfs `send`, ZFS `send`) or
block-level copy (LVM `dd`-style, VSS snapshot + copy). The trait name
reflects the execution role, not the underlying mechanism.

## Trait Definition

```rust
pub trait BackupExecutor: Backend {
    /// Execute a backup according to the given plan.
    fn backup_volume(&self, plan: &BackupPlan) -> Result<()>;
}
```

## Methods

| Method           | Parameters       | Return type | Description                       |
|------------------|------------------|-------------|-----------------------------------|
| `backup_volume()`| `&BackupPlan`    | `Result<()>`| Execute the backup as planned     |

This trait has a single method. The entire backup behavior is controlled by the
`BackupPlan` struct passed to it.

## Key Types

### BackupPlan

The complete specification of a backup operation:

```rust
pub struct BackupPlan {
    pub source: BackupSource,
    pub target: BackupTarget,
    pub snapshot_policy: SnapshotPolicy,
    pub parent_snapshot: Option<SnapshotRef>,
    pub block_size: Option<usize>,
}
```

| Field              | Type                  | Description                                      |
|--------------------|-----------------------|--------------------------------------------------|
| `source`           | `BackupSource`        | What to back up (volume or snapshot)             |
| `target`           | `BackupTarget`        | Where to write the backup                        |
| `snapshot_policy`  | `SnapshotPolicy`      | Whether to create a temporary snapshot first     |
| `parent_snapshot`  | `Option<SnapshotRef>` | Parent for incremental backup (stream-based)     |
| `block_size`       | `Option<usize>`       | I/O block size in bytes (`None` = 4 MiB default) |

### BackupSource

What to back up:

```rust
pub enum BackupSource {
    Volume(VolumeRef),       // A live volume
    Snapshot(SnapshotRef),   // An existing snapshot
}
```

### BackupTarget

Where the backup goes:

```rust
pub enum BackupTarget {
    ImageFile(PathBuf),   // Write to a file
    Device(PathBuf),      // Write to a block device
}
```

### SnapshotPolicy

Controls temporary snapshot creation before backup:

```rust
pub enum SnapshotPolicy {
    Disabled,       // Use the source as-is
    Temporary {     // Create a temporary snapshot first
        kind: SnapshotKind,
        label: Option<String>,
        read_only: bool,
    },
}
```

### SnapshotRef

A reference to an existing snapshot, used for incremental backups:

```rust
pub struct SnapshotRef {
    pub id: String,                   // Snapshot identifier
    pub origin: Option<VolumeRef>,    // Source volume (if known)
}
```

### VolumeRef

A stable identifier for a volume:

```rust
pub struct VolumeRef {
    pub id: String,
}
```

## Usage Examples

### Full backup with a temporary snapshot

```rust
use vpt_rs::{
    BackupExecutor, BackupPlan, BackupSource, BackupTarget,
    SnapshotPolicy, SnapshotKind, VolumeRef,
};
use vpt_rs::platform;
use std::path::PathBuf;

fn main() -> vpt_rs::Result<()> {
    let backend = platform::current_backend();

    let plan = BackupPlan {
        source: BackupSource::Volume(VolumeRef::new("/mnt/data")),
        target: BackupTarget::ImageFile(PathBuf::from("/backup/data.img")),
        snapshot_policy: SnapshotPolicy::temporary(
            SnapshotKind::CrashConsistent,
            Some("backup".to_string()),
            true,
        ),
        parent_snapshot: None,
        block_size: None,
    };

    backend.backup_volume(&plan)?;
    println!("Backup complete.");

    Ok(())
}
```

### Backup without a snapshot

```rust
use vpt_rs::{
    BackupExecutor, BackupPlan, BackupSource, BackupTarget,
    SnapshotPolicy, VolumeRef,
};
use vpt_rs::platform;
use std::path::PathBuf;

fn main() -> vpt_rs::Result<()> {
    let backend = platform::current_backend();

    let plan = BackupPlan {
        source: BackupSource::Volume(VolumeRef::new("/mnt/data")),
        target: BackupTarget::ImageFile(PathBuf::from("/backup/data.img")),
        snapshot_policy: SnapshotPolicy::disabled(),
        parent_snapshot: None,
        block_size: None,
    };

    backend.backup_volume(&plan)?;

    Ok(())
}
```

### Incremental backup with a parent snapshot

```rust
use vpt_rs::{
    BackupExecutor, BackupPlan, BackupSource, BackupTarget,
    SnapshotPolicy, SnapshotKind, SnapshotRef, VolumeRef,
};
use vpt_rs::platform;
use std::path::PathBuf;

fn main() -> vpt_rs::Result<()> {
    let backend = platform::current_backend();

    let plan = BackupPlan {
        source: BackupSource::Volume(VolumeRef::new("/mnt/data")),
        target: BackupTarget::ImageFile(PathBuf::from("/backup/data-incr.img")),
        snapshot_policy: SnapshotPolicy::temporary(
            SnapshotKind::CrashConsistent,
            Some("incr".to_string()),
            true,
        ),
        parent_snapshot: Some(SnapshotRef::new("/mnt/data/.snapshots/snap1")),
        block_size: None,
    };

    backend.backup_volume(&plan)?;

    Ok(())
}
```

### Backup an existing snapshot directly

```rust
use vpt_rs::{
    BackupExecutor, BackupPlan, BackupSource, BackupTarget,
    SnapshotPolicy, SnapshotRef, VolumeRef,
};
use vpt_rs::platform;
use std::path::PathBuf;

fn main() -> vpt_rs::Result<()> {
    let backend = platform::current_backend();

    let snap_ref = SnapshotRef::new("/mnt/data/.snapshots/pre-upgrade")
        .with_origin(VolumeRef::new("/mnt/data"));

    let plan = BackupPlan {
        source: BackupSource::Snapshot(snap_ref),
        target: BackupTarget::ImageFile(PathBuf::from("/backup/upgrade.img")),
        snapshot_policy: SnapshotPolicy::disabled(),
        parent_snapshot: None,
        block_size: None,
    };

    backend.backup_volume(&plan)?;

    Ok(())
}
```

## Error Handling

| Error variant           | When it occurs                                          |
|-------------------------|---------------------------------------------------------|
| `UnsupportedOperation`  | Backend does not support backup                         |
| `InvalidArgument`       | Target type not supported (e.g. device target for stream-based backends) |
| `CommandFailed`         | External tool failed during backup                      |
| `Io`                    | File I/O failed during block copy                       |
| `Timeout`               | External tool exceeded the command timeout               |

:::tip
Stream-based backends (Btrfs, ZFS) write to `BackupTarget::ImageFile` via
their native `send` command. Block-level backends (LVM) copy blocks to either
target type using `dd`-style I/O at the configured block size.
:::
