# Integration Tests

The repository includes Python-based integration tests under `tests/` that exercise the full backup/restore lifecycle against real storage backends.

## Available Tests

| Test File | Provider | Root Required | Description |
|-----------|----------|---------------|-------------|
| `tests/test_smoke.py` | all | No | CLI smoke tests: backend list, capabilities, usage output, invalid provider rejection |
| `tests/test_btrfs.py` | btrfs | Yes | Full Btrfs roundtrip on loopback filesystem |
| `tests/test_lvm.py` | lvm | Yes | Full LVM roundtrip on loopback PV/VG/LV |
| `tests/test_zfs.py` | zfs | Yes | Full ZFS roundtrip on file-backed zpool |
| `tests/test_vss.py` | vss | Yes (Windows) | Full VSS roundtrip on VHD files |

## Test Runner

`tests/run_all.py` runs all provider tests with configurable options:

```bash
# Run all tests (requires root)
sudo python3 tests/run_all.py

# Run specific providers
sudo python3 tests/run_all.py --providers btrfs,lvm

# Build before testing
sudo python3 tests/run_all.py --build

# Keep artifacts after test
sudo python3 tests/run_all.py --providers btrfs --keep

# Custom timeout (seconds)
sudo python3 tests/run_all.py --timeout 300
```

## Smoke Tests

Smoke tests require no root privileges and work on all platforms:

```bash
python3 tests/test_smoke.py
```

These tests verify:
- Backend listing (`vptcli snapshot backend list`)
- Capability reporting per provider
- Usage output for snapshot/backup/restore subcommands
- Invalid provider rejection

## Provider Roundtrip Tests

Each provider test follows an 11-step lifecycle:

1. **Volume init** — create loop device / zpool / VHD, format filesystem
2. **Mount** — mount the volume
3. **Write data** — create 3 test files with known content
4. **Snapshot create** — `vptcli snapshot create`
5. **Snapshot list** — verify snapshot appears in `vptcli snapshot list`
6. **Backup** — `vptcli backup` to stream/image file
7. **Restore** — `vptcli restore` to destination volume
8. **Mount restored** — mount the restored volume
9. **Verify files** — check all 3 files match original content
10. **Snapshot delete** — `vptcli snapshot delete`, verify removal
11. **Teardown** — unmount, detach loop device, remove artifacts

## Configuration

Environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `TEST_DATA_ROOT` | `/tmp/testvolumedata` | Root for test data files |
| `TEST_MOUNT_ROOT` | `/tmp/testvolumemnt` | Root for mount points |
| `TEST_ID` | auto-generated UUID | Test isolation namespace |
| `TEST_CLEANUP` | `1` | Set to `0` to skip cleanup |
| `TEST_KEEP_ARTIFACTS` | `0` | Set to `1` to keep image/stream files |
| `VPT_PROJECT_ROOT` | auto-detected | Project root directory |
| `RUST_LOG` | `vpt_rs=debug` | Log level for CLI tracing |

## Prerequisites

Each provider requires specific system packages:

| Provider | Required Commands | Packages (Debian/Ubuntu) |
|----------|-------------------|--------------------------|
| common | `losetup`, `truncate` | `util-linux`, `coreutils` |
| btrfs | `mkfs.btrfs`, `btrfs` | `btrfs-progs` |
| lvm | `pvcreate`, `vgcreate`, `lvcreate`, `lvremove`, `vgremove`, `pvremove`, `mkfs.ext4` | `lvm2`, `e2fsprogs` |
| zfs | `zpool`, `zfs` | `zfsutils-linux` |

All provider tests require root privileges (use `sudo`).

## Logs

Each test creates per-test log files under `<TEST_DATA_ROOT>/<TEST_ID>/logs/`:

- `<test-name>.log` — Python test log
- `cli.log` — CLI tracing output (RUST_LOG)
