---
sidebar_position: 1
title: 核心类型参考
description: vpt-rs 库中的所有公共类型
---

# 核心类型参考

本页记录 `vpt_rs` 导出的每个公共类型。所有类型位于 `vpt_rs::types` 模块中并从 crate 根重导出。

## VolumeRef

活跃卷、文件系统、数据集或提供者特定源的稳定标识符。`id` 字符串由每个后端不同地解释。

| 后端 | `id` 格式 | 示例 |
|------|-----------|------|
| Btrfs | 绝对子卷路径 | `"/mnt/data/subvol"` |
| LVM | `/dev/<vg>/<lv>` 路径 | `"/dev/vg0/data"` |
| ZFS | 数据集名称或挂载路径 | `"tank/data"` |
| Windows | 驱动器字母或卷 GUID 路径 | `"C:"` |

## Capability

枚举后端可能支持的每个功能。

| 变体 | `as_str()` | 描述 |
|------|-----------|------|
| `CrashConsistentSnapshot` | `"crash_consistent_snapshot"` | 文件系统一致性快照（无应用静默） |
| `ApplicationConsistentSnapshot` | `"application_consistent_snapshot"` | 应用静默快照（Windows 上的 VSS 写入器） |
| `WritableSnapshotMount` | `"writable_snapshot_mount"` | 可以读写挂载快照 |
| `ReadOnlySnapshotMount` | `"read_only_snapshot_mount"` | 可以只读挂载快照 |
| `BlockLevelBackup` | `"block_level_backup"` | 支持 dd 风格块复制备份 |
| `BlockLevelRestore` | `"block_level_restore"` | 支持 dd 风格块复制恢复 |
| `IncrementalSend` | `"incremental_send"` | 支持增量发送/接收流 |
| `DirectDeviceAccess` | `"direct_device_access"` | 可以读写原始块设备 |

## SnapshotKind

快照操作的一致性意图。

| 变体 | 接受的解析字符串 | 含义 |
|------|-----------------|------|
| `CrashConsistent` | `"crash"`、`"crash-consistent"` | 如拔掉电源；文件系统安全 |
| `ApplicationConsistent` | `"app"`、`"application"`、`"application-consistent"` | 与 VSS 写入器协调刷新缓冲区 |

## SnapshotRequest

提供者中立的创建快照请求。

```rust
pub struct SnapshotRequest {
    pub source: VolumeRef,       // 要快照的卷
    pub kind: SnapshotKind,      // 一致性级别
    pub label: Option<String>,   // 可选人类可读标签
    pub read_only: bool,         // 快照是否只读
}
```

## SnapshotHandle

创建后返回的具体快照标识符。

```rust
pub struct SnapshotHandle {
    pub id: String,                    // 提供者特定快照标识符
    pub source: Option<VolumeRef>,     // 源卷（如果已知）
}
```

## SnapshotRef

对现有快照的引用，用于备份/恢复规划。

```rust
pub struct SnapshotRef {
    pub id: String,                   // 快照标识符
    pub origin: Option<VolumeRef>,    // 此快照所属的卷
}
```

## BackupTarget

备份操作的目标。

| 变体 | 内部 | 描述 |
|------|------|------|
| `ImageFile` | `PathBuf` | 写入常规文件 |
| `Device` | `PathBuf` | 写入原始块设备 |

## BackupSource

备份的源 -- 活跃卷或显式快照。

```rust
pub enum BackupSource {
    Volume(VolumeRef),       // 活跃卷
    Snapshot(SnapshotRef),   // 现有快照
}
```

## SnapshotPolicy

控制提供者是否在备份前创建临时快照。

```rust
pub enum SnapshotPolicy {
    Disabled,       // 按原样使用源
    Temporary {     // 先创建临时快照
        kind: SnapshotKind,
        label: Option<String>,
        read_only: bool,
    },
}
```

## BackupPlan

描述备份什么、在哪里以及如何的提供者中立计划。

```rust
pub struct BackupPlan {
    pub source: BackupSource,
    pub target: BackupTarget,
    pub snapshot_policy: SnapshotPolicy,
    pub parent_snapshot: Option<SnapshotRef>,
    pub block_size: Option<usize>,
}
```

## RestorePlan

从备份恢复的提供者中立计划。

```rust
pub struct RestorePlan {
    pub source: BackupTarget,
    pub destination: VolumeRef,
    pub force: bool,
    pub base_snapshot: Option<SnapshotRef>,
    pub block_size: Option<usize>,
}
```

## sanitize_snapshot_label()

将用户提供的标签清理为安全的快照名称组件。`[a-zA-Z0-9\-_.+:]` 之外的字符被替换为 `-`。如果结果为空或全是破折号则返回 `"snapshot"`。

```rust
use vpt_rs::sanitize_snapshot_label;

assert_eq!(sanitize_snapshot_label("nightly backup"), "nightly-backup");
assert_eq!(sanitize_snapshot_label("2026/06/07"), "2026-06-07");
assert_eq!(sanitize_snapshot_label("---"), "snapshot");
```
