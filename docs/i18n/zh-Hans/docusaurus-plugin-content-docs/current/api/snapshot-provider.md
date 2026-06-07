# SnapshotProvider Trait

`SnapshotProvider` trait 处理快照生命周期管理：创建、删除和列出提供者管理的快照。每个平台后端为其原生快照机制实现此 trait。

## Trait 定义

```rust
pub trait SnapshotProvider: Backend {
    /// 创建给定卷的新快照。
    fn create_snapshot(&self, request: &SnapshotRequest) -> Result<SnapshotInfo>;

    /// 通过句柄删除现有快照。
    fn delete_snapshot(&self, snapshot: &SnapshotHandle) -> Result<()>;

    /// 列出此后端为给定卷管理的所有快照。
    fn list_snapshots(&self, source: &VolumeRef) -> Result<Vec<SnapshotInfo>>;
}
```

## 关键类型

### SnapshotRequest

描述创建快照的请求：

```rust
pub struct SnapshotRequest {
    pub source: VolumeRef,       // 要快照的卷
    pub kind: SnapshotKind,      // 一致性类型
    pub label: Option<String>,   // 快照名称的可选标签
    pub read_only: bool,         // true = 只读快照
}
```

### SnapshotHandle

标识现有快照的具体句柄：

```rust
pub struct SnapshotHandle {
    pub id: String,                    // 提供者特定的快照 ID
    pub source: Option<VolumeRef>,     // 源卷（如果已知）
}
```

`id` 格式是提供者特定的：

| 后端 | ID 格式 | 示例 |
|------|---------|------|
| Btrfs | 快照子卷的绝对路径 | `/mnt/data/.snapshots/snap1` |
| LVM | `/dev/<vg>/<snapshot_lv>` | `/dev/vg0/data-snap` |
| ZFS | `dataset@snapshot_name` | `tank/data@snap1` |
| VSS | `{GUID}` | `{5F34A2B1-...}` |

## 使用示例

```rust
use vpt_rs::{SnapshotProvider, SnapshotRequest, SnapshotKind, VolumeRef};
use vpt_rs::platform;

fn main() -> vpt_rs::Result<()> {
    let backend = platform::current_backend();

    let request = SnapshotRequest {
        source: VolumeRef::new("/mnt/data"),
        kind: SnapshotKind::CrashConsistent,
        label: Some("nightly".to_string()),
        read_only: true,
    };

    let info = backend.create_snapshot(&request)?;
    println!("Created snapshot: {}", info.handle.id);

    let snapshots = backend.list_snapshots(&VolumeRef::new("/mnt/data"))?;
    for snap in &snapshots {
        println!("  {} [{}]", snap.handle.id, snap.backend);
    }

    backend.delete_snapshot(&info.handle)?;
    Ok(())
}
```
