---
sidebar_position: 1
---

# 编码风格与规范

本页记录了 vpt-rs 项目使用的编码规范。遵循这些规范可以保持代码库的一致性和可审查性。

## Rust 风格

vpt-rs 遵循惯用的 Rust 规范：

| 元素 | 规范 | 示例 |
|------|------|------|
| 模块 | `snake_case` | `src/platform/linux/btrfs.rs` |
| 函数 | `snake_case` | `plan_create_snapshot()`、`copy_blocks()` |
| 类型（结构体、枚举） | `PascalCase` | `BtrfsBackend`、`SnapshotRequest`、`BackupPlan` |
| 常量 | `SCREAMING_SNAKE_CASE` | `DEFAULT_BLOCK_SIZE`、`CAPABILITIES`、`BTRFS_BIN` |
| Trait | `PascalCase` | `Backend`、`SnapshotProvider`、`BackupExecutor` |
| 枚举变体 | `PascalCase` | `CrashConsistent`、`ImageFile`、`Disabled` |

## 格式化

- **4 空格缩进**（不用制表符）
- 提交前运行 `cargo fmt --all`
- 运行 `cargo clippy --all-targets -D warnings` 检查常见错误

:::tip
在 Linux 上不要使用 `--all-features` 运行 clippy —— `windows-vss` 功能需要 Windows 环境。
:::

## 错误处理

vpt-rs 使用 `thiserror` 实现类型化错误。`src/error.rs` 中的 `Error` 枚举携带结构化上下文：

```rust title="src/error.rs"
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("operation `{operation}` is not supported by backend `{backend}`")]
    UnsupportedOperation {
        operation: &'static str,
        backend: &'static str,
    },
    // ...
}
```

规范：
- **库代码中不要 panic** —— 始终返回 `Result<T, Error>`
- **使用 `?` 传播错误** —— 不要在库代码中 unwrap
- **携带上下文** —— 在错误中包含后端名称、操作和路径
- **在调用点记录错误** —— 返回前使用 `tracing::error!`

## Unsafe 代码

:::caution
保持 `unsafe` 代码隔离、有文档、有测试覆盖。
:::

所有 `unsafe` 块必须有 `// SAFETY:` 注释说明为什么不变量成立：

```rust
// SAFETY: COM objects with COINIT_MULTITHREADED are thread-safe.
unsafe impl Send for ComPtr {}
unsafe impl Sync for ComPtr {}
```

VSS COM 模块（`src/platform/windows/vss/ffi/com.rs`）是唯一包含 `unsafe` 代码的文件，通过 `windows-vss` feature gate 隔离。

## Trait 设计

- **所有操作 trait 继承 `Backend`** —— 提供 `backend_name()`、`capabilities()`、`supports()`
- **后端名称使用 `&'static str`** —— 实现零开销日志记录
- **能力集使用 `&'static [Capability]`** —— 实现编译期分配
- **优先使用 `&self` 而非 `&mut self`** —— 后端是 `Send + Sync` 且无状态

## 测试

- **单元测试与代码放在一起** —— 在 `#[cfg(test)] mod tests` 块中
- **按行为命名测试** —— 如 `application_consistent_requests_are_rejected`
- **测试计划生成而非执行** —— 计划可以在没有特权操作的情况下测试
- **集成测试基于 Python** —— 在 `tests/` 目录中，需要 root 权限

## 提交规范

使用简短的祈使句提交：

```
Add snapshot provider trait
Implement Windows VSS adapter skeleton
Fix LVM snapshot cleanup after backup failure
```

PR 应描述：
- 平台影响（哪些后端受影响）
- 所需权限（root、admin、无）
- 测试覆盖（添加了单元测试？集成测试？）
- 对磁盘格式或恢复语义的更改

## 项目结构

```
src/
  lib.rs              # 公共 API 重新导出
  backend.rs          # Backend 超 trait
  types.rs            # 共享领域类型
  error.rs            # Error 枚举（thiserror）
  snapshot.rs         # SnapshotProvider trait
  backup.rs           # BackupExecutor trait
  restore.rs          # RestorePlanner trait
  mount.rs            # MountManager trait
  copy.rs             # 块级复制工具
  process.rs          # 外部命令执行
  logging.rs          # Tracing 初始化
  bin/vptcli.rs       # CLI 二进制文件
  platform/
    mod.rs            # 平台抽象 + StubBackend
    linux/
      mod.rs          # LinuxBackend 枚举 + delegate! 宏
      btrfs.rs        # Btrfs 提供者
      lvm.rs          # LVM 提供者
      zfs.rs          # ZFS 提供者
    windows.rs        # Windows 后端（feature-gated）
    windows/vss/      # VSS 模块树
    macos.rs          # macOS 存根
    unix.rs           # 通用 Unix 存根
```
