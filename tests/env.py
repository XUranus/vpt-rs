"""Shared test environment for vpt-rs integration tests.

Provides:
- Root privilege detection (run via `sudo python3`)
- Required command availability checks with clear messages
- UUID-based artifact isolation
- Loop device lifecycle management
- CLI wrappers for vptcli (snapshot, backup, restore subcommands)
- Structured logging to per-test log files
- CLI tracing (RUST_LOG) captured to per-test cli.log

Environment variables:
    TEST_DATA_ROOT      Root for test data (default: /tmp/testvolumedata)
    TEST_MOUNT_ROOT     Root for mount points (default: /tmp/testvolumemnt)
    TEST_ID             Test UUID (auto-generated if not set)
    TEST_CLEANUP        "1" to clean up after test (default: "1")
    TEST_KEEP_ARTIFACTS "1" to keep image/stream files (default: "0")
    VPT_PROJECT_ROOT    Project root directory (auto-detected if not set)
    RUST_LOG            Log level for CLI tools (default: vpt_rs=debug)
"""

import logging
import os
import shutil
import subprocess
import sys
import uuid
from pathlib import Path
from typing import List, Optional, Tuple

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

DATA_ROOT_DEFAULT = "/tmp/testvolumedata"
MOUNT_ROOT_DEFAULT = "/tmp/testvolumemnt"

LOG_FORMAT = "%(asctime)s [%(levelname)s] %(message)s"
LOG_DATE_FORMAT = "%H:%M:%S"

BINARY_NAMES = ("vptcli",)

# Per-provider required system commands
PROVIDER_COMMANDS = {
    "common": ("losetup", "truncate"),
    "btrfs": ("mkfs.btrfs", "btrfs"),
    "lvm": ("pvcreate", "vgcreate", "lvcreate", "lvremove", "vgremove", "pvremove", "mkfs.ext4"),
    "zfs": ("zpool", "zfs"),
}

# ---------------------------------------------------------------------------
# Privilege & command checks
# ---------------------------------------------------------------------------


def require_root():
    """Check that the script is running as root. Exit with a clear message if not."""
    if os.geteuid() != 0:
        print(
            "ERROR: This test requires root privileges.\n"
            "Please run with sudo:\n"
            f"    sudo python3 {' '.join(sys.argv)}",
            file=sys.stderr,
        )
        sys.exit(1)


def check_commands(*command_names: str) -> List[str]:
    """Check if commands are available on PATH.

    Returns a list of missing command names (empty if all found).
    """
    missing = []
    for cmd in command_names:
        if shutil.which(cmd) is None:
            missing.append(cmd)
    return missing


def require_commands(*command_names: str):
    """Check that all commands are available. Exit with a concrete message if any is missing."""
    missing = check_commands(*command_names)
    if missing:
        print(
            f"ERROR: Required commands not found: {', '.join(missing)}\n"
            f"Install the corresponding packages and try again.",
            file=sys.stderr,
        )
        sys.exit(1)


def require_provider(provider: str):
    """Check that all commands for a given provider are available."""
    cmds = PROVIDER_COMMANDS.get("common", ()) + PROVIDER_COMMANDS.get(provider, ())
    missing = check_commands(*cmds)
    if missing:
        print(
            f"ERROR: Provider '{provider}' requires these commands: {', '.join(cmds)}\n"
            f"Missing: {', '.join(missing)}\n"
            f"Install the corresponding packages (e.g. btrfs-progs, lvm2, zfsutils-linux) and try again.",
            file=sys.stderr,
        )
        sys.exit(1)


# ---------------------------------------------------------------------------
# Command execution
# ---------------------------------------------------------------------------


def run_cmd(
    cmd: List[str],
    check: bool = True,
    capture: bool = True,
    timeout: int = 120,
) -> Tuple[int, str, str]:
    """Run a command. Since the script runs as root, no sudo wrapper is needed.

    Returns (returncode, stdout, stderr).
    Raises subprocess.CalledProcessError if check=True and command fails.
    """
    result = subprocess.run(
        cmd,
        capture_output=capture,
        text=True,
        timeout=timeout,
    )
    if check and result.returncode != 0:
        raise subprocess.CalledProcessError(
            result.returncode, cmd, result.stdout, result.stderr
        )
    return result.returncode, result.stdout or "", result.stderr or ""


# ---------------------------------------------------------------------------
# Project / binary helpers
# ---------------------------------------------------------------------------


def find_project_root() -> Path:
    """Locate the project root (directory containing Cargo.toml)."""
    custom = os.environ.get("VPT_PROJECT_ROOT")
    if custom:
        return Path(custom)
    p = Path(__file__).resolve().parent.parent
    if (p / "Cargo.toml").is_file():
        return p
    raise FileNotFoundError("Cannot find project root (Cargo.toml)")


def find_bin_dir() -> Path:
    """Return path to release binary directory."""
    return find_project_root() / "target" / "release"


def ensure_built(bin_dir: Path) -> None:
    """Verify all required binaries exist."""
    missing = [n for n in BINARY_NAMES if not (bin_dir / n).is_file()]
    if missing:
        print(
            f"ERROR: binaries not found: {missing}. Run `cargo build --release` first.",
            file=sys.stderr,
        )
        sys.exit(1)


def build_release() -> None:
    """Build the project in release mode."""
    run_cmd(["cargo", "build", "--release"], timeout=600)


# ---------------------------------------------------------------------------
# TestEnv: UUID-isolated artifact management
# ---------------------------------------------------------------------------


class TestEnv:
    """Manages UUID-isolated test artifacts and logging.

    Every test run gets a unique UUID. All data, mounts, and logs are
    stored under this UUID namespace so tests never collide.
    """

    def __init__(self):
        self.data_root = Path(
            os.environ.get("TEST_DATA_ROOT", DATA_ROOT_DEFAULT)
        )
        self.mount_root = Path(
            os.environ.get("TEST_MOUNT_ROOT", MOUNT_ROOT_DEFAULT)
        )
        self.cleanup = os.environ.get("TEST_CLEANUP", "1") == "1"
        self.keep_artifacts = (
            os.environ.get("TEST_KEEP_ARTIFACTS", "0") == "1"
        )
        self.test_id = os.environ.get("TEST_ID") or str(uuid.uuid4())[:8]
        self.project_root = find_project_root()
        self.bin_dir = find_bin_dir()

        # Enable verbose CLI tracing unless overridden
        os.environ.setdefault("RUST_LOG", "vpt_rs=debug")

        ensure_built(self.bin_dir)

        self._dirs_created = False
        self._loggers: dict = {}

    # -- directory management --

    def ensure_dirs(self):
        """Create top-level data and mount directories."""
        if not self._dirs_created:
            self.data_root.mkdir(parents=True, exist_ok=True)
            self.mount_root.mkdir(parents=True, exist_ok=True)
            self._dirs_created = True

    def data_path(self, name: str) -> Path:
        """Absolute path for a test data file/dir under the UUID namespace."""
        self.ensure_dirs()
        return self.data_root / self.test_id / name

    def mount_path(self, name: str) -> Path:
        """Absolute path for a mount point under the UUID namespace."""
        self.ensure_dirs()
        return self.mount_root / self.test_id / name

    def mkdir(self, path: Path) -> Path:
        """Create directory and parents, return path."""
        path.mkdir(parents=True, exist_ok=True)
        return path

    # -- logging --

    def cli_log_path(self) -> Path:
        """Return path to the CLI tracing log file, creating it on first call."""
        log_dir = self.mkdir(self.data_path("logs"))
        log_file = log_dir / "cli.log"
        if not log_file.exists():
            log_file.touch()
        return log_file

    def _write_cli_log(self, tool: str, args: List[str], stderr: str):
        """Append CLI command and its tracing output to cli.log."""
        if not stderr.strip():
            return
        log_file = self.cli_log_path()
        with open(log_file, "a") as f:
            f.write(f"\n{'='*60}\n")
            f.write(f"$ {tool} {' '.join(args)}\n")
            f.write(f"{'='*60}\n")
            f.write(stderr)
            if not stderr.endswith("\n"):
                f.write("\n")

    def get_logger(self, name: str) -> logging.Logger:
        """Get a logger that writes to a per-test log file and console."""
        if name in self._loggers:
            return self._loggers[name]

        logger = logging.getLogger(name)
        logger.setLevel(logging.DEBUG)

        log_dir = self.mkdir(self.data_path("logs"))
        log_file = log_dir / f"{name}.log"

        # File handler
        fh = logging.FileHandler(str(log_file))
        fh.setLevel(logging.DEBUG)
        fh.setFormatter(logging.Formatter(LOG_FORMAT, datefmt=LOG_DATE_FORMAT))
        logger.addHandler(fh)

        # Console handler
        ch = logging.StreamHandler(sys.stdout)
        ch.setLevel(logging.INFO)
        ch.setFormatter(logging.Formatter(LOG_FORMAT, datefmt=LOG_DATE_FORMAT))
        logger.addHandler(ch)

        logger.info("Log file: %s", log_file)
        self._loggers[name] = logger
        return logger

    # -- cleanup --

    def cleanup_test(self):
        """Remove data and mount directories for this test ID."""
        for root in (self.data_root, self.mount_root):
            target = root / self.test_id
            if target.exists():
                shutil.rmtree(target, ignore_errors=True)

    def cleanup_mount(self, mount_point: Path):
        """Unmount and remove a mount point directory."""
        if mount_point.exists():
            run_cmd(
                ["umount", str(mount_point)],
                check=False,
                capture=True,
            )
            if self.cleanup:
                shutil.rmtree(mount_point, ignore_errors=True)


# ---------------------------------------------------------------------------
# Loop device helpers
# ---------------------------------------------------------------------------


def create_loop_device(img_path: Path, size: str = "2G") -> str:
    """Create a sparse image file and attach it as a loop device.

    Returns the loop device path (e.g. /dev/loop0).
    """
    img_path.parent.mkdir(parents=True, exist_ok=True)
    run_cmd(["truncate", "-s", size, str(img_path)])
    _, stdout, _ = run_cmd(["losetup", "--find", "--show", str(img_path)])
    return stdout.strip()


def destroy_loop_device(device: str):
    """Detach a loop device."""
    run_cmd(["losetup", "-d", device], check=False)


# ---------------------------------------------------------------------------
# CLI wrappers
# ---------------------------------------------------------------------------


def _cli(env: TestEnv, tool: str, args: List[str]) -> Tuple[int, str, str]:
    """Run a vpt CLI tool. Captures tracing output (stderr) to cli.log."""
    rc, stdout, stderr = run_cmd([str(env.bin_dir / tool)] + args, check=False)
    env._write_cli_log(tool, args, stderr)
    return rc, stdout, stderr


def snapshot_create(
    env: TestEnv,
    provider: str,
    volume: str,
    label: Optional[str] = None,
    read_only: bool = True,
) -> Tuple[int, str, str]:
    """Run vptcli snapshot create."""
    args = ["snapshot", "create", "--provider", provider]
    if label:
        args += ["--label", label]
    if not read_only:
        args += ["--read-write"]
    args.append(volume)
    return _cli(env, "vptcli", args)


def snapshot_list(
    env: TestEnv, provider: str, volume: str
) -> Tuple[int, str, str]:
    """Run vptcli snapshot list."""
    return _cli(
        env, "vptcli", ["snapshot", "list", "--provider", provider, volume]
    )


def snapshot_delete(
    env: TestEnv, provider: str, snapshot_id: str
) -> Tuple[int, str, str]:
    """Run vptcli snapshot delete."""
    return _cli(
        env, "vptcli", ["snapshot", "delete", "--provider", provider, snapshot_id]
    )


def backup(
    env: TestEnv,
    provider: str,
    source: str,
    output: str,
    snapshot_source: bool = False,
) -> Tuple[int, str, str]:
    """Run vptcli backup."""
    args = ["backup", "--provider", provider, "--output", output]
    if snapshot_source:
        args.append("--snapshot-source")
    args.append(source)
    return _cli(env, "vptcli", args)


def restore(
    env: TestEnv,
    provider: str,
    input_file: str,
    destination: str,
    force: bool = False,
) -> Tuple[int, str, str]:
    """Run vptcli restore."""
    args = ["restore", "--provider", provider, "--input", input_file]
    if force:
        args.append("--force")
    args.append(destination)
    return _cli(env, "vptcli", args)
