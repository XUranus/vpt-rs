# Integration Scripts

The repository now includes filesystem-backed integration scripts under `scripts/integration/`.

## Available Scripts
- `scripts/integration/btrfs-roundtrip.sh`
- `scripts/integration/lvm-snapshot.sh`
- `scripts/integration/zfs-roundtrip.sh`
- `scripts/integration/run-all.sh`

## Environment Assumptions
- image files are created under `/opt/volumeset`
- backup streams are written under `/opt/volumeset/copy`
- mounts use subdirectories under `/mnt/volmnt`
- each script keeps image sizes at `2G`, well below the requested limits
- `IMAGE_DIR`, `COPY_DIR`, and `MOUNT_ROOT` can be overridden from the environment when needed
- `ASSERT_RESTORE_CONTENTS=0` disables restored-data checks when you only want lifecycle coverage
- `ASSERT_SNAPSHOT_CLEANUP=0` disables post-delete / temporary-snapshot cleanup assertions

## What Each Script Covers

### Btrfs
- creates a loopback Btrfs filesystem
- creates a source subvolume
- runs `vb-snapshot create/list`
- runs `vb-backup` and `vb-restore`
- validates that restored file content exists
- validates that the temporary backup snapshot is removed
- deletes the manual snapshot and verifies it is no longer listed

### LVM
- creates a loopback PV/VG/LV
- formats source and restore logical volumes as `ext4`
- runs `vb-snapshot create/list/delete`
- mounts the snapshot through `vb-mount`, reads the test file, and unmounts it
- runs `vb-backup` to export the source LV to an image file
- runs `vb-restore --force` to write that image into a restore LV
- validates restored file content
- verifies that the deleted snapshot is no longer listed

### ZFS
- creates a file-backed zpool
- creates a dataset and snapshot
- runs `vb-snapshot list`
- runs `vb-backup` from an explicit snapshot source
- runs `vb-restore --force` into a destination dataset
- validates restored file content
- deletes the source snapshot and verifies it is no longer listed

## Running
These scripts require privileges for loop devices, mounts, and storage management. Run them directly from the repository root or invoke them with an appropriate privileged shell.

Example:

```bash
sudo IMAGE_DIR=/opt/volumeset COPY_DIR=/opt/volumeset/copy MOUNT_ROOT=/mnt/volmnt \
  bash scripts/integration/btrfs-roundtrip.sh
```

To run all currently available integration scripts:

```bash
sudo IMAGE_DIR=/opt/volumeset COPY_DIR=/opt/volumeset/copy MOUNT_ROOT=/mnt/volmnt \
  bash scripts/integration/run-all.sh
```
