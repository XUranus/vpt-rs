---
sidebar_position: 2
title: Error Types Reference
description: All error variants and error handling patterns in vpt-rs
---

# Error Types Reference

vpt-rs uses a single `Error` enum for all failure modes. A convenience alias
`Result<T>` wraps `std::result::Result<T, Error>`. The enum derives
`thiserror::Error`, so every variant implements `Display` and `std::error::Error`
automatically.

```rust
use vpt_rs::{Error, Result};
```

## The Error Enum

### UnsupportedOperation

The backend does not implement the requested operation.

```rust
use vpt_rs::Error;

let err = Error::UnsupportedOperation {
    operation: "backup_volume",
    backend: "linux-stub",
};
println!("{err}");
// "operation `backup_volume` is not supported by backend `linux-stub`"
```

| Field       | Type           | Description                              |
|-------------|----------------|------------------------------------------|
| `operation` | `&'static str` | Name of the failed operation             |
| `backend`   | `&'static str` | Backend that does not support it         |

### MissingCapability

The backend exists but lacks a specific capability.

```rust
use vpt_rs::{Error, Capability};

let err = Error::MissingCapability {
    capability: Capability::ApplicationConsistentSnapshot.as_str(),
    backend: "linux-btrfs",
};
println!("{err}");
// "capability `application_consistent_snapshot` is not available on backend `linux-btrfs`"
```

| Field        | Type           | Description                             |
|--------------|----------------|-----------------------------------------|
| `capability` | `&'static str` | The missing capability name             |
| `backend`    | `&'static str` | Backend that lacks it                   |

### InvalidVolume

A volume reference is empty or malformed.

```rust
use vpt_rs::Error;

let err = Error::InvalidVolume {
    volume: String::new(),
};
println!("{err}");
// "invalid volume reference ``"
```

| Field    | Type     | Description                              |
|----------|----------|------------------------------------------|
| `volume` | `String` | The problematic volume identifier        |

### MissingPath

An expected filesystem path does not exist.

```rust
use vpt_rs::Error;
use std::path::PathBuf;

let err = Error::MissingPath {
    path: PathBuf::from("/tmp/nonexistent.img"),
};
println!("{err}");
// "path does not exist: /tmp/nonexistent.img"
```

| Field  | Type      | Description                         |
|--------|-----------|-------------------------------------|
| `path` | `PathBuf` | The missing filesystem path         |

### Io

Wraps a standard I/O error. The `From<std::io::Error>` implementation is
provided, so the `?` operator converts automatically.

```rust
use vpt_rs::Error;

let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
let err: Error = io_err.into();
println!("{err}");
// "io error: access denied"
```

:::tip
Because `Error` implements `From<std::io::Error>`, you can use `?` on any
`std::io` operation inside a function returning `Result<T>`.
:::

### InvalidArgument

A function argument is semantically invalid.

```rust
use vpt_rs::Error;

let err = Error::InvalidArgument {
    message: "block_size must be a power of 2".to_string(),
};
println!("{err}");
// "invalid argument: block_size must be a power of 2"
```

| Field     | Type     | Description                               |
|-----------|----------|-------------------------------------------|
| `message` | `String` | Human-readable explanation of the problem |

### CommandFailed

An external command exited with a non-zero status. This is the most common
error when backends shell out to tools like `btrfs`, `zfs`, or `dd`.

```rust
use vpt_rs::Error;

let err = Error::CommandFailed {
    command: "btrfs subvolume snapshot -r /mnt/data /mnt/data/snap".to_string(),
    status: 1,
    stderr: "ERROR: cannot snapshot /mnt/data: not a btrfs filesystem".to_string(),
};
println!("{err}");
// "command `btrfs subvolume snapshot ...` failed with status 1: ERROR: cannot snapshot ..."
```

| Field     | Type     | Description                              |
|-----------|----------|------------------------------------------|
| `command` | `String` | The command that was executed            |
| `status`  | `i32`    | Exit code                                |
| `stderr`  | `String` | Captured standard error output           |

### Timeout

An external command exceeded the configured timeout (default 30 seconds,
configurable via `VPT_COMMAND_TIMEOUT_SECS`).

```rust
use vpt_rs::Error;

let err = Error::Timeout {
    operation: "backup_volume",
    backend: "linux-zfs",
    timeout_secs: 30,
};
println!("{err}");
// "operation `backup_volume` on backend `linux-zfs` timed out after 30s"
```

| Field          | Type           | Description                        |
|----------------|----------------|------------------------------------|
| `operation`    | `&'static str` | The operation that timed out       |
| `backend`      | `&'static str` | Backend that was running it        |
| `timeout_secs` | `u64`          | The timeout value in seconds       |

### Message

A catch-all variant for errors that do not fit the other categories.

```rust
use vpt_rs::Error;

let err = Error::Message {
    message: "internal invariant violated: snapshot list returned None".to_string(),
};
println!("{err}");
// "internal invariant violated: snapshot list returned None"
```

| Field     | Type     | Description           |
|-----------|----------|-----------------------|
| `message` | `String` | The error description |

## Result Type Alias

```rust
pub type Result<T> = std::result::Result<T, Error>;
```

Use this in your own functions that call vpt-rs APIs:

```rust
use vpt_rs::{BackupPlan, Result};

fn create_plan() -> Result<BackupPlan> {
    // ... build the plan
    # todo!()
}
```

## Accessors

### timeout_secs()

Returns `Some(u64)` if the error is a `Timeout` variant, `None` otherwise.

```rust
use vpt_rs::Error;

let err = Error::Timeout {
    operation: "restore_volume",
    backend: "linux-lvm",
    timeout_secs: 60,
};
assert_eq!(err.timeout_secs(), Some(60));

let err2 = Error::InvalidArgument {
    message: "bad".to_string(),
};
assert_eq!(err2.timeout_secs(), None);
```

## Matching on Specific Variants

Use `match` or `if let` to handle individual error kinds.

```rust
use vpt_rs::Error;

fn describe_error(err: &Error) {
    match err {
        Error::Timeout { operation, timeout_secs, .. } => {
            eprintln!("timed out after {timeout_secs}s during {operation}");
        }
        Error::CommandFailed { command, status, .. } => {
            eprintln!("command `{command}` failed with exit code {status}");
        }
        Error::MissingPath { path } => {
            eprintln!("file not found: {}", path.display());
        }
        other => {
            eprintln!("unexpected error: {other}");
        }
    }
}
```

## Error Handling Patterns

### Using the `?` operator

Because `Error` implements `From<std::io::Error>`, you can use `?` on any
I/O call inside a function returning `Result<T>`:

```rust
use std::fs;
use vpt_rs::{VolumeRef, Result, SnapshotRequest, SnapshotKind};

fn read_and_snapshot(path: &str) -> Result<()> {
    let _contents = fs::read_to_string(path)?; // io::Error -> Error::Io
    let request = SnapshotRequest {
        source: VolumeRef::new(path),
        kind: SnapshotKind::CrashConsistent,
        label: None,
        read_only: true,
    };
    // backend.create_snapshot(&request)?;
    Ok(())
}
```

### Mapping errors with context

Use `.map_err()` to wrap errors with additional context:

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

### Checking for timeout errors

```rust
use vpt_rs::Error;

fn retry_on_timeout(err: &Error) -> bool {
    err.timeout_secs().is_some()
}
```

:::caution
Do not pattern-match on the `Display` output strings. The string format is
not part of the public API and may change. Always match on the enum variants.
:::
