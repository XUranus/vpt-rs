# vptcli backup

将卷备份到流或镜像文件。备份命令支持活跃卷和基于快照的源、可选临时快照创建、增量（基于父级）备份和可配置块大小。

## 用法

```
vptcli backup <source> --output <stream-file> [options]
```

## 选项

| 标志 | 必需 | 默认值 | 描述 |
|------|------|--------|------|
| `<source>` | **是** | -- | 源卷标识符 |
| `--output` | **是** | -- | 输出镜像/流文件路径 |
| `--provider` | 否 | 平台默认 | 后端提供者名称（Linux） |
| `--snapshot-source` | 否 | 关闭 | 将 `<source>` 视为快照 ID 而非卷 |
| `--parent-snapshot` | 否 | None | 增量备份的父快照 ID |
| `--snapshot-kind` | 否 | `crash` | 临时快照的一致性类型 |
| `--snapshot-label` | 否 | None | 临时快照名称的标签 |
| `--no-snapshot` | 否 | 关闭 | 禁用临时快照创建 |
| `--block-size` | 否 | 4 MiB | I/O 块大小 |

## 快照策略

默认情况下，`vptcli backup` 告诉后端在复制前创建临时崩溃一致性快照。

| 策略 | 标志组合 | 行为 |
|------|----------|------|
| 临时（默认） | *（无标志）* | 创建崩溃一致性快照 |
| 临时，应用安全 | `--snapshot-kind application` | 应用一致性快照 |
| 带标签快照 | `--snapshot-label "name"` | 使用特定标签 |
| 无快照 | `--no-snapshot` | 按原样使用源 |

## 增量备份

对于支持增量发送的后端（Btrfs、ZFS），使用 `--parent-snapshot` 执行增量备份：

```bash
# 全量备份
vptcli backup /mnt/data --output /backup/data-full.img

# 基于先前快照的增量备份
vptcli backup /mnt/data --output /backup/data-incr.img --parent-snapshot /mnt/data/.snapshots/snap1
```

:::caution
增量备份仅由基于流的后端（Btrfs `send`、ZFS `send`）支持。块级别后端（LVM、VSS）忽略父快照并执行全量复制。
:::

## 示例

```bash
# 全量备份 Btrfs 子卷
vptcli backup /mnt/data --output /backup/data.img

# 使用特定提供者备份
vptcli backup --provider lvm /dev/vg0/data --output /backup/vg0-data.img

# 不创建快照备份
vptcli backup /mnt/data --output /backup/data.img --no-snapshot

# 使用自定义块大小的大卷备份
vptcli backup /dev/vg0/largedisk --output /backup/disk.img --block-size 16M
```
