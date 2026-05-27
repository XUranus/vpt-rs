"""CLI smoke tests (cross-platform).

Tests vptcli for basic functionality that does not require privileged
operations or platform-specific backends. These tests can run on any
platform where the Rust binaries are available.

Scenarios covered:
    1. vptcli snapshot backend list
    2. vptcli snapshot capabilities (per available provider on Linux)
    3. vptcli snapshot with no args shows usage
    4. vptcli backup with no args shows usage
    5. vptcli restore with no args shows usage
"""

import os
import platform
import sys

sys.path.insert(0, os.path.dirname(__file__))

from env import TestEnv, run_cmd

VPTCLI = "vptcli"


def test_backend_list(env: TestEnv):
    """vptcli snapshot backend list returns platform info."""
    log = env.get_logger("smoke-backend-list")
    rc, out, err = run_cmd(
        [str(env.bin_dir / VPTCLI), "snapshot", "backend", "list"], check=False
    )
    assert rc == 0, f"backend list failed: {err}"
    assert "platform:" in out, f"unexpected output: {out}"
    log.info("Output:\n%s", out.strip())


def test_capabilities_linux_providers(env: TestEnv):
    """vptcli snapshot capabilities works for each Linux provider."""
    if platform.system() != "Linux":
        return  # skip on non-Linux

    log = env.get_logger("smoke-capabilities")
    for provider in ("btrfs", "lvm", "zfs"):
        rc, out, err = run_cmd(
            [
                str(env.bin_dir / VPTCLI),
                "snapshot",
                "capabilities",
                "--provider",
                provider,
            ],
            check=False,
        )
        assert rc == 0, f"capabilities for {provider} failed: {err}"
        assert provider in out or "linux-" in out, (
            f"unexpected output for {provider}: {out}"
        )
        log.info("Provider %s capabilities:\n%s", provider, out.strip())


def test_snapshot_usage(env: TestEnv):
    """vptcli snapshot with no args shows usage (exit 0)."""
    log = env.get_logger("smoke-snapshot-usage")
    rc, out, err = run_cmd(
        [str(env.bin_dir / VPTCLI), "snapshot"], check=False
    )
    assert rc == 0, f"snapshot no-args should exit 0: {err}"
    assert "create" in out.lower(), f"usage output unexpected: {out}"
    log.info("Usage output OK")


def test_backup_usage(env: TestEnv):
    """vptcli backup with no args shows usage (exit 0)."""
    log = env.get_logger("smoke-backup-usage")
    rc, out, err = run_cmd(
        [str(env.bin_dir / VPTCLI), "backup"], check=False
    )
    assert rc == 0, f"backup no-args should exit 0: {err}"
    assert "--output" in out.lower(), f"usage output unexpected: {out}"
    log.info("Usage output OK")


def test_restore_usage(env: TestEnv):
    """vptcli restore with no args shows usage (exit 0)."""
    log = env.get_logger("smoke-restore-usage")
    rc, out, err = run_cmd(
        [str(env.bin_dir / VPTCLI), "restore"], check=False
    )
    assert rc == 0, f"restore no-args should exit 0: {err}"
    assert "--input" in out.lower(), f"usage output unexpected: {out}"
    log.info("Usage output OK")


def test_snapshot_invalid_provider(env: TestEnv):
    """vptcli snapshot create with unknown provider returns non-zero."""
    log = env.get_logger("smoke-invalid-provider")
    rc, out, err = run_cmd(
        [
            str(env.bin_dir / VPTCLI),
            "snapshot",
            "create",
            "--provider",
            "nonexistent",
            "/tmp/dummy",
        ],
        check=False,
    )
    assert rc != 0, "invalid provider should return non-zero"
    log.info("Invalid provider correctly rejected (rc=%d)", rc)


# ---------------------------------------------------------------------------
# Runner
# ---------------------------------------------------------------------------

ALL_TESTS = [
    ("backend_list", test_backend_list),
    ("capabilities_linux_providers", test_capabilities_linux_providers),
    ("snapshot_usage", test_snapshot_usage),
    ("backup_usage", test_backup_usage),
    ("restore_usage", test_restore_usage),
    ("snapshot_invalid_provider", test_snapshot_invalid_provider),
]


def run_all():
    """Run all smoke tests."""
    env = TestEnv()
    passed = 0
    failed = 0
    for name, func in ALL_TESTS:
        try:
            func(env)
            print(f"  PASS: {name}")
            passed += 1
        except Exception as e:
            print(f"  FAIL: {name}: {e}", file=sys.stderr)
            failed += 1
    print(f"\nSmoke tests: {passed} passed, {failed} failed")
    return failed == 0


if __name__ == "__main__":
    if not run_all():
        sys.exit(1)
