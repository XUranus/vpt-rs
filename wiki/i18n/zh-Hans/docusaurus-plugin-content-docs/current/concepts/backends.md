# 后端

vpt-rs 中的"后端"是针对特定平台和卷管理技术的五个核心 trait 的具体实现。本页解释后端如何组织、选择，以及如何添加新后端。

## 后端概览

```mermaid
graph TB
    subgraph "编译时选择"
        CFG["#[cfg(target_os)]"]
    end

    subgraph "Linux (3 个提供者)"
        LinuxBackend["LinuxBackend 枚举"]
        Btrfs["BtrfsBackend"]
        Lvm["LvmBackend"]
        Zfs["ZfsBackend"]
    end

    subgraph "Windows"
        Windows["WindowsBackend"]
        VSS["VssSnapshotProvider"]
    end

    subgraph "macOS / Unix (桩实现)"
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

## CurrentBackend 类型别名

`platform` 模块定义了一个 `CurrentBackend` 类型别名，为每个目标 OS 解析为恰好一个后端。当你调用 `platform::current_backend()` 时，它返回 `CurrentBackend::default()`：

```rust
let backend = vpt_rs::platform::current_backend();
println!("Running on: {}", backend.backend_name());
```

:::tip
`CurrentBackend` 是具体类型，不是 trait 对象 -- 零动态分发开销。
:::

## BackendDescriptor

`BackendDescriptor` 是一个轻量级元数据结构体，用于在运行时枚举可用后端：

```rust
let descriptors = vpt_rs::platform::available_backend_descriptors();
for desc in &descriptors {
    println!("[{}] {} ({} capabilities)", desc.platform, desc.backend_name, desc.capabilities.len());
}
```

在 Linux 上这返回三个描述符（btrfs、lvm、zfs）。在其他平台上恰好返回一个。

## LinuxBackend：枚举 + delegate! 模式

Linux 是独特的，因为三种快照技术共存。`LinuxBackend` 是一个封装所有三种的枚举：

```rust
pub enum LinuxBackend {
    Btrfs(BtrfsBackend),
    Lvm(LvmBackend),
    Zfs(ZfsBackend),
}
```

每个 trait 方法通过 `delegate!` 宏委托给内部变体：

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
```

### 在运行时选择提供者

```rust
let backend = LinuxBackend::named("zfs")?;  // 显式选择
```

:::note
`LinuxBackend::named()` 对未知提供者名称返回 `Err(Error::InvalidArgument)`。有效名称：`"btrfs"`、`"lvm"`、`"zfs"`。
:::

## WindowsBackend：feature gates

Windows 后端使用 Cargo feature 标志控制 VSS 支持。启用 `features = ["windows-vss"]` 时，快照和备份操作委托给真正的 VSS 提供者。不启用时，它们返回 `Error::UnsupportedOperation`。

:::caution
`windows-vss` feature 需要 Windows SDK 和 COM 互操作。它仅在 Windows 构建目标上可用。
:::

## StubBackend：后备模式

`StubBackend` 实现所有五个 trait 但对每个操作方法返回 `Error::UnsupportedOperation`。macOS 和通用 Unix 后端将其封装为内部实现。即使操作被桩化，能力仍然被声明，因此调用者可以检查平台*可以*支持什么。

## 如何添加新后端

以下是添加新后端（例如 FreeBSD ZFS）的骨架：

### 1. 创建后端结构体

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

### 2. 实现 traits

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
// 类似实现 BackupExecutor、RestorePlanner、MountManager...
```

### 3. 在 platform 模块中注册

```rust
// src/platform/mod.rs
#[cfg(target_os = "freebsd")]
mod freebsd;
#[cfg(target_os = "freebsd")]
pub use freebsd::FreeBSDBackend as CurrentBackend;
```

:::tip
从桩实现开始，逐步添加真实逻辑。能力系统让调用者在运行时发现什么可用，因此桩后端仍然有用。
:::

## 下一步

- [Capabilities](./capabilities.md) -- 每个后端支持什么
- [Plans](./plans.md) -- 计划如何与后端配合
- [Error Handling](./error-handling.md) -- 后端错误如何结构化
