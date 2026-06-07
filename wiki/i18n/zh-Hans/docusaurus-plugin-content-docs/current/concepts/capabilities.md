# 能力

能力系统让调用者在运行时发现后端能做什么，而无需硬编码平台知识。这实现了优雅降级 -- 你的代码适配可用的功能，而不是意外失败。

## 什么是能力？

`Capability` 是一个单元枚举变体，描述后端可能支持的单一功能：

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

:::tip 第一性原理
将能力想象为驻留在后端本身上的 feature flags。不要检查 `if cfg!(target_os = "linux")`，而是检查 `if backend.supports(Capability::IncrementalSend)`。这更健壮，因为它反映的是后端*实际*支持什么，而不仅仅是它在什么 OS 上运行。
:::

## 八种能力

| 能力 | 描述 |
|------|------|
| `CrashConsistentSnapshot` | 等同于拔掉电源插头。文件系统是一致的，但应用缓冲区可能未刷新。所有后端都支持。 |
| `ApplicationConsistentSnapshot` | 与应用写入器（如 Windows 上的 VSS 写入器）协调，在快照前刷新缓冲区。目前只有 Windows VSS 支持。 |
| `WritableSnapshotMount` | 以读写模式挂载快照，以便你可以修改它。 |
| `ReadOnlySnapshotMount` | 以只读模式挂载快照以供浏览。 |
| `BlockLevelBackup` | 通过从卷设备复制原始块来备份（如 `dd` 风格）。 |
| `BlockLevelRestore` | 通过向卷设备写入原始块来恢复。 |
| `IncrementalSend` | 只发送两个快照之间的差异（如 `btrfs send -p`、`zfs send -i`）。 |
| `DirectDeviceAccess` | 通过设备路径（如 `/dev/vg0/data` 或 `\\.\C:`）访问卷。 |

## 能力矩阵

| 能力 | Btrfs | LVM | ZFS | Windows VSS | macOS | Unix |
|------|:-----:|:---:|:---:|:-----------:|:-----:|:----:|
| CrashConsistentSnapshot | 是 | 是 | 是 | 是 | 是* | 是* |
| ApplicationConsistentSnapshot | -- | -- | -- | 是 | -- | -- |
| WritableSnapshotMount | -- | -- | -- | -- | -- | -- |
| ReadOnlySnapshotMount | -- | -- | -- | -- | -- | -- |
| BlockLevelBackup | 是 | 是 | 是 | 是 | 是* | 是* |
| BlockLevelRestore | 是 | 是 | 是 | 是 | 是* | 是* |
| IncrementalSend | 是 | -- | 是 | -- | -- | -- |
| DirectDeviceAccess | -- | 是 | 是 | 是 | 是* | 是* |

\* 声明为能力但操作返回 `UnsupportedOperation`（后端是桩实现）。

## 在代码中检查能力

```rust
use vpt_rs::{Backend, Capability};

let backend = vpt_rs::platform::current_backend();

// 检查单个能力
if backend.supports(Capability::IncrementalSend) {
    println!("Incremental backups are supported");
}

// 检查多个能力
let can_snapshot_and_mount = backend.supports(Capability::CrashConsistentSnapshot)
    && backend.supports(Capability::ReadOnlySnapshotMount);

// 遍历所有能力
for cap in backend.capabilities() {
    println!("  - {}", cap);
}
```

## 为什么能力重要

当功能缺失时，你可以适配而不是硬失败：

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
检查 `supports()` 是最佳实践但不是保证。后端可能报告一个能力但仍然以 `Error::CommandFailed` 失败。始终处理错误。
:::

## 下一步

- [Plans](./plans.md) -- 计划如何使用能力进行验证
- [Error Handling](./error-handling.md) -- 缺失能力如何表现为错误
- [Backends](./backends.md) -- 后端如何声明其能力
