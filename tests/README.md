# vpt-rs Integration Tests

Python-based integration tests that exercise `vptcli` CLI tool against real
storage providers. Linux tests use loop devices; Windows tests use VHD files.

## Structure

```
tests/
├── env.py              # Shared helpers: root check, command check, CLI wrappers, logging
├── test_btrfs.py       # Btrfs snapshot → backup → restore → verify roundtrip
├── test_lvm.py         # LVM snapshot → backup → restore → verify roundtrip
├── test_zfs.py         # ZFS snapshot → backup → restore → verify roundtrip
├── test_vss.py         # Windows VSS snapshot → backup → restore → verify roundtrip
├── test_smoke.py       # CLI smoke tests (no root, cross-platform)
├── run_all.py          # Runner for all tests with per-test timeout
└── README.md           # This file
```

## Prerequisites

### 1. Build the binaries

```bash
cargo build --release
```

### 2. System packages

Install all dependencies in one shot:

```bash
sudo apt-get install -y btrfs-progs lvm2 zfsutils-linux
```

Or install only what you need per provider:

| Provider | Package (Debian/Ubuntu)   | Required commands                                      |
|----------|---------------------------|--------------------------------------------------------|
| btrfs    | `btrfs-progs`             | `mkfs.btrfs`, `btrfs`                                 |
| lvm      | `lvm2`                    | `pvcreate`, `vgcreate`, `lvcreate`, `lvremove`, `vgremove`, `pvremove`, `mkfs.ext4` |
| zfs      | `zfsutils-linux`          | `zpool`, `zfs`                                        |
| all      | `util-linux`              | `losetup`, `truncate` (usually pre-installed)          |

If a required command is missing, the test will print the exact missing
commands and the package to install before exiting.

**ZFS note:** `zfsutils-linux` also requires the ZFS kernel module. Verify it
is loaded before running ZFS tests:

```bash
lsmod | grep zfs
# If missing:
sudo modprobe zfs
```

On kernels without built-in ZFS support (e.g. ARM/Rockchip boards), the ZFS
tests will fail even with the package installed. Btrfs and LVM tests are not
affected.

### 3. Root / Administrator privileges

Provider tests (btrfs, lvm, zfs) require root to create loop devices, mount
filesystems, and manage LVM/ZFS pools. Run with `sudo`.

The Windows VSS test requires Administrator privileges (elevated PowerShell
or cmd). It automatically checks for elevation and exits with a clear message
if not running as admin.

Smoke tests do not require root or admin.

### 4. Windows prerequisites

The VSS test (`test_vss.py`) requires:
- Windows 10+ (diskpart for VHD management, no Hyper-V module needed)
- VSS service running (the test starts it automatically if stopped)
- vptcli built with: `cargo build --release --features windows-vss`

No additional packages are needed — `diskpart`, `vssadmin`, and `wmic` are
built into Windows.

## Running tests

### All tests

```bash
sudo python3 tests/run_all.py
```

### A single provider

```bash
sudo python3 tests/test_btrfs.py
sudo python3 tests/test_lvm.py
sudo python3 tests/test_zfs.py
```

### Windows VSS test (from elevated PowerShell)

```powershell
# Build first
cargo build --release --features windows-vss

# Run test
python tests\test_vss.py

# Keep VHD files and logs after test
python tests\test_vss.py --keep

# Custom data directory
python tests\test_vss.py --data-root D:\vpt-debug --keep
```

### Smoke tests only (no root needed)

```bash
python3 tests/test_smoke.py
```

### Selective providers via the runner

```bash
sudo python3 tests/run_all.py --providers btrfs,smoke
sudo python3 tests/run_all.py --providers lvm
```

### Build and test in one step

```bash
sudo python3 tests/run_all.py --build
```

## Configuration

All settings are controlled via environment variables or CLI flags.

### Environment variables

| Variable             | Default                       | Description                                |
|----------------------|-------------------------------|--------------------------------------------|
| `TEST_DATA_ROOT`     | `/tmp/testvolumedata` (Linux) | Root directory for images, streams, logs   |
|                      | `C:\temp\vpt-test` (Windows)  |                                            |
| `TEST_MOUNT_ROOT`    | `/tmp/testvolumemnt`          | Root directory for mount points (Linux)    |
| `TEST_ID`            | *(auto-generated UUID)*       | Test run identifier for artifact isolation |
| `TEST_CLEANUP`       | `1`                           | `1` to remove mount dirs after test        |
| `TEST_KEEP_ARTIFACTS`| `0`                           | `1` to keep image/VHD/stream files after test |
| `VPT_PROJECT_ROOT`   | *(auto-detected)*             | Path to project root (contains Cargo.toml) |
| `RUST_LOG`           | `vpt_rs=debug`                | Log level for CLI tools (tracing)          |

### Runner CLI flags

| Flag               | Environment equivalent   | Description                          |
|--------------------|--------------------------|--------------------------------------|
| `--providers LIST` | —                        | Comma-separated providers to run     |
| `--data-root PATH` | `TEST_DATA_ROOT`         | Override data directory              |
| `--mount-root PATH`| `TEST_MOUNT_ROOT`        | Override mount directory             |
| `--keep`           | `TEST_KEEP_ARTIFACTS=1`  | Keep images and streams after test   |
| `--no-cleanup`     | `TEST_CLEANUP=0`         | Keep mount directories after test    |
| `--build`          | —                        | Run `cargo build --release` first    |
| `--timeout N`      | —                        | Per-test timeout in seconds (def: 180)|

`test_vss.py` also accepts these flags when run directly:

| Flag                 | Environment equivalent   | Description                          |
|----------------------|--------------------------|--------------------------------------|
| `--keep`             | `TEST_KEEP_ARTIFACTS=1`  | Keep VHD files, backup image, logs   |
| `--data-root PATH`   | `TEST_DATA_ROOT`         | Override VHD/log output directory    |

## Test isolation

Each test run is identified by a UUID. All artifacts are stored under
`<TEST_DATA_ROOT>/<uuid>/` (Linux) or `<TEST_DATA_ROOT>/<uuid>\` (Windows):

Linux:
```
/tmp/testvolumedata/ab12cd34/
├── logs/
│   ├── btrfs.log          # Python test log (step-by-step with timestamps)
│   ├── cli.log            # CLI tool tracing output (RUST_LOG=debug)
│   ├── lvm.log
│   └── zfs.log
├── btrfs.img              # Loop device image (sparse file)
├── btrfs.stream           # btrfs send backup stream
├── lvm.img                # Loop device image for LVM PV
├── lvm-stream.img         # dd block-level backup
├── zfs.img                # Loop device image for zpool
└── zfs.stream             # zfs send backup stream
```

Windows (VSS):
```
C:\temp\vpt-test\ab12cd34\
├── logs\
│   ├── vss.log            # Python test log
│   └── diskpart.log       # All diskpart commands with output
├── source.vhd             # Source VHD (256 MB fixed)
├── target.vhd             # Target VHD for restore verification
└── backup.img             # Block-level volume backup image
```

## Viewing logs

### Test execution log (Python)

Each test writes a Python log file with step-by-step output and timestamps:

```
<DATA_ROOT>/<uuid>/logs/btrfs.log
```

The path is printed at the start of each test:

```
16:09:34 [INFO] Log file: /tmp/testvolumedata/25ebd746/logs/btrfs.log
```

### CLI tool tracing log (Rust)

Every `vptcli` call emits tracing output
(via `RUST_LOG=debug`) to a dedicated log file:

```
<DATA_ROOT>/<uuid>/logs/cli.log
```

This log captures the internal execution details: which commands the backend
runs, snapshot planning, send/receive operations, and error diagnostics. Each
CLI invocation is separated by a header showing the command:

```
============================================================
$ vptcli backup --provider btrfs --output /tmp/.../btrfs.stream /tmp/.../btrfs/source-subvol
============================================================
 2026-05-27T08:21:03.456789Z  INFO vpt_rs::platform::linux::btrfs: create_snapshot called
 2026-05-27T08:21:03.456890Z DEBUG vpt_rs::process: starting external command command="btrfs subvolume snapshot -r ..."
 ...
```

## Inspecting intermediate volume data

### During a test run

Run with `--keep` and `--no-cleanup` to preserve all artifacts after the test
finishes. Then inspect the mount points while the volumes are still mounted:

```bash
sudo python3 tests/test_btrfs.py
# Test cleans up and exits. Data is gone.
```

```bash
# Keep everything: images, streams, AND mount directories
TEST_KEEP_ARTIFACTS=1 TEST_CLEANUP=0 sudo python3 tests/test_btrfs.py
```

After the test (with `--keep --no-cleanup`), the mount points still exist and
contain the restored data:

```bash
# View restored btrfs subvolume
ls -la /tmp/testvolumemnt/<uuid>/btrfs/restore-root/

# View restored LVM filesystem
mount /dev/vptvg-<uuid>/restore /mnt/tmp
ls -la /mnt/tmp/
umount /mnt/tmp
```

### Pin a UUID for reproducible debugging

Linux:
```bash
TEST_ID=debug123 TEST_KEEP_ARTIFACTS=1 TEST_CLEANUP=0 \
  sudo python3 tests/test_btrfs.py
```

Windows:
```powershell
$env:TEST_ID = "debug123"
python tests\test_vss.py --keep
```

This creates artifacts at known paths:

```
Linux:
/tmp/testvolumedata/debug123/logs/btrfs.log    # test log
/tmp/testvolumedata/debug123/logs/cli.log      # CLI tracing
/tmp/testvolumedata/debug123/btrfs.img         # volume image

Windows:
C:\temp\vpt-test\debug123\logs\vss.log         # test log
C:\temp\vpt-test\debug123\logs\diskpart.log    # diskpart commands
C:\temp\vpt-test\debug123\source.vhd           # source VHD
C:\temp\vpt-test\debug123\backup.img           # backup image
```

### Use the runner

```bash
# Run all tests, keep everything
sudo python3 tests/run_all.py --keep --no-cleanup

# Run only btrfs, pin UUID, keep everything
TEST_ID=debug sudo python3 tests/run_all.py --providers btrfs --keep --no-cleanup
```

## What each test does

Each provider test follows the same 11-step lifecycle:

```
1. Volume init     Create loop device + provider-specific volume structure
2. Mount           Mount the source volume
3. Write data      Create 3 files: hello.txt, data.txt, sub/nested.txt
4. Snapshot create vptcli snapshot create --provider <P>
5. Snapshot list   vptcli snapshot list --provider <P> (assert in list)
6. Backup          vptcli backup --provider <P> (assert stream file exists, non-empty)
7. Restore         vptcli restore --provider <P> (assert exit 0)
8. Mount restored  Mount the restored volume for verification
9. Verify files    Read all 3 files, assert content matches source
10. Snapshot delete vptcli snapshot delete (assert gone from list)
11. Teardown       Unmount, detach loop, cleanup LVs/pools
```

### btrfs (`test_btrfs.py`)

- Volume init: `truncate` → loop → `mkfs.btrfs -f` → `mount` → `btrfs subvolume create`
- Backup: `vptcli backup` auto-creates temp snapshot, runs `btrfs send`, cleans up temp snapshot
- Restore: `vptcli restore` runs `btrfs receive` into restore directory
- Verify: `rglob("*.txt")` to find files in received subvolume

### lvm (`test_lvm.py`)

- Volume init: `truncate` → loop → `pvcreate` → `vgcreate` → 2x `lvcreate -L 512M` → `mkfs.ext4`
- Backup: `vptcli backup` auto-creates LVM snapshot, runs `dd` to image file, cleans up snapshot
- Restore: `vptcli restore --force` runs `dd` image into destination LV
- Verify: `mount` restored LV, `cat` files, `umount`

### zfs (`test_zfs.py`)

- Volume init: `truncate` → loop → `zpool create -f` → 2x `zfs create -o mountpoint=...`
- Backup: `vptcli backup --snapshot-source` runs `zfs send` on explicit snapshot
- Restore: `vptcli restore --force` runs `zfs receive -F` into restore dataset
- Verify: `cat` files in auto-mounted dataset

### smoke (`test_smoke.py`)

- `vptcli snapshot backend list` — returns platform and provider info
- `vptcli snapshot capabilities --provider btrfs|lvm|zfs` — lists capabilities
- `vptcli snapshot` / `vptcli backup` / `vptcli restore` with no args — shows usage (exit 0)
- Invalid provider — returns non-zero exit code

### vss (`test_vss.py`) — Windows only

```powershell
python tests\test_vss.py [--keep] [--data-root PATH]
```

- Volume init: `diskpart create vdisk` → `attach vdisk` → `create partition primary` → `format fs=ntfs`
- Write data: 3 files via Python `Path.write_text()`: hello.txt, data.txt, sub/nested.txt
- Snapshot create: `vptcli snapshot create --provider windows-vss` (COM API → wmic fallback)
- Snapshot list: `vptcli snapshot list --provider windows-vss`
- Backup: `vptcli backup --provider windows-vss` (COM snapshot → direct volume copy fallback)
- Restore: detach target VHD, raw block copy of backup.img into target VHD file, re-mount
- Verify: `Path.read_text()` on all 3 files in restored volume, assert content matches
- Snapshot delete: `vptcli snapshot delete --provider windows-vss` (COM → vssadmin fallback)
- Teardown: `diskpart detach vdisk` → delete VHD files (unless `--keep`)

**Notes:**
- VSS snapshot operations use COM API (`IVssBackupComponents`) with automatic fallback
  to `wmic`/`vssadmin` CLI commands when COM is unavailable
- VHD volumes on desktop Windows (Home/Pro) don't support VSS snapshots natively;
  the backup falls back to direct block-level volume copy
- The `--keep` flag preserves all VHD files, backup image, and logs for debugging

## Coverage matrix

| Step               | btrfs | lvm | zfs | vss | smoke |
|--------------------|:-----:|:---:|:---:|:---:|:-----:|
| Volume init        |   ✓   |  ✓  |  ✓  |  ✓  |       |
| Mount              |   ✓   |  ✓  |  ✓  |  ✓  |       |
| Write data         |   ✓   |  ✓  |  ✓  |  ✓  |       |
| Snapshot create    |   ✓   |  ✓  |  ✓  |  ✓  |       |
| Snapshot list      |   ✓   |  ✓  |  ✓  |  ✓  |       |
| Backup             |   ✓   |  ✓  |  ✓  |  ✓  |       |
| Restore            |   ✓   |  ✓  |  ✓  |  ✓  |       |
| Mount restored     |   ✓   |  ✓  |  ✓  |  ✓  |       |
| Verify files       |   ✓   |  ✓  |  ✓  |  ✓  |       |
| Snapshot delete    |   ✓   |  ✓  |  ✓  |  ✓  |       |
| Teardown/cleanup   |   ✓   |  ✓  |  ✓  |  ✓  |       |
| CLI usage output   |       |     |     |     |   ✓   |
| Backend list       |       |     |     |     |   ✓   |
| Capabilities query |       |     |     |     |   ✓   |
| Invalid provider   |       |     |     |     |   ✓   |

- **btrfs**: loop → `mkfs.btrfs` → subvolume → `btrfs send`/`btrfs receive`
- **lvm**: loop → `pvcreate` → `vgcreate` → `lvcreate` → `dd` block copy
- **zfs**: loop → `zpool create` → `zfs create` → `zfs send`/`zfs receive`
- **vss**: `diskpart create vdisk` → NTFS → VSS snapshot (COM/wmic) → block copy
- **smoke**: CLI argument validation, no root required
