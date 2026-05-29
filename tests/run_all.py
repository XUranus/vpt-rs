#!/usr/bin/env python3
"""vpt-rs integration test runner.

Runs all integration tests (btrfs, lvm, zfs, smoke) in sequence.

Usage:
    sudo python3 tests/run_all.py [options]

Options:
    --providers PROVIDERS   Comma-separated providers to test
                            (default: btrfs,lvm,zfs,smoke)
    --data-root PATH        Test data root (default: /tmp/testvolumedata)
    --mount-root PATH       Mount root (default: /tmp/testvolumemnt)
    --keep                  Keep artifacts after test (sets TEST_KEEP_ARTIFACTS=1)
    --no-cleanup            Disable cleanup (sets TEST_CLEANUP=0)
    --build                 Build release binaries before testing
    --timeout SECONDS       Per-test timeout in seconds (default: 180)

Environment variables:
    TEST_DATA_ROOT          Root for test data files
    TEST_MOUNT_ROOT         Root for mount points
    TEST_KEEP_ARTIFACTS     "1" to keep image/stream files
    TEST_CLEANUP            "1" to clean up mount directories

Examples:
    # Run all tests
    sudo python3 tests/run_all.py

    # Run only btrfs and smoke tests
    sudo python3 tests/run_all.py --providers btrfs,smoke

    # Keep artifacts for debugging
    sudo python3 tests/run_all.py --keep

    # Build first, then test
    sudo python3 tests/run_all.py --build
"""

import argparse
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))

from env import TestEnv, build_release, check_commands, PROVIDER_COMMANDS, require_root


def _run_test(name: str, func, env: TestEnv, timeout: int) -> bool:
    """Run a single test with timeout. Returns True on success."""
    import multiprocessing

    def worker(result_queue):
        try:
            func(env)
            result_queue.put(("pass", None))
        except Exception as e:
            result_queue.put(("fail", str(e)))

    ctx = multiprocessing.get_context("fork")
    result_queue = ctx.Queue()
    proc = ctx.Process(target=worker, args=(result_queue,))
    proc.start()
    proc.join(timeout=timeout)

    if proc.is_alive():
        proc.kill()
        proc.join(timeout=5)
        print(f"  FAIL: {name}: TIMEOUT ({timeout}s)")
        return False

    if result_queue.empty():
        print(f"  FAIL: {name}: process exited without result")
        return False

    status, msg = result_queue.get()
    if status == "pass":
        print(f"  PASS: {name}")
        return True
    else:
        print(f"  FAIL: {name}: {msg}")
        return False


def main():
    parser = argparse.ArgumentParser(description="vpt-rs integration tests")
    parser.add_argument(
        "--providers",
        default="btrfs,lvm,zfs,smoke",
        help="Comma-separated providers (default: btrfs,lvm,zfs,smoke)",
    )
    parser.add_argument(
        "--data-root",
        default=None,
        help="Test data root directory",
    )
    parser.add_argument(
        "--mount-root",
        default=None,
        help="Test mount root directory",
    )
    parser.add_argument(
        "--keep",
        action="store_true",
        help="Keep artifacts after test",
    )
    parser.add_argument(
        "--no-cleanup",
        action="store_true",
        help="Disable cleanup",
    )
    parser.add_argument(
        "--build",
        action="store_true",
        help="Build release binaries before testing",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=180,
        help="Per-test timeout in seconds (default: 180)",
    )
    args = parser.parse_args()

    # -- propagate options to environment --
    if args.data_root:
        os.environ["TEST_DATA_ROOT"] = args.data_root
    if args.mount_root:
        os.environ["TEST_MOUNT_ROOT"] = args.mount_root
    if args.keep:
        os.environ["TEST_KEEP_ARTIFACTS"] = "1"
    if args.no_cleanup:
        os.environ["TEST_CLEANUP"] = "0"

    # -- build if requested --
    if args.build:
        print("Building release binaries...")
        build_release()
        print("Build complete.\n")

    # -- parse providers --
    providers = [p.strip() for p in args.providers.split(",") if p.strip()]

    # -- check required commands for selected providers --
    providers_needing_sudo = {"btrfs", "lvm", "zfs"}
    selected_provider_tests = [p for p in providers if p in providers_needing_sudo]

    # VSS requires admin on Windows
    if "vss" in providers:
        import platform
        if platform.system() == "Windows":
            try:
                from test_vss import check_admin_privileges
                check_admin_privileges()
            except ImportError:
                pass

    if selected_provider_tests:
        require_root()
        # Collect all required commands
        needed = set(PROVIDER_COMMANDS["common"])
        for p in selected_provider_tests:
            needed.update(PROVIDER_COMMANDS.get(p, ()))

        missing = check_commands(*needed)
        if missing:
            print(
                f"ERROR: Required commands not found: {', '.join(missing)}\n"
                f"Selected providers: {', '.join(selected_provider_tests)}\n"
                f"Install the corresponding packages:\n",
                file=sys.stderr,
            )
            if "mkfs.btrfs" in missing or "btrfs" in missing:
                print("  btrfs-progs  (for btrfs tests)", file=sys.stderr)
            if "pvcreate" in missing or "lvcreate" in missing:
                print("  lvm2         (for lvm tests)", file=sys.stderr)
            if "zpool" in missing or "zfs" in missing:
                print("  zfsutils-linux (for zfs tests)", file=sys.stderr)
            sys.exit(1)

    # -- import test functions --
    test_modules = {}
    if "btrfs" in providers:
        from test_btrfs import test_btrfs_roundtrip

        test_modules["btrfs"] = test_btrfs_roundtrip
    if "lvm" in providers:
        from test_lvm import test_lvm_roundtrip

        test_modules["lvm"] = test_lvm_roundtrip
    if "zfs" in providers:
        from test_zfs import test_zfs_roundtrip

        test_modules["zfs"] = test_zfs_roundtrip
    if "smoke" in providers:
        from test_smoke import run_all as smoke_run_all

        test_modules["smoke"] = smoke_run_all
    if "vss" in providers:
        try:
            from test_vss import test_vss_roundtrip

            test_modules["vss"] = test_vss_roundtrip
        except ImportError:
            pass  # not on Windows or Hyper-V not available

    # -- run tests --
    env = TestEnv()
    print(f"Test ID: {env.test_id}")
    print(f"Data root: {env.data_root}")
    print(f"Mount root: {env.mount_root}")
    print(f"Cleanup: {env.cleanup}")
    print(f"Timeout: {args.timeout}s per test")
    print()

    passed = 0
    failed = 0
    start = time.time()

    for name in providers:
        func = test_modules.get(name)
        if func is None:
            print(f"  SKIP: {name} (not available)")
            continue

        print(f"--- {name} ---")
        if name == "smoke":
            # Smoke tests manage their own sub-tests
            if func():
                passed += 1
            else:
                failed += 1
        else:
            if _run_test(name, func, env, args.timeout):
                passed += 1
            else:
                failed += 1
        print()

    elapsed = time.time() - start
    print(f"{'='*50}")
    print(f"Results: {passed} passed, {failed} failed ({elapsed:.1f}s)")
    print(f"{'='*50}")

    sys.exit(0 if failed == 0 else 1)


if __name__ == "__main__":
    main()
