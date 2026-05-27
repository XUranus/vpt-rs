"""ZFS provider integration test.

Scenario: volume init -> mount -> write data -> snapshot create -> snapshot list
        -> backup -> restore -> mount restored -> verify files -> snapshot delete
        -> cleanup

Uses a loop device as a zpool, creates a dataset, writes data, creates a
snapshot, backs up via zfs send, restores via zfs receive, and verifies
restored content.

Note: ZFS backup requires an explicit snapshot source (--snapshot-source).

Run with: sudo python3 tests/test_zfs.py
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


def test_zfs_roundtrip(env: TestEnv):
    """Full lifecycle: init, mount, snapshot, backup, restore, verify, cleanup."""
    log = env.get_logger("zfs")

    img = env.data_path("zfs.img")
    data_mount = env.mount_path("zfs-data")
    restore_mount = env.mount_path("zfs-restore")
    stream = env.data_path("zfs.stream")

    POOL = f"vptpool-{env.test_id}"
    DATASET = f"{POOL}/data"
    RESTORE_DS = f"{POOL}/restore"
    SNAP_ID = f"{DATASET}@snap1"

    try:
        # -- 1. volume init: loop device -> zpool -> datasets --
        log.info("Step 1: volume init")
        run_cmd(["zpool", "create", "-f", POOL, str(img)])
        run_cmd(["zfs", "create", "-o", f"mountpoint={data_mount}", DATASET])
        run_cmd(["zfs", "create", "-o", f"mountpoint={restore_mount}", RESTORE_DS])
        log.info("  Pool %s created with 2 datasets", POOL)

        # -- 2. mount --
        # zfs create auto-mounts the dataset
        log.info("Step 2: mount (auto-mounted by zfs create)")
        log.info("  Data mounted at %s", data_mount)
        log.info("  Restore mounted at %s", restore_mount)

        # -- 3. write source data --
        log.info("Step 3: write source data")
        run_cmd([
            "bash", "-c",
            f"echo 'hello-from-zfs' > '{data_mount}/hello.txt' && "
            f"echo 'line2-data' > '{data_mount}/data.txt' && "
            f"mkdir -p '{data_mount}/sub' && "
            f"echo 'nested-file' > '{data_mount}/sub/nested.txt'"
        ])
        log.info("  3 files written to %s", DATASET)

        # -- 4. snapshot create (via CLI) --
        log.info("Step 4: snapshot create")
        rc, out, err = snapshot_create(env, "zfs", DATASET, label="snap1")
        assert rc == 0, f"snapshot create failed: {err}"
        log.info("  Snapshot created: %s", SNAP_ID)

        # -- 5. snapshot list --
        log.info("Step 5: snapshot list")
        rc, out, err = snapshot_list(env, "zfs", DATASET)
        assert rc == 0, f"snapshot list failed: {err}"
        assert SNAP_ID in out, "snapshot not found in list"
        log.info("  Snapshot list OK")

        # -- 6. backup (explicit snapshot source) --
        log.info("Step 6: backup")
        rc, out, err = backup(
            env, "zfs", SNAP_ID, str(stream), snapshot_source=True
        )
        assert rc == 0, f"backup failed: {err}"
        assert stream.is_file(), "stream file not created"
        stream_size = stream.stat().st_size
        assert stream_size > 0, "stream file is empty"
        log.info("  Backup stream: %s (%d bytes)", stream, stream_size)

        # -- 7. restore --
        log.info("Step 7: restore")
        rc, out, err = restore(
            env, "zfs", str(stream), RESTORE_DS, force=True
        )
        assert rc == 0, f"restore failed: {err}"

        # verify dataset exists
        rc, out, err = run_cmd(["zfs", "list", RESTORE_DS], check=False)
        assert rc == 0, f"restored dataset not found: {err}"
        log.info("  Restored to %s", RESTORE_DS)

        # -- 8. mount restored + verify files --
        # zfs receive auto-mounts the dataset at restore_mount
        log.info("Step 8: mount restored + verify")
        rc, out, err = run_cmd(
            ["cat", str(restore_mount / "hello.txt")], check=False
        )
        assert rc == 0, "hello.txt not found in restored dataset"
        assert out.strip() == "hello-from-zfs", (
            f"unexpected hello.txt content: {out.strip()!r}"
        )

        rc, out, err = run_cmd(
            ["cat", str(restore_mount / "data.txt")], check=False
        )
        assert rc == 0, "data.txt not found in restored dataset"
        assert out.strip() == "line2-data", (
            f"unexpected data.txt content: {out.strip()!r}"
        )

        rc, out, err = run_cmd(
            ["cat", str(restore_mount / "sub" / "nested.txt")], check=False
        )
        assert rc == 0, "sub/nested.txt not found in restored dataset"
        assert out.strip() == "nested-file", (
            f"unexpected nested.txt content: {out.strip()!r}"
        )
        log.info("  All 3 files verified OK")

        # -- 9. snapshot delete (via CLI) --
        log.info("Step 9: snapshot delete")
        rc, out, err = snapshot_delete(env, "zfs", SNAP_ID)
        assert rc == 0, f"snapshot delete failed: {err}"

        rc, out, err = snapshot_list(env, "zfs", DATASET)
        assert rc == 0
        assert SNAP_ID not in out, "snapshot still listed after delete"
        log.info("  Snapshot deleted and verified gone from list")

    finally:
        # -- teardown --
        run_cmd(["zpool", "destroy", "-f", POOL], check=False)
        for mp in [data_mount, restore_mount]:
            if mp.exists() and env.cleanup:
                mp.rmdir()
        if not env.keep_artifacts:
            img.unlink(missing_ok=True)
            stream.unlink(missing_ok=True)


if __name__ == "__main__":
    require_root()
    require_provider("zfs")
    env = TestEnv()
    try:
        test_zfs_roundtrip(env)
        print("zfs roundtrip PASSED")
    except Exception as e:
        print(f"zfs roundtrip FAILED: {e}", file=sys.stderr)
        sys.exit(1)
