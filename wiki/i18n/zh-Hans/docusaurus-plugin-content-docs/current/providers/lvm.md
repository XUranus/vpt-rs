---
sidebar_position: 2
title: LVM 提供者
description: 使用快照和 copy_blocks 对 LVM 逻辑卷进行块级别备份
---

# LVM 提供者

LVM 提供者使用 Linux 逻辑卷管理器（LVM）快照结合块级别复制（`copy_blocks`）来备份和恢复逻辑卷。它创建临时 LVM 快照，将原始块复制到镜像文件，然后清理。

## 能力

| 能力 | 支持 |
|------|------|
| `crash_consistent_snapshot` | 是 |
| `application_consistent_snapshot` | 否 |
| `block_level_backup` | 是 |
| `block_level_restore` | 是 |
| `incremental_send` | 否 |
| `direct_device_access` | 是 |
| `writable_snapshot_mount` | 否 |
| `read_only_snapshot_mount` | 否 |

:::info
LVM 提供者不支持增量备份。每次备份都是源卷的完整块级别复制。
:::

## 工作原理

当请求带有临时快照策略的备份时，提供者：

1. 使用 `lvcreate --snapshot --extents 20%ORIGIN` 创建 LVM 快照
2. 使用 `lvchange --permission r` 将快照设为只读
3. 使用 `copy_blocks` 将所有块从快照设备复制到输出镜像文件
4. 使用 `lvremove --yes` 删除临时快照

## CLI 示例

```bash
# 创建快照
vptcli snapshot create /dev/vg0/data --provider linux-lvm --label "pre-upgrade"

# 全量备份（自动临时快照）
vptcli backup /dev/vg0/data --provider linux-lvm --output /backup/data.img --snapshot-label "nightly"

# 恢复到逻辑卷
vptcli restore /dev/vg0/restore --provider linux-lvm --input /backup/data.img --force
```

:::warning
LVM 恢复是破坏性的。它覆盖整个目标逻辑卷。`--force` 标志是必须的。
:::

## 限制

- **不支持增量备份**：每次备份复制所有块。
- **破坏性恢复**：恢复覆盖整个目标 LV。`--force` 标志是强制的。
- **快照空间**：快照使用源卷大小的 20%。
- **不支持挂载/卸载**：手动使用 `mount` 访问快照内容。
