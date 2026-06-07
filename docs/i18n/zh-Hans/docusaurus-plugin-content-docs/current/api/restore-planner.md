# RestorePlanner Trait

`RestorePlanner` trait 从备份流或镜像文件恢复卷。实现可能使用基于流的接收（Btrfs `receive`、ZFS `receive`）或块级别写入（LVM、VSS）。破坏性后端需要计划中的 `force: true`。

## Trait 定义

```rust
pub trait RestorePlanner: Backend {
    /// 根据给定计划执行恢复。
    fn restore_volume(&self, plan: &RestorePlan) -> Result<()>;
}
```

## 关键类型

### RestorePlan

恢复操作的完整规格：

```rust
pub struct RestorePlan {
    pub source: BackupTarget,            // 从哪里读取备份
    pub destination: VolumeRef,          // 要恢复到的目标卷
    pub force: bool,                     // 允许破坏性覆盖
    pub base_snapshot: Option<SnapshotRef>, // 增量恢复的基础
    pub block_size: Option<usize>,       // I/O 块大小（None = 4 MiB）
}
```

## Force 标志

块级别后端（LVM、VSS）用备份内容覆盖目标卷。这是破坏性的 -- 目标上的所有现有数据都会丢失。为防止意外数据丢失，这些后端检查 `force` 并在为 `false` 时返回 `InvalidArgument` 错误。

基于流的后端（Btrfs `receive`、ZFS `receive`）创建新子卷或数据集，**不需要** `force`。

| 后端类型 | 需要 `force` | 行为 |
|----------|-------------|------|
| Btrfs | 否 | 从流创建新子卷 |
| ZFS | 否 | 从流创建新数据集 |
| LVM | 是 | 用块覆盖逻辑卷 |
| VSS（Windows） | 是 | 向目标卷写入块 |

## 使用示例

```rust
use vpt_rs::{RestorePlanner, RestorePlan, BackupTarget, VolumeRef};
use std::path::PathBuf;

// 流式恢复（Btrfs）
let plan = RestorePlan {
    source: BackupTarget::ImageFile(PathBuf::from("/backup/data.img")),
    destination: VolumeRef::new("/mnt/restored"),
    force: false,
    base_snapshot: None,
    block_size: None,
};
backend.restore_volume(&plan)?;

// 破坏性恢复（LVM，需要 force）
let plan = RestorePlan {
    source: BackupTarget::ImageFile(PathBuf::from("/backup/data.img")),
    destination: VolumeRef::new("/dev/vg0/data"),
    force: true,
    base_snapshot: None,
    block_size: None,
};
backend.restore_volume(&plan)?;
```
