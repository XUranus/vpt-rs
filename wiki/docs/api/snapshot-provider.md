# SnapshotProvider Trait

The `SnapshotProvider` trait handles snapshot lifecycle management: creating,
deleting, and listing provider-managed snapshots. Each platform backend
implements this trait for its native snapshot mechanism.

## Trait Definition

```rust
pub trait SnapshotProvider: Backend {
    /// Create a new snapshot of the given volume.
    fn create_snapshot(&self, request: &SnapshotRequest) -> Result<SnapshotInfo>;

    /// Delete an existing snapshot by its handle.
    fn delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<()>;

    /// List all snapshots managed by this backend for the given volume.
    fn list_snapshots(&self, source: &VolumeRef) -> Result<Vec<SnapshotInfo>>;
}
```

## Methods

| Method                | Parameters                    | Return type            | Description                          |
|-----------------------|-------------------------------|------------------------|--------------------------------------|
| `create_snapshot()`   | `&SnapshotRequest`            | `Result<SnapshotInfo>` | Create a new snapshot                |
| `delete_snapshot()`   | `&SnapshotHandle`             | `Result<()>`           | Delete an existing snapshot          |
| `list_snapshots()`    | `&VolumeRef`                  | `Result<Vec<SnapshotInfo>>` | List snapshots for a volume     |

## Key Types

### SnapshotRequest

Describes a request to create a snapshot:

```rust
pub struct SnapshotRequest {
    pub source: VolumeRef,       // Volume to snapshot
    pub kind: SnapshotKind,      // Consistency kind
    pub label: Option<String>,   // Optional label for the snapshot name
    pub read_only: bool,         // true = read-only snapshot
}
```

### SnapshotKind

```rust
pub enum SnapshotKind {
    CrashConsistent,       // Filesystem-consistent, no app quiescing
    ApplicationConsistent, // Coordinates with VSS writers (Windows)
}
```

Accepted string forms for parsing: `"crash"`, `"crash-consistent"`, `"app"`,
`"application"`, `"application-consistent"`.

### SnapshotHandle

A concrete handle identifying an existing snapshot:

```rust
pub struct SnapshotHandle {
    pub id: String,                    // Provider-specific snapshot ID
    pub source: Option<VolumeRef>,     // Source volume (if known)
}
```

The `id` format is provider-specific:

| Backend | ID format                         | Example                         |
|---------|-----------------------------------|---------------------------------|
| Btrfs   | Absolute path to snapshot subvol  | `/mnt/data/.snapshots/snap1`    |
| LVM     | `/dev/<vg>/<snapshot_lv>`         | `/dev/vg0/data-snap`            |
| ZFS     | `dataset@snapshot_name`           | `tank/data@snap1`               |
| VSS     | `{GUID}`                          | `{5F34A2B1-...}`                |

### SnapshotInfo

Metadata returned after creating or listing a snapshot:

```rust
pub struct SnapshotInfo {
    pub handle: SnapshotHandle,        // Snapshot handle
    pub backend: &'static str,         // Backend name
    pub path_hint: Option<PathBuf>,    // Filesystem path (if available)
    pub read_only: bool,               // Whether the snapshot is read-only
}
```

### VolumeRef

A stable identifier for a volume. The format is interpreted by each backend:

```rust
pub struct VolumeRef {
    pub id: String,
}
```

## Usage Examples

### Creating a snapshot

```rust
use vpt_rs::{SnapshotProvider, SnapshotRequest, SnapshotKind, VolumeRef};
use vpt_rs::platform;

fn main() -> vpt_rs::Result<()> {
    let backend = platform::current_backend();

    let request = SnapshotRequest {
        source: VolumeRef::new("/mnt/data"),
        kind: SnapshotKind::CrashConsistent,
        label: Some("nightly".to_string()),
        read_only: true,
    };

    let info = backend.create_snapshot(&request)?;
    println!("Created snapshot: {}", info.handle.id);
    println!("Backend: {}", info.backend);
    if let Some(path) = &info.path_hint {
        println!("Path: {}", path.display());
    }

    Ok(())
}
```

### Listing snapshots

```rust
use vpt_rs::{SnapshotProvider, VolumeRef};
use vpt_rs::platform;

fn main() -> vpt_rs::Result<()> {
    let backend = platform::current_backend();
    let source = VolumeRef::new("/mnt/data");

    let snapshots = backend.list_snapshots(&source)?;
    println!("Found {} snapshot(s):", snapshots.len());

    for snap in &snapshots {
        let read_only = if snap.read_only { "ro" } else { "rw" };
        println!("  {} [{}] {}", snap.handle.id, read_only, snap.backend);
    }

    Ok(())
}
```

### Deleting a snapshot

```rust
use vpt_rs::{SnapshotProvider, SnapshotHandle};
use vpt_rs::platform;

fn main() -> vpt_rs::Result<()> {
    let backend = platform::current_backend();

    let handle = SnapshotHandle {
        id: "/mnt/data/.snapshots/old-snap".to_string(),
        source: None,
    };

    backend.delete_snapshot(&handle)?;
    println!("Snapshot deleted.");

    Ok(())
}
```

### Choosing a backend by name (Linux)

```rust
use vpt_rs::{SnapshotProvider, SnapshotRequest, SnapshotKind, VolumeRef};
use vpt_rs::platform;

fn main() -> vpt_rs::Result<()> {
    // Select the LVM backend explicitly
    let backend = platform::CurrentBackend::named("lvm")?;

    let request = SnapshotRequest {
        source: VolumeRef::new("/dev/vg0/data"),
        kind: SnapshotKind::CrashConsistent,
        label: None,
        read_only: true,
    };

    let info = backend.create_snapshot(&request)?;
    println!("LVM snapshot: {}", info.handle.id);

    Ok(())
}
```

:::note
`CurrentBackend::named()` is only available on Linux where multiple backends
are registered. On other platforms, use `platform::current_backend()` which
returns the native backend.
:::

## Error Handling

All methods return `Result<T>` using the crate's `Error` enum:

| Error variant              | When it occurs                                          |
|----------------------------|---------------------------------------------------------|
| `UnsupportedOperation`     | Backend does not implement snapshot operations          |
| `MissingCapability`        | Requested snapshot kind not supported (e.g. app-consistent on Btrfs) |
| `InvalidVolume`            | Volume reference is empty                               |
| `MissingPath`              | Source path does not exist on disk                      |
| `CommandFailed`            | External tool (btrfs, lvcreate, zfs) failed             |
| `Timeout`                  | External tool exceeded the command timeout               |
