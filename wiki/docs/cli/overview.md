# CLI Overview

`vptcli` is the command-line interface for the **vpt-rs** volume backup toolkit. It exposes the core library operations -- snapshot management, backup, and restore -- as a single binary with three top-level subcommands.

## Binary

```
vptcli <command> [args]
```

Run `vptcli` with no arguments (or pass `help`) to see the available commands.

## Subcommands

| Subcommand  | Description                                               |
|-------------|-----------------------------------------------------------|
| `snapshot`  | Create, list, and delete provider-managed snapshots       |
| `backup`    | Back up a volume to a stream or image file                |
| `restore`   | Restore a volume from a stream or image file              |

Every subcommand accepts `--help` or `-h` to print its own usage.

## Common Options

The `--provider` flag appears across all subcommands. On Linux, it selects the
snapshot provider backend (`btrfs`, `lvm`, or `zfs`). On other platforms the
flag is accepted but must match the platform's native backend name.

| Flag          | Description                                  |
|---------------|----------------------------------------------|
| `--provider`  | Select a backend provider by name            |
| `--help`      | Show usage for the current subcommand        |

## Environment Variables

| Variable                    | Default | Description                                                 |
|-----------------------------|---------|-------------------------------------------------------------|
| `RUST_LOG`                  | `vpt_rs=info` | Tracing log filter (see `tracing-subscriber`)         |
| `VPT_COMMAND_TIMEOUT_SECS`  | `30`    | Timeout in seconds for external commands invoked by backends |

### Setting the log level

```bash
# Show debug-level tracing output
RUST_LOG=debug vptcli snapshot backend

# Trace a specific module
RUST_LOG=vpt_rs::platform::linux::btrfs=trace vptcli snapshot list /mnt/data
```

### Adjusting the command timeout

Some backends invoke external tools (e.g. `btrfs`, `lvcreate`, `zfs`). The
default timeout is 30 seconds. Increase it for large volumes:

```bash
VPT_COMMAND_TIMEOUT_SECS=300 vptcli backup /dev/vg0/data --output /backup/data.img
```

## Quick Reference

| Task                          | Command                                                                            |
|-------------------------------|------------------------------------------------------------------------------------|
| Show backend info             | `vptcli snapshot backend`                                                          |
| List all backends (Linux)     | `vptcli snapshot backend list`                                                     |
| Query capabilities            | `vptcli snapshot capabilities`                                                     |
| Create a snapshot             | `vptcli snapshot create /mnt/data`                                                  |
| List snapshots                | `vptcli snapshot list /mnt/data`                                                    |
| Delete a snapshot             | `vptcli snapshot delete <snapshot-id>`                                              |
| Full backup                   | `vptcli backup /mnt/data --output /backup/data.img`                                 |
| Backup with snapshot          | `vptcli backup /mnt/data --output /backup/data.img --snapshot-kind crash`           |
| Incremental backup            | `vptcli backup /mnt/data --output /backup/data.img --parent-snapshot <snap-id>`     |
| Restore from image            | `vptcli restore /mnt/data --input /backup/data.img`                                 |
| Force-restore (destructive)   | `vptcli restore /dev/vg0/data --input /backup/data.img --force`                     |

## Platform Backends

On Linux, `vptcli` auto-detects the best available backend. The default is
`btrfs`. You can override it with `--provider`:

```bash
# Explicitly use the LVM backend
vptcli --provider lvm snapshot create /dev/vg0/data

# Explicitly use ZFS
vptcli --provider zfs backup tank/dataset --output /backup/tank.img
```

:::tip
On non-Linux platforms (macOS, Windows), the backend is fixed to the
platform-native snapshot mechanism. The `--provider` flag is accepted only if
it matches the native backend name.
:::

## Error Handling

`vptcli` prints errors to stderr and exits with code `1` on failure. Error
messages include the backend name, the operation that failed, and structured
context:

```
error: command `btrfs subvolume snapshot` failed with status 1: ERROR: cannot snapshot ...
```

:::info
Increase `RUST_LOG=debug` to see the full command line and internal tracing
output when diagnosing failures.
:::
