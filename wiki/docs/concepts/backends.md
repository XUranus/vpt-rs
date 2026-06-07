# Backends

A "backend" in vpt-rs is a concrete implementation of the five core traits for
a specific platform and volume management technology. This page explains how
backends are organized, selected, and how to add a new one.

## Backend overview

```mermaid
graph TB
    subgraph "Compile-time selection"
        CFG["#[cfg(target_os)]"]
    end

    subgraph "Linux (3 providers)"
        LinuxBackend["LinuxBackend enum"]
        Btrfs["BtrfsBackend"]
        Lvm["LvmBackend"]
        Zfs["ZfsBackend"]
    end

    subgraph "Windows"
        Windows["WindowsBackend"]
        VSS["VssSnapshotProvider"]
    end

    subgraph "macOS / Unix (stubs)"
        MacOS["MacOsBackend"]
        Unix["UnixBackend"]
    end

    CFG -->|"target_os = linux"| LinuxBackend
    CFG -->|"target_os = windows"| Windows
    CFG -->|"target_os = macos"| MacOS
    CFG -->|"other unix"| Unix

    LinuxBackend --> Btrfs
    LinuxBackend --> Lvm
    LinuxBackend --> Zfs
    Windows --> VSS
    MacOS --> StubBackend["StubBackend"]
    Unix --> StubBackend
```

## CurrentBackend type alias

The `platform` module defines a `CurrentBackend` type alias that resolves to
exactly one backend per target OS. When you call `platform::current_backend()`,
it returns `CurrentBackend::default()`:

```rust
let backend = vpt_rs::platform::current_backend();
println!("Running on: {}", backend.backend_name());
```

:::tip
`CurrentBackend` is a concrete type, not a trait object -- zero dynamic
dispatch overhead.
:::

## BackendDescriptor

A `BackendDescriptor` is a lightweight metadata struct for enumerating
available backends at runtime:

```rust
let descriptors = vpt_rs::platform::available_backend_descriptors();
for desc in &descriptors {
    println!("[{}] {} ({} capabilities)", desc.platform, desc.backend_name, desc.capabilities.len());
}
```

On Linux this returns three descriptors (btrfs, lvm, zfs). On other platforms
it returns exactly one.

## LinuxBackend: the enum + delegate! pattern

Linux is unique because three snapshot technologies coexist. `LinuxBackend` is
an enum that wraps all three:

```rust
pub enum LinuxBackend {
    Btrfs(BtrfsBackend),
    Lvm(LvmBackend),
    Zfs(ZfsBackend),
}
```

Every trait method delegates to the inner variant using the `delegate!` macro:

```rust
macro_rules! delegate {
    ($self:expr, $method:ident $(, $arg:expr)*) => {
        match $self {
            Self::Btrfs(inner) => inner.$method($($arg),*),
            Self::Lvm(inner) => inner.$method($($arg),*),
            Self::Zfs(inner) => inner.$method($($arg),*),
        }
    };
}

impl SnapshotProvider for LinuxBackend {
    fn create_snapshot(&self, request: &SnapshotRequest) -> Result<SnapshotInfo> {
        delegate!(self, create_snapshot, request)
    }
    // delete_snapshot, list_snapshots, backup_volume, etc.
}
```

### Selecting a provider at runtime

```rust
let backend = LinuxBackend::named("zfs")?;  // explicit selection
```

:::note
`LinuxBackend::named()` returns `Err(Error::InvalidArgument)` for unknown
provider names. Valid names: `"btrfs"`, `"lvm"`, `"zfs"`.
:::

## WindowsBackend: feature gates

The Windows backend uses Cargo feature flags to control VSS support. With
`features = ["windows-vss"]` enabled, snapshot and backup operations delegate
to the real VSS provider. Without the feature, they return
`Error::UnsupportedOperation`.

:::caution
The `windows-vss` feature requires the Windows SDK and COM interop. It is only
available on Windows build targets.
:::

## StubBackend: the fallback pattern

`StubBackend` implements all five traits but returns
`Error::UnsupportedOperation` for every operational method. macOS and generic
Unix backends wrap it as their inner implementation. Even though operations are
stubbed, capabilities are still declared, so callers can check what the
platform *could* support.

## How to add a new backend

Here is a skeleton for adding a new backend (e.g. FreeBSD ZFS):

### 1. Create the backend struct

```rust
// src/platform/freebsd.rs
use super::StubBackend;
use crate::types::Capability;

const CAPABILITIES: &[Capability] = &[
    Capability::CrashConsistentSnapshot,
    Capability::BlockLevelBackup,
    Capability::BlockLevelRestore,
];

#[derive(Debug, Clone)]
pub struct FreeBSDBackend(StubBackend);

impl FreeBSDBackend {
    pub fn new() -> Self {
        Self(StubBackend::new("freebsd-zfs", CAPABILITIES))
    }
}
```

### 2. Implement the traits

```rust
impl Backend for FreeBSDBackend {
    fn backend_name(&self) -> &'static str { self.0.backend_name() }
    fn capabilities(&self) -> &'static [Capability] { self.0.capabilities() }
}

impl SnapshotProvider for FreeBSDBackend {
    fn create_snapshot(&self, request: &SnapshotRequest) -> Result<SnapshotInfo> { todo!() }
    fn delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<()> { todo!() }
    fn list_snapshots(&self, source: &VolumeRef) -> Result<Vec<SnapshotInfo>> { todo!() }
}
// Implement BackupExecutor, RestorePlanner, MountManager similarly...
```

### 3. Register in the platform module

```rust
// src/platform/mod.rs
#[cfg(target_os = "freebsd")]
mod freebsd;
#[cfg(target_os = "freebsd")]
pub use freebsd::FreeBSDBackend as CurrentBackend;
```

:::tip
Start with a stub and add real logic incrementally. The capability system
lets callers discover what works at runtime, so a stubbed backend is still
useful.
:::

## Next steps

- [Capabilities](./capabilities.md) -- what each backend supports
- [Plans](./plans.md) -- how plans work with backends
- [Error Handling](./error-handling.md) -- how backend errors are structured
