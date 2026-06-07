# Architecture

This page explains how vpt-rs is organized, how its layers connect, and how a
backup request flows from the CLI down to the platform-specific tool that does
the actual work. If you are new to the project, start here.

## High-level overview

vpt-rs is structured as three layers:

1. **Public API** -- the five traits (`Backend`, `SnapshotProvider`,
   `BackupExecutor`, `RestorePlanner`, `MountManager`) and the plan/request
   structs that callers interact with.
2. **Platform dispatch** -- compile-time `cfg` selects the right backend for the
   target OS, while runtime helpers let the CLI enumerate available backends.
3. **Provider implementations** -- each backend wraps a native tool (`btrfs`,
   `lvcreate`, `zfs`, VSS) and translates plans into shell commands.

```mermaid
graph TB
    subgraph "Public API"
        Backend["Backend trait<br/>src/backend.rs"]
        SnapshotProvider["SnapshotProvider trait<br/>src/snapshot.rs"]
        BackupExecutor["BackupExecutor trait<br/>src/backup.rs"]
        RestorePlanner["RestorePlanner trait<br/>src/restore.rs"]
        MountManager["MountManager trait<br/>src/mount.rs"]
        Types["Plan & Request structs<br/>src/types.rs"]
    end

    subgraph "Platform Dispatch"
        CurrentBackend["CurrentBackend type alias<br/>src/platform/mod.rs"]
        StubBackend["StubBackend (fallback)<br/>src/platform/mod.rs"]
        BackendDescriptor["BackendDescriptor<br/>src/platform/mod.rs"]
    end

    subgraph "Linux Providers"
        LinuxBackend["LinuxBackend enum<br/>src/platform/linux/mod.rs"]
        BtrfsBackend["BtrfsBackend<br/>src/platform/linux/btrfs.rs"]
        LvmBackend["LvmBackend<br/>src/platform/linux/lvm.rs"]
        ZfsBackend["ZfsBackend<br/>src/platform/linux/zfs.rs"]
    end

    subgraph "Windows"
        WindowsBackend["WindowsBackend<br/>src/platform/windows.rs"]
        VssProvider["VssSnapshotProvider"]
    end

    subgraph "macOS / Unix (stubs)"
        MacOsBackend["MacOsBackend<br/>src/platform/macos.rs"]
        UnixBackend["UnixBackend<br/>src/platform/unix.rs"]
    end

    Backend --> CurrentBackend
    CurrentBackend --> LinuxBackend
    CurrentBackend --> WindowsBackend
    CurrentBackend --> MacOsBackend
    CurrentBackend --> UnixBackend

    LinuxBackend --> BtrfsBackend
    LinuxBackend --> LvmBackend
    LinuxBackend --> ZfsBackend
    WindowsBackend --> VssProvider
    MacOsBackend --> StubBackend
    UnixBackend --> StubBackend

    BtrfsBackend -->|"btrfs subvolume/send"| Shell["Shell commands via<br/>src/process.rs"]
    LvmBackend -->|"lvcreate/dd"| Shell
    ZfsBackend -->|"zfs snapshot/send"| Shell
    VssProvider -->|"VSS COM API"| Shell
```

## Source file map

Every source file has a clear responsibility. This table is your map for
navigating the codebase:

| File | Purpose |
|------|---------|
| `src/lib.rs` | Public re-exports; the crate's front door |
| `src/backend.rs` | The `Backend` supertrait (identity + capabilities) |
| `src/snapshot.rs` | The `SnapshotProvider` trait (create/delete/list) |
| `src/backup.rs` | The `BackupExecutor` trait (backup a volume) |
| `src/restore.rs` | The `RestorePlanner` trait (restore a volume) |
| `src/mount.rs` | The `MountManager` trait (mount/unmount snapshots) |
| `src/types.rs` | All plan/request/handle structs and the `Capability` enum |
| `src/error.rs` | The `Error` enum and `Result<T>` alias |
| `src/process.rs` | Shell command runner with timeout and I/O redirection |
| `src/copy.rs` | Block-level copy helper (used by LVM backend) |
| `src/logging.rs` | Tracing/logging setup |
| `src/platform/mod.rs` | `CurrentBackend` alias, `StubBackend`, `BackendDescriptor` |
| `src/platform/linux/mod.rs` | `LinuxBackend` enum + `delegate!` macro |
| `src/platform/linux/btrfs.rs` | `BtrfsBackend` (subvolume snapshots + send/receive) |
| `src/platform/linux/lvm.rs` | `LvmBackend` (LVM snapshots + block copy) |
| `src/platform/linux/zfs.rs` | `ZfsBackend` (ZFS snapshots + send/receive) |
| `src/platform/windows.rs` | `WindowsBackend` + VSS integration |
| `src/platform/macos.rs` | `MacOsBackend` (stub) |
| `src/platform/unix.rs` | `UnixBackend` (stub) |

:::tip How to navigate
Start from the trait you want to understand (e.g. `SnapshotProvider` in
`src/snapshot.rs`), then follow the implementations into the platform modules.
Each backend file is self-contained -- it declares its capabilities, implements
the traits, and defines its internal plan types all in one place.
:::

## The trait hierarchy

All five traits share a common supertrait pattern. `Backend` sits at the root
and provides identity (`backend_name()`) and capability metadata
(`capabilities()`, `supports()`). The four operational traits extend it.

```mermaid
classDiagram
    class Backend {
        <<trait>>
        +backend_name() &'static str
        +capabilities() &'static [Capability]
        +supports(capability) bool
    }

    class SnapshotProvider {
        <<trait>>
        +create_snapshot(&SnapshotRequest) Result~SnapshotInfo~
        +delete_snapshot(&SnapshotHandle) Result~()~
        +list_snapshots(&VolumeRef) Result~Vec~SnapshotInfo~~
    }

    class BackupExecutor {
        <<trait>>
        +backup_volume(&BackupPlan) Result~()~
    }

    class RestorePlanner {
        <<trait>>
        +restore_volume(&RestorePlan) Result~()~
    }

    class MountManager {
        <<trait>>
        +mount_snapshot(&MountRequest) Result~MountHandle~
        +unmount(&MountHandle) Result~()~
    }

    Backend <|-- SnapshotProvider : extends
    Backend <|-- BackupExecutor : extends
    Backend <|-- RestorePlanner : extends
    Backend <|-- MountManager : extends
```

Every operational trait uses `: Backend` as a supertrait bound. This means if
you have a `Box<dyn SnapshotProvider>`, you can always call `backend_name()` on
it without downcasting. The compiler guarantees the method exists.

:::note
The `Backend` trait also requires `Send + Sync` (`src/backend.rs:20`). This
means all backends are safe to share across threads -- a requirement for async
executors and thread pools that may run backup operations concurrently.
:::

## Platform dispatch with cfg(target_os)

The `platform` module (`src/platform/mod.rs`) uses Rust's `#[cfg(target_os)]`
attribute to select exactly one backend type per compilation target. This
happens at compile time, so there is zero runtime branching cost.

```mermaid
flowchart TD
    Compile["Compilation starts"]
    Compile --> Check{"target_os?"}
    Check -->|linux| Linux["mod linux<br/>pub use LinuxBackend as CurrentBackend"]
    Check -->|windows| Win["mod windows<br/>pub use WindowsBackend as CurrentBackend"]
    Check -->|macos| Mac["mod macos<br/>pub use MacOsBackend as CurrentBackend"]
    Check -->|other unix| Unix["mod unix<br/>pub use UnixBackend as CurrentBackend"]

    Linux --> LMod["LinuxBackend enum wraps<br/>BtrfsBackend, LvmBackend, ZfsBackend"]
    Win --> WMod["WindowsBackend with<br/>optional VSS feature gate"]
    Mac --> MMod["MacOsBackend wraps<br/>StubBackend"]
    Unix --> UMod["UnixBackend wraps<br/>StubBackend"]

    LMod --> Output["CurrentBackend = LinuxBackend"]
    WMod --> Output2["CurrentBackend = WindowsBackend"]
    MMod --> Output3["CurrentBackend = MacOsBackend"]
    UMod --> Output4["CurrentBackend = UnixBackend"]
```

The key code in `src/platform/mod.rs`:

```rust
// src/platform/mod.rs:1-12
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(all(
    target_family = "unix",
    not(target_os = "linux"),
    not(target_os = "macos")
))]
mod unix;
#[cfg(target_os = "windows")]
mod windows;
```

And the type alias that selects the concrete backend:

```rust
// src/platform/mod.rs:14-16
#[cfg(target_os = "linux")]
pub use linux::LinuxBackend as CurrentBackend;
```

:::tip Why compile-time dispatch?
Using `cfg` instead of trait objects means `CurrentBackend` is a concrete type.
This gives the compiler full visibility for inlining, monomorphization, and
dead-code elimination. There is no vtable indirection.
:::

## The StubBackend pattern

`StubBackend` (`src/platform/mod.rs:88-170`) is a concrete struct that
implements all five traits but returns `Error::UnsupportedOperation` for every
operational method. It serves two purposes:

1. **Default backend for unsupported platforms** -- macOS and generic Unix
   backends wrap a `StubBackend` and delegate to it.
2. **Capability declaration** -- even when operations are stubbed, the backend
   still declares its capabilities so callers can check what *would* be
   supported.

Here is the full struct definition:

```rust
// src/platform/mod.rs:88-100
#[derive(Debug, Clone, Default)]
pub struct StubBackend {
    backend_name: &'static str,
    capabilities: &'static [Capability],
}

impl StubBackend {
    pub const fn new(backend_name: &'static str, capabilities: &'static [Capability]) -> Self {
        Self {
            backend_name,
            capabilities,
        }
    }
}
```

Every operational method follows the same pattern -- build an error, log it,
and return `Err`:

```rust
// src/platform/mod.rs:122-128
impl SnapshotProvider for StubBackend {
    fn create_snapshot(&self, _request: &SnapshotRequest) -> Result<SnapshotInfo> {
        let error = unsupported("create_snapshot", self.backend_name);
        error!(backend = self.backend_name, error = %error, "create_snapshot failed");
        Err(error)
    }
    // delete_snapshot, list_snapshots follow the same pattern
}
```

:::caution
A `StubBackend` is not useless. Its `capabilities()` method still returns real
data. Code that only checks capabilities (e.g. to decide whether to offer
incremental backups in a UI) will work correctly even on stubbed platforms.
:::

## LinuxBackend and the delegate! macro

On Linux, three snapshot technologies coexist (Btrfs, LVM, ZFS). The
`LinuxBackend` enum (`src/platform/linux/mod.rs:34-39`) wraps all three:

```rust
// src/platform/linux/mod.rs:34-39
#[derive(Debug, Clone)]
pub enum LinuxBackend {
    Btrfs(BtrfsBackend),
    Lvm(LvmBackend),
    Zfs(ZfsBackend),
}
```

To avoid repeating the same `match` block for every trait method, the module
defines a `delegate!` macro:

```rust
// src/platform/linux/mod.rs:24-32
macro_rules! delegate {
    ($self:expr, $method:ident $(, $arg:expr)*) => {
        match $self {
            Self::Btrfs(inner) => inner.$method($($arg),*),
            Self::Lvm(inner) => inner.$method($($arg),*),
            Self::Zfs(inner) => inner.$method($($arg),*),
        }
    };
}
```

This macro is used in every trait implementation:

```rust
// src/platform/linux/mod.rs:85-93
impl Backend for LinuxBackend {
    fn backend_name(&self) -> &'static str {
        delegate!(self, backend_name)
    }
    fn capabilities(&self) -> &'static [Capability] {
        delegate!(self, capabilities)
    }
}
```

```mermaid
flowchart LR
    Call["backend.backup_volume(plan)"]
    Call --> Match{"match self"}
    Match -->|Btrfs| B["BtrfsBackend::backup_volume(plan)"]
    Match -->|Lvm| L["LvmBackend::backup_volume(plan)"]
    Match -->|Zfs| Z["ZfsBackend::backup_volume(plan)"]
    B --> BPlan["plan_backup() -> BtrfsSendPlan<br/>run_send()"]
    L --> LPlan["plan_backup() -> LvmBackupPlan<br/>copy_blocks()"]
    Z --> ZPlan["plan_backup() -> ZfsSendPlan<br/>zfs send"]
```

:::note
The `delegate!` macro eliminates ~120 lines of boilerplate that would otherwise
be needed for the 6 methods across 5 traits (Backend, SnapshotProvider,
BackupExecutor, RestorePlanner, MountManager). Adding a new method to any trait
requires only one extra `delegate!` call in the `LinuxBackend` implementation.
:::

## The plan-then-execute pattern

Every backup, restore, and snapshot operation follows a two-phase pattern:

1. **Plan** -- build a backend-specific plan struct that describes *what* will
   happen. Plans are pure data; they do not touch the filesystem.
2. **Execute** -- run the plan by invoking shell commands or copy routines.

```mermaid
sequenceDiagram
    participant Caller as Caller code
    participant Backend as BtrfsBackend
    participant Plan as BtrfsSendPlan
    participant Shell as btrfs CLI

    Caller->>Backend: backup_volume(BackupPlan)
    Backend->>Backend: plan_backup(BackupPlan)
    Note over Backend: Validates source, target,<br/>snapshot policy, path existence
    Backend-->>Plan: BtrfsSendPlan created
    Note over Plan: Contains: source path,<br/>target path, parent snapshot,<br/>temporary snapshot plan, command args
    Backend->>Backend: run_send(BtrfsSendPlan)
    alt Has temporary snapshot
        Backend->>Shell: btrfs subvolume snapshot ...
        Shell-->>Backend: exit 0
    end
    Backend->>Shell: btrfs send -p parent source > target
    Shell-->>Backend: exit 0
    alt Has temporary snapshot
        Backend->>Shell: btrfs subvolume delete (temp)
        Shell-->>Backend: exit 0
    end
    Backend-->>Caller: Ok(())
```

This separation has concrete benefits:

- **Unit testing** -- you can call `plan_backup()` on a `BtrfsBackend` in a
  test without root privileges or a real Btrfs filesystem. The plan struct is
  just data you can inspect with `assert_eq!`.
- **Validation** -- plans reject invalid inputs (missing paths, unsupported
  snapshot kinds) before any work begins.
- **Composability** -- a plan can include a nested plan (e.g. a temporary
  snapshot plan inside a send plan).

## End-to-end backup flow

Here is the full sequence when a user runs `vpt backup /mnt/data /tmp/backup.img`:

```mermaid
sequenceDiagram
    participant CLI as vpt CLI
    participant Platform as platform::current_backend()
    participant Backend as LinuxBackend enum
    participant Btrfs as BtrfsBackend
    participant Process as process::run_command()
    participant Shell as btrfs binary

    CLI->>Platform: current_backend()
    Platform-->>CLI: LinuxBackend::Btrfs(BtrfsBackend)
    CLI->>Backend: backup_volume(BackupPlan)
    Backend->>Backend: delegate!(self, backup_volume, plan)
    Backend->>Btrfs: backup_volume(plan)
    Btrfs->>Btrfs: plan_backup(plan)
    Note over Btrfs: Validates paths,<br/>resolves snapshot policy
    Btrfs-->>Btrfs: BtrfsSendPlan
    Btrfs->>Btrfs: run_send(send_plan)
    Btrfs->>Process: run_command("btrfs", ["subvolume", "snapshot", ...])
    Process->>Shell: spawn process with timeout
    Shell-->>Process: exit 0, stderr=""
    Process-->>Btrfs: Ok(Output)
    Btrfs->>Process: run_command("btrfs", ["send", ...], stdout=file)
    Process->>Shell: spawn with stdout redirected to backup file
    Shell-->>Process: exit 0
    Process-->>Btrfs: Ok(Output)
    Btrfs->>Process: run_command("btrfs", ["subvolume", "delete", temp])
    Process->>Shell: spawn
    Shell-->>Process: exit 0
    Process-->>Btrfs: Ok(Output)
    Btrfs-->>Backend: Ok(())
    Backend-->>CLI: Ok(())
    CLI->>CLI: Print success message
```

:::caution
Backends require elevated privileges. Btrfs snapshot operations, LVM
`lvcreate`, and Windows VSS all need root or Administrator access. The library
returns `Error::CommandFailed` with the stderr output if the underlying tool
is denied permission.
:::

## File layout

```
src/
  lib.rs              -- public re-exports (the crate's front door)
  backend.rs          -- Backend supertrait (20 lines)
  snapshot.rs         -- SnapshotProvider trait (31 lines)
  backup.rs           -- BackupExecutor trait (22 lines)
  restore.rs          -- RestorePlanner trait (22 lines)
  mount.rs            -- MountManager trait (17 lines)
  types.rs            -- plan/request structs, Capability enum (~350 lines)
  error.rs            -- Error enum, Result alias (~60 lines)
  process.rs          -- shell command runner with timeout + I/O redirect
  copy.rs             -- block-level copy helper for LVM
  logging.rs          -- tracing setup
  platform/
    mod.rs            -- CurrentBackend alias, StubBackend, BackendDescriptor
    linux/
      mod.rs          -- LinuxBackend enum + delegate! macro
      btrfs.rs        -- BtrfsBackend (~700 lines, includes tests)
      lvm.rs          -- LvmBackend (~670 lines, includes tests)
      zfs.rs          -- ZfsBackend (~750 lines, includes tests)
    windows.rs        -- WindowsBackend + VSS integration
    macos.rs          -- MacOsBackend (stub)
    unix.rs           -- UnixBackend (stub)
```

:::tip How to explore
1. Start with `src/lib.rs` to see what the crate exports.
2. Read `src/backend.rs` (20 lines) to understand the supertrait.
3. Read `src/types.rs` to understand the data structures.
4. Pick one backend (e.g. `src/platform/linux/btrfs.rs`) and trace a single
   operation from trait implementation through plan creation to execution.
5. Return to `src/platform/mod.rs` to understand how dispatch works.
:::

## Next steps

- [Traits](./traits.md) -- deep dive into each of the five traits
- [Backends](./backends.md) -- how backends are selected and how to add a new one
- [Capabilities](./capabilities.md) -- the capability system and graceful degradation
- [Plans](./plans.md) -- the plan-then-execute pattern in detail
- [Error Handling](./error-handling.md) -- structured errors with context
