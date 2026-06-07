# Backend Trait

`Backend` trait 是所有平台后端共享的通用接口。每个操作 trait -- `SnapshotProvider`、`BackupExecutor`、`RestorePlanner` 和 `MountManager` -- 都扩展 `Backend`，因此调用者可以在不知道后端实现哪个特定 trait 的情况下查询能力。

## Trait 定义

```rust
pub trait Backend: Send + Sync {
    /// 返回此后端的规范名称
    /// （如 "linux-btrfs"、"linux-lvm"、"windows-vss"）。
    fn backend_name(&self) -> &'static str;

    /// 返回此后端支持的能力集。
    fn capabilities(&self) -> &'static [Capability];

    /// 检查此后端是否支持特定能力。
    fn supports(&self, capability: Capability) -> bool {
        self.capabilities().contains(&capability)
    }
}
```

## 方法

| 方法 | 返回类型 | 描述 |
|------|----------|------|
| `backend_name()` | `&'static str` | 规范后端名称字符串 |
| `capabilities()` | `&'static [Capability]` | 所有支持能力的切片 |
| `supports()` | `bool` | 单个能力的便捷检查 |

## Trait 层级

所有四个操作 trait 都扩展 `Backend`：

```
Backend (超级 trait)
  +-- SnapshotProvider
  +-- BackupExecutor
  +-- RestorePlanner
  +-- MountManager
```

## 使用示例

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

## 平台后端

| 平台 | 后端名称 | 提供者名称 | 备注 |
|------|----------|-----------|------|
| Linux | `linux-btrfs` | `btrfs` | Linux 默认 |
| Linux | `linux-lvm` | `lvm` | 需要 LVM2 |
| Linux | `linux-zfs` | `zfs` | 需要 ZFS on Linux |
| macOS | `darwin-apfs` | -- | APFS 快照支持 |
| Windows | `windows-vss` | -- | 卷影复制服务 |
