"""Btrfs provider integration test.

Scenario: volume init -> mount -> write data -> snapshot create -> snapshot list
        -> backup -> restore -> mount restored -> verify files -> snapshot delete
        -> cleanup

Uses a loop device with a btrfs filesystem.

Run with: sudo python3 tests/test_btrfs.py
"""

import os
import sys

sys.path.insert(0, os.path.dirname(__file__))

from env import (
    TestEnv,
    backup,
    create_loop_device,
    destroy_loop_device,
    require_provider,
    require_root,
    restore,
    run_cmd,
    snapshot_create,
    snapshot_delete,
    snapshot_list,
)


def test_btrfs_roundtrip(env: TestEnv):
    """Full lifecycle: init, mount, snapshot, backup, restore, verify, cleanup."""
    log = env.get_logger("btrfs")

    img = env.data_path("btrfs.img")
    mount = env.mount_path("btrfs")
    source_subvol = mount / "source-subvol"
    restore_root = mount / "restore-root"
    stream = env.data_path("btrfs.stream")
    loop_dev = None

    try:
        # -- 1. volume init: loop device + btrfs filesystem --
        log.info("Step 1: volume init")
        loop_dev = create_loop_device(img)
        log.info("  Loop device: %s", loop_dev)
        run_cmd(["mkfs.btrfs", "-f", loop_dev])
        log.info("  btrfs filesystem created")

        # -- 2. mount --
        log.info("Step 2: mount")
        env.mkdir(mount)
        run_cmd(["mount", loop_dev, str(mount)])
        log.info("  Mounted at %s", mount)

        # -- 3. write source data --
        log.info("Step 3: write source data")
        run_cmd(["btrfs", "subvolume", "create", str(source_subvol)])
        env.mkdir(restore_root)
        run_cmd([
            "bash", "-c",
            f"echo 'hello-from-btrfs' > '{source_subvol}/hello.txt' && "
            f"echo 'line2-data' > '{source_subvol}/data.txt' && "
            f"mkdir -p '{source_subvol}/sub' && "
            f"echo 'nested-file' > '{source_subvol}/sub/nested.txt'"
        ])
        log.info("  Source subvolume with 3 files: %s", source_subvol)

        # -- 4. snapshot create (via CLI) --
        log.info("Step 4: snapshot create")
        rc, out, err = snapshot_create(
            env, "btrfs", str(source_subvol), label="integ"
        )
        assert rc == 0, f"snapshot create failed: {err}"
        log.info("  Snapshot created")

        # -- 5. snapshot list --
        log.info("Step 5: snapshot list")
        rc, out, err = snapshot_list(env, "btrfs", str(source_subvol))
        assert rc == 0, f"snapshot list failed: {err}"
        assert "integ" in out, "snapshot 'integ' not found in list"
        log.info("  Snapshot list OK")

        # -- 6. backup --
        log.info("Step 6: backup")
        rc, out, err = backup(env, "btrfs", str(source_subvol), str(stream))
        assert rc == 0, f"backup failed: {err}"
        assert stream.is_file(), "stream file not created"
        stream_size = stream.stat().st_size
        assert stream_size > 0, "stream file is empty"
        log.info("  Backup stream: %s (%d bytes)", stream, stream_size)

        # -- 7. restore --
        log.info("Step 7: restore")
        rc, out, err = restore(
            env, "btrfs", str(stream), str(restore_root)
        )
        assert rc == 0, f"restore failed: {err}"
        log.info("  Restored to %s", restore_root)

        # -- 8. mount restored + verify files --
        # btrfs receive creates a subvolume accessible directly in restore_root
        log.info("Step 8: verify restored content")
        restored_hello = list(restore_root.rglob("hello.txt"))
        assert len(restored_hello) > 0, "hello.txt not found in restore"
        assert restored_hello[0].read_text().strip() == "hello-from-btrfs"

        restored_data = list(restore_root.rglob("data.txt"))
        assert len(restored_data) > 0, "data.txt not found in restore"
        assert restored_data[0].read_text().strip() == "line2-data"

        restored_nested = list(restore_root.rglob("nested.txt"))
        assert len(restored_nested) > 0, "nested.txt not found in restore"
        assert restored_nested[0].read_text().strip() == "nested-file"
        log.info("  All 3 files verified OK")

        # -- 9. temp snapshot cleanup --
        log.info("Step 9: temp snapshot cleanup")
        snapshot_root = mount / ".vb-snapshots"
        if snapshot_root.exists():
            tmp_snaps = [
                d for d in snapshot_root.iterdir()
                if d.name.startswith("source-subvol-")
            ]
            assert len(tmp_snaps) == 0, (
                "temporary backup snapshot was not cleaned up"
            )
            log.info("  Temp snapshot cleaned up by backup")

        # -- 10. snapshot delete (via CLI) --
        log.info("Step 10: snapshot delete")
        snap_path = snapshot_root / "integ"
        rc, out, err = snapshot_delete(env, "btrfs", str(snap_path))
        assert rc == 0, f"snapshot delete failed: {err}"

        rc, out, err = snapshot_list(env, "btrfs", str(source_subvol))
        assert rc == 0
        assert str(snap_path) not in out, "snapshot still listed after delete"
        log.info("  Snapshot deleted and verified gone from list")

    finally:
        # -- teardown --
        if mount.exists():
            run_cmd(["umount", str(mount)], check=False)
        if loop_dev:
            destroy_loop_device(loop_dev)
        if not env.keep_artifacts:
            img.unlink(missing_ok=True)
            stream.unlink(missing_ok=True)
        env.cleanup_mount(mount)


if __name__ == "__main__":
    require_root()
    require_provider("btrfs")
    env = TestEnv()
    try:
        test_btrfs_roundtrip(env)
        print("btrfs roundtrip PASSED")
    except Exception as e:
        print(f"btrfs roundtrip FAILED: {e}", file=sys.stderr)
        sys.exit(1)
