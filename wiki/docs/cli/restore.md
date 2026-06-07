# vptcli restore

Restore a volume from a backup stream or image file created by `vptcli backup`.

## Usage

```
vptcli restore <destination> --input <stream-file> [options]
```

## Options

| Flag               | Required | Default          | Description                                         |
|--------------------|----------|------------------|-----------------------------------------------------|
| `<destination>`    | **Yes**  | --               | Target volume or directory to restore into          |
| `--input`          | **Yes**  | --               | Path to the backup image/stream file                |
| `--provider`       | No       | Platform default | Backend provider name (Linux)                       |
| `--force`          | No       | Off              | Allow destructive restore on block-level backends   |
| `--base-snapshot`  | No       | None             | Base snapshot reference for incremental restore     |
| `--block-size`     | No       | 4 MiB            | I/O block size for block-level copy                 |

## Force Flag

Some backends perform **destructive** restores -- they overwrite the target
volume with the contents of the backup. For safety, these backends require the
`--force` flag:

- **LVM**: Overwrites the logical volume with `dd`-style block copy.
- **VSS (Windows)**: Writes blocks directly to the target volume.

Stream-based backends (Btrfs `receive`, ZFS `receive`) create new subvolumes or
datasets and do **not** require `--force`.

:::danger
Using `--force` on a block-level backend **destroys all existing data** on the
target volume. Double-check the destination path before running the command.
:::

**Without `--force` on a destructive backend:**

```bash
$ vptcli restore --provider lvm /dev/vg0/data --input /backup/data.img
error: invalid argument: `--force` is required for destructive restore on backend `linux-lvm`
```

**With `--force`:**

```bash
$ vptcli restore --provider lvm /dev/vg0/data --input /backup/data.img --force
backend: linux-lvm
input: /backup/data.img
```

## Block Size

The `--block-size` flag works the same as in `vptcli backup`. It controls the
I/O chunk size for block-level copy backends:

| Suffix | Multiplier     | Example  |
|--------|----------------|----------|
| *(none)* | 1 (bytes)    | `4194304` |
| `K`    | 1,024          | `4096K`  |
| `M`    | 1,048,576      | `4M`     |
| `G`    | 1,073,741,824  | `1G`     |

See the [Backup -- Block Size](./backup.md#block-size) section for full details.

## Incremental Restore

Some backends support incremental restore workflows where you provide a base
snapshot that the backup was diffed against:

```bash
vptcli restore /mnt/data \
  --input /backup/data-incr.img \
  --base-snapshot /mnt/data/.snapshots/snap1
```

:::info
The `--base-snapshot` flag is currently reserved for backends that need an
explicit base reference during incremental receive. Most backends determine the
base automatically from the stream metadata.
:::

## Examples

### Restore a Btrfs subvolume from a stream

```bash
vptcli restore /mnt/restored --input /backup/data.img
```

The Btrfs backend creates a new subvolume at `/mnt/restored` from the received
stream.

### Restore with an explicit provider

```bash
vptcli restore --provider zfs tank/restored --input /backup/tank.img
```

### Force-restore to an LVM logical volume

```bash
vptcli restore --provider lvm /dev/vg0/restored --input /backup/data.img --force
```

### Restore with a custom block size

```bash
vptcli restore --provider lvm /dev/vg0/restored --input /backup/data.img --force --block-size 8M
```

### Restore to a ZFS dataset

```bash
vptcli restore --provider zfs tank/restored --input /backup/tank-data.img
```

## Output

On success, the CLI prints:

```
backend: linux-btrfs
input: /backup/data.img
```

On failure, the CLI prints an error to stderr and exits with code `1`.

## Error Reference

| Error                 | Common cause                                          |
|-----------------------|-------------------------------------------------------|
| `InvalidArgument`     | Missing `--input`, missing destination, or `--force` required but not given |
| `MissingPath`         | Backup file specified by `--input` does not exist     |
| `UnsupportedOperation`| Backend does not support restore                      |
| `CommandFailed`       | External tool returned an error                       |
| `Timeout`             | External tool exceeded `VPT_COMMAND_TIMEOUT_SECS`      |

## Comparison: Stream vs Block-Level Restore

| Aspect              | Stream-based (Btrfs, ZFS)           | Block-level (LVM, VSS)          |
|---------------------|--------------------------------------|----------------------------------|
| `--force` required  | No                                   | Yes                              |
| Destination         | New subvolume/dataset path           | Existing device path             |
| Existing data       | Preserved (new subvolume created)    | Destroyed                        |
| `--block-size` used | No (stream receive)                  | Yes                              |
| `--base-snapshot`   | Usually auto-detected from stream    | Reserved for future use          |
