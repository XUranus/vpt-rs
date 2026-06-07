# Capabilities

The capability system lets callers discover what a backend can do *at runtime*
without hardcoding platform knowledge. This enables graceful degradation --
your code adapts to what is available instead of failing unexpectedly.

## What is a capability?

A `Capability` is a unit enum variant that describes a single feature a
backend might support:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

:::tip First principles
Think of capabilities as feature flags that live on the backend itself. Instead
of checking `if cfg!(target_os = "linux")`, you check
`if backend.supports(Capability::IncrementalSend)`. This is more robust because
it reflects what the backend *actually* supports, not just what OS it runs on.
:::

## The eight capabilities

| Capability | Description |
|-----------|-------------|
| `CrashConsistentSnapshot` | Equivalent to pulling the power plug. The filesystem is consistent but application buffers may not be flushed. All backends support this. |
| `ApplicationConsistentSnapshot` | Coordinates with application writers (e.g. VSS writers on Windows) to flush buffers before snapshotting. Only Windows VSS currently supports this. |
| `WritableSnapshotMount` | Mount a snapshot in read-write mode so you can modify it. |
| `ReadOnlySnapshotMount` | Mount a snapshot in read-only mode for browsing. |
| `BlockLevelBackup` | Backup by copying raw blocks from the volume device (e.g. `dd`-style). |
| `BlockLevelRestore` | Restore by writing raw blocks to the volume device. |
| `IncrementalSend` | Send only the differences between two snapshots (e.g. `btrfs send -p`, `zfs send -i`). |
| `DirectDeviceAccess` | Access volumes through device paths like `/dev/vg0/data` or `\\.\C:`. |

## How capabilities are declared

Each backend declares a static slice of capabilities:

```rust
// BtrfsBackend
const CAPABILITIES: &[Capability] = &[
    Capability::CrashConsistentSnapshot,
    Capability::BlockLevelBackup,
    Capability::BlockLevelRestore,
    Capability::IncrementalSend,
];

// LvmBackend
const CAPABILITIES: &[Capability] = &[
    Capability::CrashConsistentSnapshot,
    Capability::BlockLevelBackup,
    Capability::BlockLevelRestore,
    Capability::DirectDeviceAccess,
];
```

The `Backend` trait provides the `capabilities()` accessor and a default
`supports()` method:

```rust
pub trait Backend: Send + Sync {
    fn capabilities(&self) -> &'static [Capability];

    fn supports(&self, capability: Capability) -> bool {
        self.capabilities().contains(&capability)
    }
}
```

## Checking capabilities in your code

```rust
use vpt_rs::{Backend, Capability};

let backend = vpt_rs::platform::current_backend();

// Check a single capability
if backend.supports(Capability::IncrementalSend) {
    println!("Incremental backups are supported");
}

// Check multiple capabilities
let can_snapshot_and_mount = backend.supports(Capability::CrashConsistentSnapshot)
    && backend.supports(Capability::ReadOnlySnapshotMount);

// Iterate all capabilities
for cap in backend.capabilities() {
    println!("  - {}", cap);
}
```

## Capability matrix

Here is what each backend currently supports:

| Capability | Btrfs | LVM | ZFS | Windows VSS | macOS | Unix |
|-----------|:-----:|:---:|:---:|:-----------:|:-----:|:----:|
| CrashConsistentSnapshot | Yes | Yes | Yes | Yes | Yes* | Yes* |
| ApplicationConsistentSnapshot | -- | -- | -- | Yes | -- | -- |
| WritableSnapshotMount | -- | -- | -- | -- | -- | -- |
| ReadOnlySnapshotMount | -- | -- | -- | -- | -- | -- |
| BlockLevelBackup | Yes | Yes | Yes | Yes | Yes* | Yes* |
| BlockLevelRestore | Yes | Yes | Yes | Yes | Yes* | Yes* |
| IncrementalSend | Yes | -- | Yes | -- | -- | -- |
| DirectDeviceAccess | -- | Yes | Yes | Yes | Yes* | Yes* |

\* Declared as capabilities but operations return `UnsupportedOperation`
(backend is a stub).

:::note
The macOS and Unix backends declare capabilities but all operations are
stubbed. The capabilities represent what the platform *could* support with a
real implementation. This is intentional -- it lets code that only checks
capabilities work correctly even before the backend is fully implemented.
:::

## Why capabilities matter

Instead of failing hard when a feature is missing, you can adapt:

```rust
fn backup_with_best_strategy(
    backend: &dyn vpt_rs::BackupExecutor,
    plan: &mut vpt_rs::BackupPlan,
) -> vpt_rs::Result<()> {
    if backend.supports(vpt_rs::Capability::IncrementalSend) {
        if let Some(parent) = find_last_snapshot(backend)? {
            plan.parent_snapshot = Some(parent);
        }
    }
    if backend.supports(vpt_rs::Capability::CrashConsistentSnapshot) {
        if matches!(plan.snapshot_policy, vpt_rs::SnapshotPolicy::Disabled) {
            plan.snapshot_policy = vpt_rs::SnapshotPolicy::temporary(
                vpt_rs::SnapshotKind::CrashConsistent,
                Some("auto".to_string()), true,
            );
        }
    }
    backend.backup_volume(plan)
}
```

:::warning
Checking `supports()` is a best practice but not a guarantee. A backend may
report a capability but still fail with `Error::CommandFailed`. Always handle
errors.
:::

## Capability as a Display type

`Capability` implements `Display` and `as_str()` returns the canonical
snake_case name:

```rust
let cap = vpt_rs::Capability::IncrementalSend;
println!("{}", cap);  // "incremental_send"
assert_eq!(cap.as_str(), "incremental_send");
```

## Next steps

- [Plans](./plans.md) -- how plans use capabilities for validation
- [Error Handling](./error-handling.md) -- how missing capabilities surface as errors
- [Backends](./backends.md) -- how backends declare their capabilities
