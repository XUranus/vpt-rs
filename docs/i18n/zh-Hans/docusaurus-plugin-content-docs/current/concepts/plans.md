# 计划

vpt-rs 对所有操作使用"先计划后执行"模式。计划是一个纯数据结构，描述*应该*发生什么。执行是*使其*发生的单独步骤。本页解释为什么存在此模式以及如何使用计划类型。

## 为什么需要计划？

```mermaid
flowchart LR
    A["构建计划"] --> B["验证计划"]
    B --> C["执行计划"]
    C --> D["报告结果"]

    style A fill:#e1f5fe
    style B fill:#fff3e0
    style C fill:#e8f5e9
    style D fill:#fce4ec
```

将计划与执行分离给你三个具体好处：

1. **可测试性** -- 你可以在 `#[test]` 函数中调用 `BtrfsBackend` 上的 `plan_backup()`，无需 root 权限或真实 Btrfs 文件系统。计划只是可以用 `assert_eq!` 检查的结构体。

2. **验证** -- 计划在任何工作开始前拒绝无效输入（缺失路径、不支持的快照类型、错误的目标类型）。没有半完成的操作需要清理。

3. **可组合性** -- 计划可以包含嵌套计划。例如，`BtrfsSendPlan` 包含一个可选的 `BtrfsSnapshotPlan`，用于它将在发送前创建的临时快照。

:::tip 关键洞察
计划是用户请求的"编译"形式。`BackupPlan` 是源语言；后端特定计划（如 `BtrfsSendPlan`）是解决了所有歧义的编译形式。
:::

## BackupPlan

`BackupPlan` 是备份操作的公共计划类型。它是提供者中立的 -- 它不知道 Btrfs、LVM 或 ZFS。

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupPlan {
    pub source: BackupSource,
    pub target: BackupTarget,
    pub snapshot_policy: SnapshotPolicy,
    pub parent_snapshot: Option<SnapshotRef>,
    pub block_size: Option<usize>,
}
```

### 字段说明

| 字段 | 类型 | 描述 |
|------|------|------|
| `source` | `BackupSource` | 备份什么 -- 活跃 `Volume` 或现有 `Snapshot`。 |
| `target` | `BackupTarget` | 写入哪里 -- `ImageFile` 路径或 `Device` 路径。 |
| `snapshot_policy` | `SnapshotPolicy` | 是否先创建临时快照。 |
| `parent_snapshot` | `Option<SnapshotRef>` | 增量备份的前一个快照。 |
| `block_size` | `Option<usize>` | I/O 块大小。`None` 使用默认值（4 MiB）。 |

## SnapshotPolicy

`SnapshotPolicy` 控制后端是否在备份前创建临时快照：

- **`Disabled`** -- 按原样备份源。对快照没问题，对活跃卷有风险。
- **`Temporary`** -- 创建快照，备份它，然后删除它。活跃卷的推荐默认值。

:::caution
在没有临时快照的情况下备份活跃卷可能产生不一致的镜像，特别是如果有应用正在写入该卷。始终对活跃卷使用 `SnapshotPolicy::temporary()`，除非你有特定理由不这样做。
:::

## RestorePlan

`RestorePlan` 描述如何从备份恢复卷：

```rust
let plan = RestorePlan {
    source: BackupTarget::ImageFile(PathBuf::from("/backups/data.img")),
    destination: VolumeRef::new("/dev/vg0/restore"),
    force: true,  // LVM 和 VSS 需要
    base_snapshot: None,
    block_size: None,
};

backend.restore_volume(&plan)?;
```

关键字段：`source`（备份文件）、`destination`（目标卷）、`force`（破坏性后端如 LVM/VSS 需要）。

:::warning
`force` 标志是一个安全机制。LVM 和 VSS 恢复操作会覆盖整个目标卷。
:::

## 后端特定计划类型

每个后端将公共计划转换为描述要运行的确切命令的内部计划。这些不是公共 API 的一部分，但帮助你理解底层发生了什么。

| 后端 | 内部计划类型 | 机制 |
|------|-------------|------|
| Btrfs | `BtrfsSendPlan` | `btrfs send` 流管道到文件 |
| LVM | `LvmBackupPlan` | 块级别 `dd` 风格复制 |
| ZFS | `ZfsSendPlan` | `zfs send` 流管道到文件 |

## 使用计划进行测试

先计划后执行模式使单元测试变得简单：

```rust
#[test]
fn btrfs_backup_plan_uses_send_to_image_file() {
    let backend = BtrfsBackend::new();
    let root = std::env::temp_dir().join("test-plan");
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("subvol");
    std::fs::create_dir_all(&source).unwrap();

    let plan = backend.plan_backup(&BackupPlan {
        source: BackupSource::Volume(VolumeRef::new(source.display().to_string())),
        target: BackupTarget::ImageFile(root.join("backup.stream")),
        snapshot_policy: SnapshotPolicy::temporary(
            SnapshotKind::CrashConsistent, Some("tmp".to_string()), true,
        ),
        parent_snapshot: None,
        block_size: None,
    }).unwrap();

    // 验证计划使用 btrfs send 和临时快照
    assert_eq!(plan.command.program, "btrfs");
    assert_eq!(plan.command.args[0], "send");
    assert!(plan.temporary_snapshot.is_some());

    let _ = std::fs::remove_dir_all(&root);
}
```

:::tip
后端结构体上的 `plan_*` 方法是公共的。你可以在测试中直接调用它们，无需 root 权限或真实卷。
:::

## 下一步

- [Error Handling](./error-handling.md) -- 计划失败时会发生什么
- [Capabilities](./capabilities.md) -- 能力如何影响计划验证
- [Traits](./traits.md) -- 接受计划的 traits
