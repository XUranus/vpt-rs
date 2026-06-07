---
sidebar_position: 1
title: Installation
description: How to install vpt-rs and its CLI tool on Linux, Windows, and macOS.
---

# Installation

This guide walks you through installing **vpt-rs** and its command-line tool `vptcli`. By the end you will have a working binary ready to create snapshots and run backups.

---

## Prerequisites

### Rust Toolchain

vpt-rs is written in Rust. You need a working Rust installation (1.82+ recommended).

```bash
# Install rustup if you don't have it yet
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Verify installation
rustc --version
cargo --version
```

:::tip
After installing rustup, open a new terminal or run `source $HOME/.cargo/env` so the `cargo` command is available.
:::

### System Packages by Platform

vpt-rs delegates snapshot and backup operations to the native storage tools on your platform. You must install the correct system packages **before** building or running the CLI.

| Provider | Platform | Required Packages | Install Command |
|----------|----------|-------------------|-----------------|
| **Btrfs** | Linux | `btrfs-progs` | `sudo apt install btrfs-progs` or `sudo pacman -S btrfs-progs` |
| **LVM** | Linux | `lvm2` | `sudo apt install lvm2` or `sudo pacman -S lvm2` |
| **ZFS** | Linux | `zfsutils-linux` | `sudo apt install zfsutils-linux` or `sudo pacman -S zfs-utils` |
| **VSS** | Windows | Built-in (wmic/vssadmin) | No extra packages needed |
| **APFS** | macOS | Not yet implemented | -- |

:::caution
On Linux, `vptcli` calls the underlying storage tools (`btrfs`, `lvs`, `zfs`, etc.) as external processes. If the tool is not installed, the corresponding backend will fail with a `CommandFailed` error at runtime.
:::

---

## Install from Source (Recommended)

The simplest way to get the binary is `cargo install` from the project root. This compiles the project and places `vptcli` in your Cargo bin directory.

```bash
cd /path/to/vpt-rs
cargo install --path .
```

After this completes, verify it is on your `PATH`:

```bash
which vptcli
# Expected: /home/<you>/.cargo/bin/vptcli
```

---

## Build Manually

If you prefer not to install globally, build a release binary directly:

```bash
cd /path/to/vpt-rs
cargo build --release
```

The binary is produced at:

```
target/release/vptcli
```

You can run it with its full path or copy it somewhere on your `PATH`:

```bash
# Option A: run with full path
./target/release/vptcli --help

# Option B: copy to a local bin directory
mkdir -p ~/.local/bin
cp target/release/vptcli ~/.local/bin/
```

---

## Verify the Installation

Run the built-in help command to confirm everything works:

```bash
vptcli --help
```

Expected output:

```
vptcli <command> [args]

Commands:
  snapshot    Create, list, delete snapshots; query backends and capabilities
  backup      Back up a volume to a stream or image file
  restore     Restore a volume from a stream or image file

Run `vptcli <command>` with no args for subcommand usage.
```

You can also check what backends are available on your system:

```bash
vptcli snapshot backend list
```

:::note
On Linux this always lists btrfs, lvm, and zfs backends. On other platforms only the platform-native backend is shown.
:::

---

## Build with Windows VSS Support (Optional)

On Windows, you can enable the experimental VSS (Volume Shadow Copy) feature by passing the feature flag:

```bash
cargo build --release --features windows-vss
```

This pulls in the `windows` crate and compiles native COM FFI code for VSS requestor support. Without this flag the Windows backend falls back to CLI-only snapshot management via `wmic` and `vssadmin`.

---

## Enable Debug Logging

vpt-rs uses the Rust `tracing` crate. Set the `RUST_LOG` environment variable to see detailed output:

```bash
# Show debug-level messages from vpt-rs
RUST_LOG=vpt_rs=debug vptcli snapshot backend list

# Show trace-level messages (very verbose)
RUST_LOG=trace vptcli backup --provider btrfs --output /tmp/test.stream /mnt/data
```

---

## Platform-Specific Notes

### Linux

All three Linux providers (btrfs, lvm, zfs) are compiled in unconditionally. The `--provider` flag selects which one to use at runtime. The default provider is **btrfs**.

```bash
# Explicitly select a provider
vptcli snapshot capabilities --provider lvm
vptcli snapshot capabilities --provider zfs
```

Root privileges are typically required for snapshot operations:

```bash
sudo vptcli snapshot create --provider btrfs /mnt/data/subvol
```

### Windows

The default Windows backend uses `wmic` and `vssadmin` CLI tools that ship with Windows. No additional packages are required. For the COM-based VSS requestor, build with `--features windows-vss`.

### macOS

A stub backend exists for macOS (targeting APFS), but snapshot and backup operations are not yet implemented. The architecture is in place for future development.

---

## Using vpt-rs as a Library

If you are integrating vpt-rs into your own Rust project instead of using the CLI, add it as a dependency. Since vpt-rs is not published on crates.io yet, use a path or git dependency:

```toml
# In your Cargo.toml
[dependencies]
vpt-rs = { path = "/path/to/vpt-rs" }
```

Or for a git dependency:

```toml
[dependencies]
vpt-rs = { git = "https://github.com/xuranus/vpt-rs.git" }
```

Once added, you can use the library API directly:

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
When using vpt-rs as a library, the `windows-vss` feature must be enabled explicitly in your `Cargo.toml` if you need VSS support on Windows:

```toml
[dependencies]
vpt-rs = { path = "/path/to/vpt-rs", features = ["windows-vss"] }
```
:::

---

## Project Structure at a Glance

Understanding the source layout helps when navigating the codebase or filing issues:

```
vpt-rs/
  Cargo.toml            Package manifest
  src/
    lib.rs              Public API re-exports
    bin/vptcli.rs       CLI binary entry point
    types.rs            Core types (VolumeRef, BackupPlan, etc.)
    snapshot.rs         SnapshotProvider trait
    backup.rs           BackupExecutor trait
    restore.rs          RestorePlanner trait
    error.rs            Error enum (thiserror)
    platform/
      linux/
        btrfs.rs        Btrfs backend (send/receive)
        lvm.rs          LVM backend (block-level copy)
        zfs.rs          ZFS backend (send/receive)
      windows.rs        Windows VSS backend
  tests/                Python-based integration tests
```

---

## Troubleshooting

**`cargo build` fails with "linker not found"**
: Install the C toolchain: `sudo apt install build-essential` (Debian/Ubuntu) or `sudo pacman -S base-devel` (Arch).

**`vptcli: command not found` after `cargo install`**
: Ensure `~/.cargo/bin` is on your `PATH`. Add this to your shell profile:
  ```bash
  export PATH="$HOME/.cargo/bin:$PATH"
  ```

**`CommandFailed` when running snapshot operations**
: The underlying storage tool is not installed. Refer to the [system packages table](#system-packages-by-platform) above.

**Permission denied errors**
: Most snapshot and backup operations require root privileges. Use `sudo` or run as root.

---

## Next Steps

Once installation is verified, proceed to the [Quick Start](./quick-start.md) guide to create your first backup in under five minutes.
