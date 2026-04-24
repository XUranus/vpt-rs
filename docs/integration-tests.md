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

## What Each Script Covers

### Btrfs
- creates a loopback Btrfs filesystem
- creates a source subvolume
- runs `vb-snapshot create/list`
- runs `vb-backup` and `vb-restore`
- validates that restored file content exists

### LVM
- creates a loopback PV/VG/LV
- runs `vb-snapshot create/list/delete`
- validates full snapshot lifecycle for the current LVM provider

### ZFS
- creates a file-backed zpool
- creates a dataset and snapshot
- runs `vb-snapshot list`
- runs `vb-backup` from an explicit snapshot source
- runs `vb-restore --force` into a destination dataset
- validates restored file content

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
