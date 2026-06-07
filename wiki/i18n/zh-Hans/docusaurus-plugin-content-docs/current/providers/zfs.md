---
sidebar_position: 3
title: ZFS 提供者
description: 使用 send/receive 对 ZFS 数据集进行流式备份
---

# ZFS 提供者

ZFS 提供者使用 `zfs send` 和 `zfs receive` 进行 ZFS 数据集的基于流的备份。它支持全量和增量发送，使其在只需传输变化块的定期备份计划中非常高效。

## 能力

| 能力 | 支持 |
|------|------|
| `crash_consistent_snapshot` | 是 |
| `application_consistent_snapshot` | 否 |
| `block_level_backup` | 是 |
| `block_level_restore` | 是 |
| `incremental_send` | 是 |
| `direct_device_access` | 是 |
| `writable_snapshot_mount` | 否 |
| `read_only_snapshot_mount` | 否 |

:::info
ZFS 提供者需要快照源才能执行 `zfs send`。你必须传递显式快照引用（如 `tank/data@snap1`）或使用临时快照策略。不支持在没有快照的情况下发送活跃数据集。
:::

## CLI 示例

```bash
# 创建快照
vptcli snapshot create tank/data --provider linux-zfs --label "nightly"

# 全量备份（自动临时快照）
vptcli backup tank/data --provider linux-zfs --output /backup/data.zfs --snapshot-label "backup"

# 增量备份
vptcli backup tank/data@snap2 --provider linux-zfs --snapshot-source --output /backup/incr.zfs --parent-snapshot tank/data@snap1

# 恢复到数据集
vptcli restore tank/restore --provider linux-zfs --input /backup/data.zfs --force
```

## 限制

- **需要快照源**：`zfs send` 需要快照引用（`pool/fs@snap`）。
- **不支持挂载/卸载**：手动通过 `.zfs/snapshot/` 目录访问 ZFS 快照。
- **不支持应用一致性快照**：请求 `ApplicationConsistent` 返回 `MissingCapability`。
- **恢复仅限数据集名**：`zfs receive` 需要数据集名称如 `pool/fs`。挂载路径会被拒绝。
- **仅基于流**：备份和恢复使用镜像文件。不支持原始块设备目标。
