# vptcli restore

从 `vptcli backup` 创建的备份流或镜像文件恢复卷。

## 用法

```
vptcli restore <destination> --input <stream-file> [options]
```

## 选项

| 标志 | 必需 | 默认值 | 描述 |
|------|------|--------|------|
| `<destination>` | **是** | -- | 要恢复到的目标卷或目录 |
| `--input` | **是** | -- | 备份镜像/流文件路径 |
| `--provider` | 否 | 平台默认 | 后端提供者名称（Linux） |
| `--force` | 否 | 关闭 | 允许块级别后端的破坏性恢复 |
| `--base-snapshot` | 否 | None | 增量恢复的基础快照引用 |
| `--block-size` | 否 | 4 MiB | 块级别复制的 I/O 块大小 |

## Force 标志

某些后端执行**破坏性**恢复 -- 它们用备份内容覆盖目标卷。为安全起见，这些后端需要 `--force` 标志：

- **LVM**：用 `dd` 风格块复制覆盖逻辑卷。
- **VSS（Windows）**：直接向目标卷写入块。

基于流的后端（Btrfs `receive`、ZFS `receive`）创建新子卷或数据集，**不需要** `--force`。

:::danger
在块级别后端上使用 `--force` **会销毁目标卷上的所有现有数据**。运行命令前仔细检查目标路径。
:::

## 示例

```bash
# 从流恢复 Btrfs 子卷
vptcli restore /mnt/restored --input /backup/data.img

# 强制恢复到 LVM 逻辑卷
vptcli restore --provider lvm /dev/vg0/restored --input /backup/data.img --force

# 恢复到 ZFS 数据集
vptcli restore --provider zfs tank/restored --input /backup/tank-data.img

# 使用自定义块大小恢复
vptcli restore --provider lvm /dev/vg0/restored --input /backup/data.img --force --block-size 8M
```

## 流式与块级别恢复对比

| 方面 | 基于流（Btrfs、ZFS） | 块级别（LVM、VSS） |
|------|----------------------|---------------------|
| 需要 `--force` | 否 | 是 |
| 目标 | 新子卷/数据集路径 | 现有设备路径 |
| 现有数据 | 保留（创建新子卷） | 销毁 |
| 使用 `--block-size` | 否（流接收） | 是 |
| `--base-snapshot` | 通常从流元数据自动检测 | 保留供未来使用 |
