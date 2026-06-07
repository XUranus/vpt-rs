# vptcli snapshot

Manage provider-managed snapshots: create, list, delete them, and inspect
backend capabilities.

## Usage

```
vptcli snapshot <command> [args]
```

Run `vptcli snapshot` with no arguments to see the available subcommands.

## Subcommands

### backend

Show information about the current backend.

```
vptcli snapshot backend
```

**Output fields:**

| Field      | Description                                  |
|------------|----------------------------------------------|
| `platform` | Operating system (e.g. `linux`, `windows`)   |
| `provider` | Backend provider name (Linux only)           |
| `backend`  | Canonical backend name                       |

**Example:**

```bash
$ vptcli snapshot backend
platform: linux
provider: btrfs
backend: linux-btrfs
```

### backend list

List all backends available on the current platform. On Linux this shows
`btrfs`, `lvm`, and `zfs`. On other platforms only the native backend is listed.

```
vptcli snapshot backend list
```

**Example:**

```bash
$ vptcli snapshot backend list
platform: linux
provider: btrfs
backend: linux-btrfs
platform: linux
provider: lvm
backend: linux-lvm
platform: linux
provider: zfs
backend: linux-zfs
```

### capabilities

Print the capabilities of the selected backend.

```
vptcli snapshot capabilities [--provider <name>]
```

| Flag          | Required | Description                     |
|---------------|----------|---------------------------------|
| `--provider`  | No       | Backend provider name (Linux)   |

**Example:**

```bash
$ vptcli snapshot capabilities
linux-btrfs
- crash_consistent_snapshot
- incremental_send
- read_only_snapshot_mount
- writable_snapshot_mount
```

### create

Create a new snapshot of a volume.

```
vptcli snapshot create [--provider <name>] <volume> [--kind crash|application] [--label <name>] [--read-write]
```

| Flag           | Required | Default            | Description                                |
|----------------|----------|--------------------|--------------------------------------------|
| `--provider`   | No       | Platform default   | Backend provider name                      |
| `<volume>`     | **Yes**  | --                 | Source volume identifier                   |
| `--kind`       | No       | `crash`            | Snapshot consistency kind                  |
| `--label`      | No       | None               | Human-readable label for the snapshot name |
| `--read-write` | No       | Read-only          | Create a writable snapshot                 |

**Snapshot kinds:**

- `crash` -- Crash-consistent (filesystem-consistent, no application quiescing). Supported by all backends.
- `application` -- Application-consistent. Coordinates with VSS writers on Windows. May not be available on all backends.

**Output fields:**

| Field      | Description                                  |
|------------|----------------------------------------------|
| `snapshot` | Provider-specific snapshot identifier        |
| `source`   | Source volume (if reported)                  |
| `backend`  | Backend that created the snapshot            |
| `path`     | Filesystem path hint (if available)          |

**Examples:**

```bash
# Create a crash-consistent snapshot of a Btrfs subvolume
$ vptcli snapshot create /mnt/data
snapshot: /mnt/data/.snapshots/snapshot-20240101
source: /mnt/data
backend: linux-btrfs
path: /mnt/data/.snapshots/snapshot-20240101

# Create a labeled, writable snapshot
$ vptcli snapshot create /mnt/data --label "pre-upgrade" --read-write

# Use a specific provider
$ vptcli snapshot create --provider lvm /dev/vg0/data --kind crash

# Application-consistent snapshot (Windows VSS)
$ vptcli snapshot create C: --kind application
```

### list

List all snapshots managed by this backend for a given volume.

```
vptcli snapshot list [--provider <name>] <volume>
```

| Flag          | Required | Description                     |
|---------------|----------|---------------------------------|
| `--provider`  | No       | Backend provider name (Linux)   |
| `<volume>`    | **Yes**  | Volume identifier               |

**Output format:**

Each line prints: `<snapshot-id> <source-id> <backend>`

**Example:**

```bash
$ vptcli snapshot list /mnt/data
/mnt/data/.snapshots/snap1 /mnt/data linux-btrfs
/mnt/data/.snapshots/snap2 /mnt/data linux-btrfs
```

### delete

Delete a snapshot by its identifier.

```
vptcli snapshot delete [--provider <name>] <snapshot-id>
```

| Flag            | Required | Description                     |
|-----------------|----------|---------------------------------|
| `--provider`    | No       | Backend provider name (Linux)   |
| `<snapshot-id>` | **Yes**  | Snapshot identifier to delete   |

:::caution
Deleting a snapshot is irreversible. Make sure no backup or restore operations
are referencing the snapshot before deleting it.
:::

**Example:**

```bash
$ vptcli snapshot delete /mnt/data/.snapshots/snap1
```

## Volume Identifiers

The format of the `<volume>` argument depends on the backend:

| Backend | Example identifier              |
|---------|---------------------------------|
| Btrfs   | `/mnt/data/subvol`              |
| LVM     | `/dev/vg0/data`                 |
| ZFS     | `tank/data`                     |
| Windows | `C:` or a volume GUID path      |

:::tip
Use `vptcli snapshot backend` to check which backend is active, then refer to
the table above to construct the correct volume identifier.
:::
