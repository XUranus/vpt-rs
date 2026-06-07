---
sidebar_position: 1
title: 安装指南
description: 如何在 Linux、Windows 和 macOS 上安装 vpt-rs 及其 CLI 工具
---

# 安装指南

本指南将引导你安装 **vpt-rs** 及其命令行工具 `vptcli`。完成安装后，你将拥有一个可工作的二进制文件，可以创建快照和运行备份。

---

## 前提条件

### Rust 工具链

vpt-rs 使用 Rust 编写。你需要一个可用的 Rust 安装（推荐 1.82+）。

```bash
# 如果还没有 rustup，先安装
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 验证安装
rustc --version
cargo --version
```

:::tip
安装 rustup 后，打开一个新终端或运行 `source $HOME/.cargo/env`，这样 `cargo` 命令就可用了。
:::

### 各平台系统包

vpt-rs 将快照和备份操作委托给你平台上的原生存储工具。你必须在构建或运行 CLI **之前**安装正确的系统包。

| 提供者 | 平台 | 所需包 | 安装命令 |
|--------|------|--------|----------|
| **Btrfs** | Linux | `btrfs-progs` | `sudo apt install btrfs-progs` 或 `sudo pacman -S btrfs-progs` |
| **LVM** | Linux | `lvm2` | `sudo apt install lvm2` 或 `sudo pacman -S lvm2` |
| **ZFS** | Linux | `zfsutils-linux` | `sudo apt install zfsutils-linux` 或 `sudo pacman -S zfs-utils` |
| **VSS** | Windows | 内置 (wmic/vssadmin) | 无需额外安装 |
| **APFS** | macOS | 尚未实现 | -- |

:::caution
在 Linux 上，`vptcli` 以外部进程方式调用底层存储工具（`btrfs`、`lvs`、`zfs` 等）。如果工具未安装，对应的后端将在运行时以 `CommandFailed` 错误失败。
:::

---

## 从源码安装（推荐）

获取二进制文件最简单的方式是在项目根目录运行 `cargo install`。这会编译项目并将 `vptcli` 放置到你的 Cargo bin 目录。

```bash
cd /path/to/vpt-rs
cargo install --path .
```

完成后，验证它是否在你的 `PATH` 上：

```bash
which vptcli
# 预期输出: /home/<you>/.cargo/bin/vptcli
```

---

## 手动构建

如果你不想全局安装，可以直接构建一个 release 二进制文件：

```bash
cd /path/to/vpt-rs
cargo build --release
```

二进制文件生成在：

```
target/release/vptcli
```

你可以使用完整路径运行或将其复制到 PATH 上的某个位置：

```bash
# 方式 A：使用完整路径
./target/release/vptcli --help

# 方式 B：复制到本地 bin 目录
mkdir -p ~/.local/bin
cp target/release/vptcli ~/.local/bin/
```

---

## 验证安装

运行内置帮助命令确认一切正常：

```bash
vptcli --help
```

预期输出：

```
vptcli <command> [args]

Commands:
  snapshot    Create, list, delete snapshots; query backends and capabilities
  backup      Back up a volume to a stream or image file
  restore     Restore a volume from a stream or image file

Run `vptcli <command>` with no args for subcommand usage.
```

你还可以查看系统上有哪些可用后端：

```bash
vptcli snapshot backend list
```

:::note
在 Linux 上这总是列出 btrfs、lvm 和 zfs 后端。在其他平台上只显示平台原生后端。
:::

---

## 构建 Windows VSS 支持（可选）

在 Windows 上，你可以通过传递 feature 标志启用实验性 VSS（卷影复制）功能：

```bash
cargo build --release --features windows-vss
```

这会引入 `windows` crate 并编译 VSS 请求者支持的原生 COM FFI 代码。不使用此标志时，Windows 后端会回退到通过 `wmic` 和 `vssadmin` 的 CLI 快照管理。

---

## 启用调试日志

vpt-rs 使用 Rust 的 `tracing` crate。设置 `RUST_LOG` 环境变量以查看详细输出：

```bash
# 显示 vpt-rs 的 debug 级别消息
RUST_LOG=vpt_rs=debug vptcli snapshot backend list

# 显示 trace 级别消息（非常详细）
RUST_LOG=trace vptcli backup --provider btrfs --output /tmp/test.stream /mnt/data
```

---

## 各平台注意事项

### Linux

三个 Linux 提供者（btrfs、lvm、zfs）无条件编译。`--provider` 标志在运行时选择使用哪个。默认提供者是 **btrfs**。

```bash
# 明确选择提供者
vptcli snapshot capabilities --provider lvm
vptcli snapshot capabilities --provider zfs
```

快照操作通常需要 root 权限：

```bash
sudo vptcli snapshot create --provider btrfs /mnt/data/subvol
```

### Windows

默认 Windows 后端使用随 Windows 附带的 `wmic` 和 `vssadmin` CLI 工具。无需额外安装。要使用基于 COM 的 VSS 请求者，请使用 `--features windows-vss` 构建。

### macOS

macOS 有一个桩后端（针对 APFS），但快照和备份操作尚未实现。架构已为未来开发做好准备。

---

## 将 vpt-rs 作为库使用

如果你要在自己的 Rust 项目中集成 vpt-rs 而不是使用 CLI，请将其添加为依赖。由于 vpt-rs 尚未发布到 crates.io，请使用路径或 git 依赖：

```toml
# 在你的 Cargo.toml 中
[dependencies]
vpt-rs = { path = "/path/to/vpt-rs" }
```

或使用 git 依赖：

```toml
[dependencies]
vpt-rs = { git = "https://github.com/xuranus/vpt-rs.git" }
```

添加后，你可以直接使用库 API：

```rust
use vpt_rs::platform;
use vpt_rs::{Backend, SnapshotProvider, VolumeRef};

fn main() -> vpt_rs::Result<()> {
    let backend = platform::current_backend();
    println!("Using backend: {}", backend.backend_name());

    let snapshots = backend.list_snapshots(&VolumeRef::new("/mnt/data"))?;
    for snap in snapshots {
        println!("  {} [{}]", snap.handle.id, snap.backend);
    }
    Ok(())
}
```

:::note
将 vpt-rs 作为库使用时，如果需要 Windows 上的 VSS 支持，必须在 `Cargo.toml` 中显式启用 `windows-vss` feature：

```toml
[dependencies]
vpt-rs = { path = "/path/to/vpt-rs", features = ["windows-vss"] }
```
:::

---

## 项目结构概览

了解源码布局有助于浏览代码库或提交 issue：

```
vpt-rs/
  Cargo.toml            包清单
  src/
    lib.rs              公共 API 重导出
    bin/vptcli.rs       CLI 二进制入口点
    types.rs            核心类型 (VolumeRef, BackupPlan 等)
    snapshot.rs         SnapshotProvider trait
    backup.rs           BackupExecutor trait
    restore.rs          RestorePlanner trait
    error.rs            Error 枚举 (thiserror)
    platform/
      linux/
        btrfs.rs        Btrfs 后端 (send/receive)
        lvm.rs          LVM 后端 (块级别复制)
        zfs.rs          ZFS 后端 (send/receive)
      windows.rs        Windows VSS 后端
  tests/                基于 Python 的集成测试
```

---

## 故障排除

**`cargo build` 失败，提示 "linker not found"**
: 安装 C 工具链：`sudo apt install build-essential`（Debian/Ubuntu）或 `sudo pacman -S base-devel`（Arch）。

**`cargo install` 后提示 `vptcli: command not found`**
: 确保 `~/.cargo/bin` 在你的 `PATH` 上。在 shell 配置中添加：
  ```bash
  export PATH="$HOME/.cargo/bin:$PATH"
  ```

**运行快照操作时出现 `CommandFailed`**
: 底层存储工具未安装。请参考上面的[系统包表格](#各平台系统包)。

**权限被拒绝错误**
: 大多数快照和备份操作需要 root 权限。使用 `sudo` 或以 root 身份运行。

---

## 下一步

安装验证完成后，请继续阅读[快速开始](./quick-start.md)指南，在五分钟内创建你的第一个备份。
