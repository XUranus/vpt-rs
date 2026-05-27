"""LVM provider integration test.

Scenario: volume init -> mount -> write data -> snapshot create -> snapshot list
        -> backup -> restore -> mount restored -> verify files -> snapshot delete
        -> cleanup

Uses a loop device as a PV, creates a VG with two LVs (source + restore).

Note: LVM restore requires --force because it overwrites the destination LV.

Run with: sudo python3 tests/test_lvm.py
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


def test_lvm_roundtrip(env: TestEnv):
    """Full lifecycle: init, mount, snapshot, backup, restore, verify, cleanup."""
    log = env.get_logger("lvm")

    img = env.data_path("lvm.img")
    source_mount = env.mount_path("lvm-source")
    restore_mount = env.mount_path("lvm-restore")
    stream = env.data_path("lvm-stream.img")
    loop_dev = None

    VG = f"vptvg-{env.test_id}"
    LV_SRC = "source"
    LV_RST = "restore"
    SNAP_NAME = "integ-lvm"

    try:
        # -- 1. volume init: loop -> PV -> VG -> LVs -> ext4 --
        log.info("Step 1: volume init")
        loop_dev = create_loop_device(img)
        log.info("  Loop device: %s", loop_dev)

        run_cmd(["pvcreate", "-ff", "-y", loop_dev])
        run_cmd(["vgcreate", VG, loop_dev])
        run_cmd(["lvcreate", "-L", "512M", "-n", LV_SRC, VG])
        run_cmd(["lvcreate", "-L", "512M", "-n", LV_RST, VG])

        lv_src = f"/dev/{VG}/{LV_SRC}"
        lv_rst = f"/dev/{VG}/{LV_RST}"
        run_cmd(["mkfs.ext4", "-F", lv_src])
        log.info("  PV/VG/LV created, ext4 formatted: %s", lv_src)

        # -- 2. mount source LV --
        log.info("Step 2: mount source")
        env.mkdir(source_mount)
        run_cmd(["mount", lv_src, str(source_mount)])
        log.info("  Mounted at %s", source_mount)

        # -- 3. write source data --
        log.info("Step 3: write source data")
        run_cmd([
            "bash", "-c",
            f"echo 'hello-from-lvm' > '{source_mount}/hello.txt' && "
            f"echo 'line2-data' > '{source_mount}/data.txt' && "
            f"mkdir -p '{source_mount}/sub' && "
            f"echo 'nested-file' > '{source_mount}/sub/nested.txt'"
        ])
        run_cmd(["sync"])
        run_cmd(["umount", str(source_mount)])
        log.info("  3 files written, source unmounted")

        # -- 4. snapshot create (via CLI) --
        log.info("Step 4: snapshot create")
        rc, out, err = snapshot_create(env, "lvm", lv_src, label=SNAP_NAME)
        assert rc == 0, f"snapshot create failed: {err}"
        log.info("  Snapshot created")

        # -- 5. snapshot list --
        log.info("Step 5: snapshot list")
        rc, out, err = snapshot_list(env, "lvm", lv_src)
        assert rc == 0, f"snapshot list failed: {err}"
        log.info("  Snapshot list OK")

        # -- 6. backup --
        log.info("Step 6: backup")
        rc, out, err = backup(env, "lvm", lv_src, str(stream))
        assert rc == 0, f"backup failed: {err}"
        assert stream.is_file(), "stream file not created"
        stream_size = stream.stat().st_size
        assert stream_size > 0, "stream file is empty"
        log.info("  Backup image: %s (%d bytes)", stream, stream_size)

        # -- 7. restore (force required) --
        log.info("Step 7: restore")
        rc, out, err = restore(env, "lvm", str(stream), lv_rst, force=True)
        assert rc == 0, f"restore failed: {err}"
        log.info("  Restored to %s", lv_rst)

        # -- 8. snapshot delete (via CLI) --
        log.info("Step 8: snapshot delete")
        snap_path = f"/dev/{VG}/{SNAP_NAME}"
        rc, out, err = snapshot_delete(env, "lvm", snap_path)
        assert rc == 0, f"snapshot delete failed: {err}"

        rc, out, err = snapshot_list(env, "lvm", lv_src)
        assert rc == 0
        assert snap_path not in out, "snapshot still listed after delete"
        log.info("  Snapshot deleted and verified gone from list")

        # -- 9. mount restored + verify files --
        # dd wrote the raw ext4 image, so LV already has a valid filesystem
        log.info("Step 9: mount restored + verify")
        env.mkdir(restore_mount)
        run_cmd(["mount", lv_rst, str(restore_mount)])

        rc, out, err = run_cmd(
            ["cat", str(restore_mount / "hello.txt")], check=False
        )
        assert rc == 0, "hello.txt not found in restored LV"
        assert out.strip() == "hello-from-lvm", (
            f"unexpected hello.txt content: {out.strip()!r}"
        )

        rc, out, err = run_cmd(
            ["cat", str(restore_mount / "data.txt")], check=False
        )
        assert rc == 0, "data.txt not found in restored LV"
        assert out.strip() == "line2-data", (
            f"unexpected data.txt content: {out.strip()!r}"
        )

        rc, out, err = run_cmd(
            ["cat", str(restore_mount / "sub" / "nested.txt")], check=False
        )
        assert rc == 0, "sub/nested.txt not found in restored LV"
        assert out.strip() == "nested-file", (
            f"unexpected nested.txt content: {out.strip()!r}"
        )
        log.info("  All 3 files verified OK")

        run_cmd(["umount", str(restore_mount)], check=False)

    finally:
        # -- teardown --
        for mp in [source_mount, restore_mount]:
            run_cmd(["umount", str(mp)], check=False)
            if env.cleanup and mp.is_dir():
                mp.rmdir()

        # Remove snapshot if still exists
        run_cmd(
            ["lvremove", "-fy", f"/dev/{VG}/{SNAP_NAME}"], check=False
        )
        # Remove LVs
        run_cmd(["lvremove", "-fy", f"/dev/{VG}/{LV_RST}"], check=False)
        run_cmd(["lvremove", "-fy", f"/dev/{VG}/{LV_SRC}"], check=False)
        run_cmd(["vgremove", "-fy", VG], check=False)
        if loop_dev:
            run_cmd(["pvremove", "-fy", loop_dev], check=False)
            destroy_loop_device(loop_dev)

        if not env.keep_artifacts:
            img.unlink(missing_ok=True)
            stream.unlink(missing_ok=True)


if __name__ == "__main__":
    require_root()
    require_provider("lvm")
    env = TestEnv()
    try:
        test_lvm_roundtrip(env)
        print("lvm roundtrip PASSED")
    except Exception as e:
        print(f"lvm roundtrip FAILED: {e}", file=sys.stderr)
        sys.exit(1)
