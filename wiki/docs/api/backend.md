# Backend Trait

The `Backend` trait is the common interface shared by all platform backends.
Every operational trait -- `SnapshotProvider`, `BackupExecutor`, `RestorePlanner`,
and `MountManager` -- extends `Backend`, so callers can query capabilities
without knowing which specific trait a backend implements.

## Trait Definition

```rust
pub trait Backend: Send + Sync {
    /// Return the canonical name of this backend
    /// (e.g. "linux-btrfs", "linux-lvm", "windows-vss").
    fn backend_name(&self) -> &'static str;

    /// Return the set of capabilities this backend supports.
    fn capabilities(&self) -> &'static [Capability];

    /// Check whether this backend supports a specific capability.
    fn supports(&self, capability: Capability) -> bool {
        self.capabilities().contains(&capability)
    }
}
```

## Methods

| Method           | Return type             | Description                                    |
|------------------|-------------------------|------------------------------------------------|
| `backend_name()` | `&'static str`          | Canonical backend name string                  |
| `capabilities()` | `&'static [Capability]` | Slice of all supported capabilities            |
| `supports()`     | `bool`                  | Convenience check for a single capability      |

The `supports()` method has a default implementation. You generally do not need
to override it.

## Capability Variants

The `Capability` enum lists all features a backend may support:

```rust
pub enum Capability {
    CrashConsistentSnapshot,
    ApplicationConsistentSnapshot,
    WritableSnapshotMount,
    ReadOnlySnapshotMount,
    BlockLevelBackup,
    BlockLevelRestore,
    IncrementalSend,
    DirectDeviceAccess,
}
```

## Trait Hierarchy

All four operational traits extend `Backend`:

```
Backend (supertrait)
  +-- SnapshotProvider
  +-- BackupExecutor
  +-- RestorePlanner
  +-- MountManager
```

This means any backend that implements, say, `SnapshotProvider` also implements
`Backend`. You can always call `backend_name()` and `capabilities()` on it.

## Usage Examples

### Querying the current backend

```rust
use vpt_rs::{Backend, Capability};
use vpt_rs::platform;

fn main() {
    let backend = platform::current_backend();

    println!("Backend: {}", backend.backend_name());
    println!("Supports crash-consistent snapshots: {}",
        backend.supports(Capability::CrashConsistentSnapshot));
    println!("Supports incremental send: {}",
        backend.supports(Capability::IncrementalSend));
}
```

### Listing all capabilities

```rust
use vpt_rs::Backend;
use vpt_rs::platform;

fn main() {
    let backend = platform::current_backend();

    println!("{} capabilities:", backend.backend_name());
    for cap in backend.capabilities() {
        println!("  - {}", cap);
    }
}
```

Output on a Linux Btrfs system:

```
linux-btrfs capabilities:
  - crash_consistent_snapshot
  - read_only_snapshot_mount
  - writable_snapshot_mount
  - incremental_send
```

### Checking capabilities before calling operations

```rust
use vpt_rs::{Backend, Capability, SnapshotProvider};
use vpt_rs::platform;
use vpt_rs::{SnapshotRequest, SnapshotKind, VolumeRef};

fn safe_create_snapshot(backend: &impl SnapshotProvider) -> vpt_rs::Result<()> {
    if !backend.supports(Capability::CrashConsistentSnapshot) {
        eprintln!("backend {} does not support snapshots", backend.backend_name());
        return Ok(());
    }

    let request = SnapshotRequest {
        source: VolumeRef::new("/mnt/data"),
        kind: SnapshotKind::CrashConsistent,
        label: None,
        read_only: true,
    };
    let info = backend.create_snapshot(&request)?;
    println!("created snapshot: {}", info.handle.id);
    Ok(())
}
```

## BackendDescriptor Struct

The `BackendDescriptor` struct is a static summary of a backend, used by the
CLI to print backend information without holding a live backend instance:

```rust
pub struct BackendDescriptor {
    pub platform: &'static str,          // e.g. "linux"
    pub provider_name: Option<&'static str>, // e.g. Some("btrfs")
    pub backend_name: &'static str,      // e.g. "linux-btrfs"
    pub capabilities: &'static [Capability],
}
```

### Getting descriptors programmatically

```rust
use vpt_rs::platform;

fn main() {
    // Current backend descriptor
    let desc = platform::current_backend_descriptor();
    println!("{}: {}", desc.backend_name, desc.platform);

    // All available backends on this platform
    for desc in platform::available_backend_descriptors() {
        println!("{:?} -- {} caps", desc.provider_name, desc.capabilities.len());
    }
}
```

## Platform Backends

| Platform     | Backend name    | Provider name | Notes                               |
|--------------|-----------------|---------------|--------------------------------------|
| Linux        | `linux-btrfs`   | `btrfs`       | Default on Linux                     |
| Linux        | `linux-lvm`     | `lvm`         | Requires LVM2                        |
| Linux        | `linux-zfs`     | `zfs`         | Requires ZFS on Linux                |
| macOS        | `darwin-apfs`   | --            | APFS snapshot support                |
| Windows      | `windows-vss`   | --            | Volume Shadow Copy Service           |

:::note
On non-Linux platforms, only the native backend is available. On Linux, all
three backends are registered and the default is `btrfs`. Use `--provider` to
override on the CLI or call `LinuxBackend::named("lvm")` in library code.
:::
