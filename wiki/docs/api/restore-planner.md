# RestorePlanner Trait

The `RestorePlanner` trait restores a volume from a backup stream or image file.
Implementations may use stream-based receive (Btrfs `receive`, ZFS `receive`) or
block-level write (LVM, VSS). Destructive backends require `force: true` in the
plan.

## Trait Definition

```rust
pub trait RestorePlanner: Backend {
    /// Execute a restore according to the given plan.
    fn restore_volume(&self, plan: &RestorePlan) -> Result<()>;
}
```

## Methods

| Method            | Parameters       | Return type  | Description                        |
|-------------------|------------------|--------------|------------------------------------|
| `restore_volume()`| `&RestorePlan`   | `Result<()>` | Execute the restore as planned     |

Like `BackupExecutor`, this trait has a single method. All behavior is driven
by the `RestorePlan` struct.

## Key Types

### RestorePlan

The complete specification of a restore operation:

```rust
pub struct RestorePlan {
    pub source: BackupTarget,            // Where to read the backup from
    pub destination: VolumeRef,          // Target volume to restore into
    pub force: bool,                     // Allow destructive overwrite
    pub base_snapshot: Option<SnapshotRef>, // Base for incremental restore
    pub block_size: Option<usize>,       // I/O block size (None = 4 MiB)
}
```

| Field             | Type                  | Description                                        |
|-------------------|-----------------------|----------------------------------------------------|
| `source`          | `BackupTarget`        | Backup file or device to read from                 |
| `destination`     | `VolumeRef`           | Target volume or directory to restore into         |
| `force`           | `bool`                | Required for destructive (block-level) backends    |
| `base_snapshot`   | `Option<SnapshotRef>` | Base snapshot for incremental restore              |
| `block_size`      | `Option<usize>`       | I/O block size in bytes (`None` = 4 MiB default)   |

### BackupTarget

Where the backup data is read from:

```rust
pub enum BackupTarget {
    ImageFile(PathBuf),   // Read from a file
    Device(PathBuf),      // Read from a block device
}
```

### VolumeRef

A stable identifier for the destination volume:

```rust
pub struct VolumeRef {
    pub id: String,
}
```

### SnapshotRef

An optional reference to a base snapshot for incremental workflows:

```rust
pub struct SnapshotRef {
    pub id: String,
    pub origin: Option<VolumeRef>,
}
```

## The Force Flag

Block-level backends (LVM, VSS) overwrite the destination volume with the
backup contents. This is destructive -- all existing data on the target is lost.
To prevent accidental data loss, these backends check `force` and return an
`InvalidArgument` error if it is `false`.

Stream-based backends (Btrfs `receive`, ZFS `receive`) create new subvolumes
or datasets and do **not** require `force`.

| Backend type   | `force` required | Behavior                              |
|----------------|------------------|---------------------------------------|
| Btrfs          | No               | Creates a new subvolume from stream   |
| ZFS            | No               | Creates a new dataset from stream     |
| LVM            | Yes              | Overwrites logical volume with blocks |
| VSS (Windows)  | Yes              | Writes blocks to the target volume    |

## Usage Examples

### Basic stream-based restore (Btrfs)

```rust
use vpt_rs::{RestorePlanner, RestorePlan, BackupTarget, VolumeRef};
use vpt_rs::platform;
use std::path::PathBuf;

fn main() -> vpt_rs::Result<()> {
    let backend = platform::current_backend();

    let plan = RestorePlan {
        source: BackupTarget::ImageFile(PathBuf::from("/backup/data.img")),
        destination: VolumeRef::new("/mnt/restored"),
        force: false,
        base_snapshot: None,
        block_size: None,
    };

    backend.restore_volume(&plan)?;
    println!("Restore complete.");

    Ok(())
}
```

### Destructive restore with force (LVM)

```rust
use vpt_rs::{RestorePlanner, RestorePlan, BackupTarget, VolumeRef};
use vpt_rs::platform;
use std::path::PathBuf;

fn main() -> vpt_rs::Result<()> {
    let backend = platform::CurrentBackend::named("lvm")?;

    let plan = RestorePlan {
        source: BackupTarget::ImageFile(PathBuf::from("/backup/data.img")),
        destination: VolumeRef::new("/dev/vg0/data"),
        force: true, // Required for LVM
        base_snapshot: None,
        block_size: None,
    };

    backend.restore_volume(&plan)?;
    println!("LVM restore complete.");

    Ok(())
}
```

### Restore with a custom block size

```rust
use vpt_rs::{RestorePlanner, RestorePlan, BackupTarget, VolumeRef};
use vpt_rs::platform;
use std::path::PathBuf;

fn main() -> vpt_rs::Result<()> {
    let backend = platform::current_backend();

    let plan = RestorePlan {
        source: BackupTarget::ImageFile(PathBuf::from("/backup/large.img")),
        destination: VolumeRef::new("/dev/vg0/restored"),
        force: true,
        base_snapshot: None,
        block_size: Some(8 * 1024 * 1024), // 8 MiB
    };

    backend.restore_volume(&plan)?;

    Ok(())
}
```

### Restore to a ZFS dataset

```rust
use vpt_rs::{RestorePlanner, RestorePlan, BackupTarget, VolumeRef};
use vpt_rs::platform;
use std::path::PathBuf;

fn main() -> vpt_rs::Result<()> {
    let backend = platform::CurrentBackend::named("zfs")?;

    let plan = RestorePlan {
        source: BackupTarget::ImageFile(PathBuf::from("/backup/tank.img")),
        destination: VolumeRef::new("tank/restored"),
        force: false,
        base_snapshot: None,
        block_size: None,
    };

    backend.restore_volume(&plan)?;

    Ok(())
}
```

### Incremental restore with a base snapshot

```rust
use vpt_rs::{
    RestorePlanner, RestorePlan, BackupTarget, SnapshotRef, VolumeRef,
};
use vpt_rs::platform;
use std::path::PathBuf;

fn main() -> vpt_rs::Result<()> {
    let backend = platform::current_backend();

    let plan = RestorePlan {
        source: BackupTarget::ImageFile(PathBuf::from("/backup/data-incr.img")),
        destination: VolumeRef::new("/mnt/restored"),
        force: false,
        base_snapshot: Some(SnapshotRef::new("/mnt/data/.snapshots/snap1")),
        block_size: None,
    };

    backend.restore_volume(&plan)?;

    Ok(())
}
```

## Error Handling

| Error variant           | When it occurs                                          |
|-------------------------|---------------------------------------------------------|
| `UnsupportedOperation`  | Backend does not support restore                        |
| `InvalidArgument`       | `force` is required but not set, or source type unsupported |
| `MissingPath`           | Backup file specified in `source` does not exist        |
| `CommandFailed`         | External tool failed during restore                     |
| `Timeout`               | External tool exceeded the command timeout               |

## Stream vs Block-Level Restore

| Aspect            | Stream-based (Btrfs, ZFS)         | Block-level (LVM, VSS)            |
|-------------------|------------------------------------|------------------------------------|
| `force` required  | `false`                            | `true`                             |
| Destination       | New subvolume/dataset path         | Existing device path               |
| Existing data     | Preserved (new object created)     | Destroyed                          |
| `block_size` used | No (stream receive)                | Yes (block copy I/O)               |
| `base_snapshot`   | Auto-detected from stream metadata | Reserved for future use            |

:::caution
Always verify the destination path before setting `force: true`. A block-level
restore with `force` will overwrite the target volume with no confirmation
prompt.
:::
