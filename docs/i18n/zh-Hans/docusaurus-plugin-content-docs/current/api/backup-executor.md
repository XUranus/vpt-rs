# BackupExecutor Trait

`BackupExecutor` trait 将卷导出为流或镜像文件。实现可能使用基于流的发送（Btrfs `send`、ZFS `send`）或块级别复制（LVM `dd` 风格、VSS 快照 + 复制）。

## Trait 定义

```rust
pub trait BackupExecutor: Backend {
    /// 根据给定计划执行备份。
    fn backup_volume(&self, plan: &BackupPlan) -> Result<()>;
}
```

## 关键类型

### BackupPlan

备份操作的完整规格：

```rust
pub struct BackupPlan {
    pub source: BackupSource,              // 备份什么
    pub target: BackupTarget,              // 写入哪里
    pub snapshot_policy: SnapshotPolicy,   // 是否先创建临时快照
    pub parent_snapshot: Option<SnapshotRef>, // 增量备份的父级
    pub block_size: Option<usize>,         // I/O 块大小（None = 4 MiB 默认）
}
```

## 使用示例

### 使用临时快照的全量备份

```rust
use vpt_rs::{BackupExecutor, BackupPlan, BackupSource, BackupTarget, SnapshotPolicy, SnapshotKind, VolumeRef};
use std::path::PathBuf;

let plan = BackupPlan {
    source: BackupSource::Volume(VolumeRef::new("/mnt/data")),
    target: BackupTarget::ImageFile(PathBuf::from("/backup/data.img")),
    snapshot_policy: SnapshotPolicy::temporary(
        SnapshotKind::CrashConsistent, Some("backup".to_string()), true,
    ),
    parent_snapshot: None,
    block_size: None,
};

backend.backup_volume(&plan)?;
```

### 使用父快照的增量备份

```rust
let plan = BackupPlan {
    source: BackupSource::Volume(VolumeRef::new("/mnt/data")),
    target: BackupTarget::ImageFile(PathBuf::from("/backup/data-incr.img")),
    snapshot_policy: SnapshotPolicy::temporary(
        SnapshotKind::CrashConsistent, Some("incr".to_string()), true,
    ),
    parent_snapshot: Some(SnapshotRef::new("/mnt/data/.snapshots/snap1")),
    block_size: None,
};

backend.backup_volume(&plan)?;
```
