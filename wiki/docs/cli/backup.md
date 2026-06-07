# vptcli backup

Back up a volume to a stream or image file. The backup command supports both
live-volume and snapshot-based sources, optional temporary snapshot creation,
incremental (parent-based) backups, and configurable block sizes.

## Usage

```
vptcli backup <source> --output <stream-file> [options]
```

## Options

| Flag                    | Required | Default          | Description                                          |
|-------------------------|----------|------------------|------------------------------------------------------|
| `<source>`              | **Yes**  | --               | Source volume identifier                             |
| `--output`              | **Yes**  | --               | Path to the output image/stream file                 |
| `--provider`            | No       | Platform default | Backend provider name (Linux)                        |
| `--snapshot-source`     | No       | Off              | Treat `<source>` as a snapshot ID instead of a volume |
| `--parent-snapshot`     | No       | None             | Parent snapshot ID for incremental backup            |
| `--snapshot-kind`       | No       | `crash`          | Consistency kind for the temporary snapshot          |
| `--snapshot-label`      | No       | None             | Label for the temporary snapshot name                |
| `--snapshot-read-write` | No       | Read-only        | Create a writable temporary snapshot                 |
| `--no-snapshot`         | No       | Off              | Disable temporary snapshot creation                  |
| `--block-size`          | No       | 4 MiB            | I/O block size (see [Block Size](#block-size))       |

## Backup Sources

There are two ways to specify what to back up:

### Live volume (default)

Provide the volume identifier directly. The backend may create a temporary
snapshot before copying, depending on the snapshot policy:

```bash
vptcli backup /mnt/data --output /backup/data.img
```

### Explicit snapshot source

Use `--snapshot-source` to tell the CLI that `<source>` is an existing snapshot
identifier, not a live volume:

```bash
vptcli snapshot create /mnt/data --label "backup"
vptcli backup /mnt/data/.snapshots/backup --output /backup/data.img --snapshot-source
```

## Snapshot Policies

By default, `vptcli backup` tells the backend to create a temporary
crash-consistent snapshot before copying. You can customize this behavior:

| Policy               | Flag combination                            | Behavior                                  |
|----------------------|---------------------------------------------|-------------------------------------------|
| Temporary (default)  | *(no flags)*                                | Create a crash-consistent snapshot        |
| Temporary, app-safe  | `--snapshot-kind application`               | Application-consistent snapshot           |
| Labeled snapshot     | `--snapshot-label "name"`                   | Use a specific label for the snapshot     |
| Writable snapshot    | `--snapshot-read-write`                     | Snapshot is writable (default: read-only) |
| No snapshot          | `--no-snapshot`                             | Use the source as-is, no snapshot created |

:::info
Not all backends support all snapshot kinds. The `application` kind requires
VSS writer coordination on Windows. Backends that do not support a given kind
will return a `MissingCapability` error.
:::

## Block Size

The `--block-size` flag controls the I/O chunk size used by block-level copy
backends (e.g. LVM `dd`-style copy). It accepts a plain number or a number
with a suffix:

| Suffix | Multiplier     | Example    | Result         |
|--------|----------------|------------|----------------|
| *(none)* | 1 (bytes)    | `4194304`  | 4,194,304 bytes |
| `K`    | 1,024          | `4096K`    | 4,194,304 bytes |
| `M`    | 1,048,576      | `4M`       | 4,194,304 bytes |
| `G`    | 1,073,741,824  | `1G`       | 1,073,741,824 bytes |

The suffix is case-insensitive (`4m` and `4M` are equivalent). The value must
be greater than zero and must not overflow `usize`.

:::tip
For most workloads the default 4 MiB block size is a good choice. Increase it
to `8M` or `16M` for large volumes to improve throughput.
:::

## Incremental Backup

For backends that support incremental send (Btrfs, ZFS), use `--parent-snapshot`
to perform an incremental backup. The backend will only transmit the differences
since the parent snapshot:

```bash
# Full backup
vptcli backup /mnt/data --output /backup/data-full.img

# Incremental backup based on a previous snapshot
vptcli backup /mnt/data --output /backup/data-incr.img --parent-snapshot /mnt/data/.snapshots/snap1
```

:::caution
Incremental backups are only supported by stream-based backends (Btrfs `send`,
ZFS `send`). Block-level backends (LVM, VSS) ignore the parent snapshot and
perform a full copy.
:::

## Examples

### Full backup of a Btrfs subvolume

```bash
vptcli backup /mnt/data --output /backup/data.img
```

### Backup with a specific provider

```bash
vptcli backup --provider lvm /dev/vg0/data --output /backup/vg0-data.img
```

### Backup without creating a snapshot

```bash
vptcli backup /mnt/data --output /backup/data.img --no-snapshot
```

### Incremental backup with a labeled snapshot

```bash
vptcli backup /mnt/data \
  --output /backup/data-incr.img \
  --parent-snapshot /mnt/data/.snapshots/nightly \
  --snapshot-label "nightly" \
  --snapshot-kind crash
```

### Large-volume backup with custom block size

```bash
vptcli backup /dev/vg0/largedisk --output /backup/disk.img --block-size 16M
```

### Backup an existing snapshot directly

```bash
vptcli snapshot create /mnt/data --label "pre-migration"
vptcli backup /mnt/data/.snapshots/pre-migration --output /backup/migration.img --snapshot-source
```

## Output

On success, the CLI prints:

```
backend: linux-btrfs
output: /backup/data.img
```

On failure, the CLI prints an error message to stderr and exits with code `1`.

## Error Reference

| Error                       | Common cause                                         |
|-----------------------------|------------------------------------------------------|
| `InvalidArgument`           | Missing `--output`, unknown flag, or invalid block size |
| `MissingPath`               | Source path does not exist                           |
| `MissingCapability`         | Snapshot kind not supported by the backend           |
| `CommandFailed`             | External tool (btrfs, lvcreate, zfs) returned an error |
| `Timeout`                   | External tool exceeded `VPT_COMMAND_TIMEOUT_SECS`     |
