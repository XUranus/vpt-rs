---
sidebar_position: 2
title: 错误类型参考
description: vpt-rs 中的所有错误变体和错误处理模式
---

# 错误类型参考

vpt-rs 使用单个 `Error` 枚举处理所有失败模式。便捷别名 `Result<T>` 包装 `std::result::Result<T, Error>`。枚举派生 `thiserror::Error`，因此每个变体自动实现 `Display` 和 `std::error::Error`。

```rust
use vpt_rs::{Error, Result};
```

## Error 枚举

### UnsupportedOperation

后端未实现请求的操作。

| 字段 | 类型 | 描述 |
|------|------|------|
| `operation` | `&'static str` | 失败操作的名称 |
| `backend` | `&'static str` | 不支持它的后端 |

### MissingCapability

后端存在但缺少特定能力。

| 字段 | 类型 | 描述 |
|------|------|------|
| `capability` | `&'static str` | 缺失的能力名称 |
| `backend` | `&'static str` | 缺少它的后端 |

### InvalidVolume

卷引用为空或格式错误。

| 字段 | 类型 | 描述 |
|------|------|------|
| `volume` | `String` | 有问题的卷标识符 |

### MissingPath

预期的文件系统路径不存在。

| 字段 | 类型 | 描述 |
|------|------|------|
| `path` | `PathBuf` | 缺失的文件系统路径 |

### Io

包装标准 I/O 错误。提供了 `From<std::io::Error>` 实现，因此 `?` 操作符自动转换。

:::tip
因为 `Error` 实现了 `From<std::io::Error>`，你可以在返回 `Result<T>` 的函数中对任何 `std::io` 操作使用 `?`。
:::

### InvalidArgument

函数参数在语义上无效。

| 字段 | 类型 | 描述 |
|------|------|------|
| `message` | `String` | 问题的人类可读解释 |

### CommandFailed

外部命令以非零状态退出。当后端调用 `btrfs`、`zfs` 或 `dd` 等工具时最常见的错误。

| 字段 | 类型 | 描述 |
|------|------|------|
| `command` | `String` | 执行的命令 |
| `status` | `i32` | 退出码 |
| `stderr` | `String` | 捕获的标准错误输出 |

### Timeout

外部命令超过配置的超时时间（默认 30 秒，通过 `VPT_COMMAND_TIMEOUT_SECS` 配置）。

| 字段 | 类型 | 描述 |
|------|------|------|
| `operation` | `&'static str` | 超时的操作 |
| `backend` | `&'static str` | 运行它的后端 |
| `timeout_secs` | `u64` | 超时值（秒） |

### Message

不适合其他类别的错误的通用变体。

## 访问器

### timeout_secs()

如果错误是 `Timeout` 变体返回 `Some(u64)`，否则返回 `None`。

```rust
use vpt_rs::Error;

let err = Error::Timeout {
    operation: "restore_volume", backend: "linux-lvm", timeout_secs: 60,
};
assert_eq!(err.timeout_secs(), Some(60));
```

## 错误处理模式

### 使用 `?` 操作符

```rust
use std::fs;
use vpt_rs::{VolumeRef, Result};

fn read_and_snapshot(path: &str) -> Result<()> {
    let _contents = fs::read_to_string(path)?; // io::Error -> Error::Io
    Ok(())
}
```

### 使用上下文映射错误

```rust
use vpt_rs::{Error, Result};

fn load_config(path: &str) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| {
        Error::Message {
            message: format!("failed to load config from `{path}`: {e}"),
        }
    })
}
```

:::caution
不要在 `Display` 输出字符串上进行模式匹配。字符串格式不是公共 API 的一部分，可能会改变。始终在枚举变体上匹配。
:::
