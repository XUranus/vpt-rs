---
sidebar_position: 1
title: Btrfs 提供者
description: 使用 send/receive 对 Btrfs 子卷进行流式备份
---

# Btrfs 提供者

Btrfs 提供者使用 `btrfs send` 和 `btrfs receive` 进行基于流的卷备份和恢复。它创建 Btrfs 子卷的只读快照，并将其导出为二进制流，可以保存到文件或通过网络管道传输。它是唯一支持**增量发送**的 Linux 提供者，即只传输两个快照之间变化的块。

## 能力

| 能力 | 支持 | 备注 |
|------|------|------|
| `crash_consistent_snapshot` | 是 | 使用 `btrfs subvolume snapshot -r` |
| `application_consistent_snapshot` | 否 | 返回 `MissingCapability` 错误 |
| `block_level_backup` | 是 | 通过 `btrfs send` 流导出 |
| `block_level_restore` | 是 | 通过 `btrfs receive` 流导入 |
| `incremental_send` | 是 | `btrfs send -p <parent> <snap>` |
| `direct_device_access` | 否 | 操作子卷路径而非原始设备 |
| `writable_snapshot_mount` | 否 | `mount_snapshot` 返回 `UnsupportedOperation` |
| `read_only_snapshot_mount` | 否 | `unmount` 返回 `UnsupportedOperation` |

## 源文件

| 文件 | 用途 |
|------|------|
| `src/platform/linux/btrfs.rs` | 完整提供者实现：快照、备份、恢复、列表、删除 |

提供者注册在后端名称 `"linux-btrfs"` 下（`src/platform/linux/btrfs.rs:72`）。

## 快照目录布局

提供者将快照存储在源子卷**父目录**中的隐藏目录 `.vb-snapshots/` 中：

| 源子卷 | 快照目录 |
|--------|----------|
| `/mnt/data/subvol` | `/mnt/data/.vb-snapshots/` |
| `/srv/db/main` | `/srv/db/.vb-snapshots/` |

## CLI 示例

```bash
# 创建只读快照
vptcli snapshot create /mnt/data/subvol --provider linux-btrfs --label "nightly"

# 列出子卷的快照
vptcli snapshot list --provider linux-btrfs /mnt/data/subvol

# 删除快照
vptcli snapshot delete --provider linux-btrfs /mnt/data/.vb-snapshots/nightly

# 全量备份（自动临时快照）
vptcli backup /mnt/data/subvol --provider linux-btrfs --output /backup/subvol.stream

# 使用父快照的增量备份
vptcli backup /mnt/data/.vb-snapshots/snap2 --provider linux-btrfs --snapshot-source --output /backup/incr.stream --parent-snapshot /mnt/data/.vb-snapshots/snap1

# 从流文件恢复
vptcli restore /mnt/restore --provider linux-btrfs --input /backup/subvol.stream
```

## 限制

- **不支持挂载/卸载**：`mount_snapshot` 和 `unmount` 返回 `UnsupportedOperation`。
- **不支持应用一致性快照**：请求 `ApplicationConsistent` 返回 `MissingCapability`。
- **仅基于流**：备份和恢复操作于镜像文件（流），不操作原始块设备。
- **需要绝对路径**：源必须是 Btrfs 子卷的绝对路径。
