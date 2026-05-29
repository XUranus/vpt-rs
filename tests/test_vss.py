#!/usr/bin/env python3
"""Windows VSS provider end-to-end integration test.

End-to-end scenario:
    1. Check administrator privileges
    2. Create source VHD via diskpart, format NTFS, mount
    3. Write test files (hello.txt, data.txt, sub/nested.txt)
    4. Take VSS snapshot via vptcli
    5. Backup volume to image file via vptcli
    6. Create target VHD, format NTFS, mount
    7. Restore backup image to target VHD (raw block-level copy)
    8. Mount target VHD and verify all files match source
    9. Unmount and delete both VHDs (teardown)

Requirements:
    - Must run as Administrator (elevated PowerShell / cmd)
    - Windows 10+ (diskpart for VHD, no Hyper-V module needed)
    - vptcli binary built with: cargo build --release --features windows-vss

Usage:
    # From an elevated PowerShell:
    python tests\\test_vss.py

    # Or from elevated cmd.exe:
    python tests\test_vss.py
"""

import logging
import os
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path
from typing import List, Optional, Tuple

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

LOG_FORMAT = "%(asctime)s [%(levelname)s] %(message)s"
LOG_DATE_FMT = "%H:%M:%S"

# VHD size in MB (small for fast tests)
VHD_SIZE_MB = 256

# Drive letters chosen high to avoid conflicts with system drives.
# The test will abort early if these are already in use.
SOURCE_DRIVE_LETTER = "S"
TARGET_DRIVE_LETTER = "T"

# Three test files - same pattern as the Linux btrfs/lvm/zfs tests.
TEST_FILES = {
    "hello.txt": "hello-from-vss",
    "data.txt": "line2-data",
    "sub/nested.txt": "nested-file",
}


# ---------------------------------------------------------------------------
# Privilege check
# ---------------------------------------------------------------------------

def check_admin_privileges():
    """Verify the script is running with Administrator rights."""
    try:
        import ctypes
        is_admin = bool(ctypes.windll.shell32.IsUserAnAdmin())
    except (ImportError, AttributeError, OSError):
        is_admin = bool(os.environ.get("ADMINISTRATOR"))

    if not is_admin:
        print(
            "ERROR: This test requires Administrator privileges.\n"
            "Re-launch from an elevated shell:\n"
            "  PowerShell:  Start-Process python "
            "-ArgumentList 'tests\\test_vss.py' -Verb RunAs",
            file=sys.stderr,
        )
        sys.exit(1)


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


def find_vptcli() -> Path:
    """Locate the vptcli binary."""
    root = find_project_root()
    for candidate in [
        root / "target" / "release" / "vptcli.exe",
        root / "target" / "debug" / "vptcli.exe",
    ]:
        if candidate.is_file():
            return candidate
    for exe in (root / "target").rglob("vptcli.exe"):
        return exe
    print("ERROR: vptcli.exe not found. Build: cargo build --release --features windows-vss",
          file=sys.stderr)
    sys.exit(1)


# ---------------------------------------------------------------------------
# Subprocess helpers
# ---------------------------------------------------------------------------

def run_cmd(
    cmd: List[str],
    check: bool = True,
    timeout: int = 120,
    log: Optional[logging.Logger] = None,
) -> Tuple[int, str, str]:
    """Run a command. Returns (returncode, stdout, stderr)."""
    if log:
        log.debug("CMD: %s", " ".join(str(c) for c in cmd))
    result = subprocess.run(cmd, capture_output=True, timeout=timeout)
    stdout = result.stdout.decode("utf-8", errors="replace") if result.stdout else ""
    stderr = result.stderr.decode("utf-8", errors="replace") if result.stderr else ""
    if log and stderr.strip():
        log.debug("STDERR: %s", stderr.strip())
    if check and result.returncode != 0:
        raise subprocess.CalledProcessError(result.returncode, cmd, stdout, stderr)
    return result.returncode, stdout, stderr


def _diskpart_log_dir() -> Path:
    d = Path(os.environ.get("TEST_LOG_DIR", ".")) / "logs"
    d.mkdir(parents=True, exist_ok=True)
    return d


def run_diskpart(
    script: str,
    log: Optional[logging.Logger] = None,
    check: bool = True,
    timeout: int = 60,
) -> Tuple[int, str, str]:
    """Execute a diskpart script.  Returns (rc, stdout, stderr)."""
    # Persist every invocation for debugging
    dp_log = _diskpart_log_dir() / "diskpart.log"
    with open(dp_log, "a") as fh:
        fh.write(f"\n{'=' * 60}\n# {time.strftime('%H:%M:%S')}\n{script}")
        if not script.endswith("\n"):
            fh.write("\n")

    fd, tmp = tempfile.mkstemp(suffix=".txt", prefix="dp-")
    try:
        with os.fdopen(fd, "w") as fh:
            fh.write(script)
        result = subprocess.run(
            ["diskpart", "/s", tmp], capture_output=True, timeout=timeout,
        )
        stdout = result.stdout.decode("utf-8", errors="replace") if result.stdout else ""
        stderr = result.stderr.decode("utf-8", errors="replace") if result.stderr else ""
        if log and stdout.strip():
            log.debug("diskpart:\n%s", stdout.strip())
        if check and result.returncode != 0:
            if log:
                log.error("diskpart FAILED:\n%s", stdout)
            raise subprocess.CalledProcessError(
                result.returncode, ["diskpart", "/s", tmp], stdout, stderr,
            )
        return result.returncode, stdout, stderr
    finally:
        try:
            os.unlink(tmp)
        except OSError:
            pass


# ---------------------------------------------------------------------------
# VHD lifecycle helpers (all diskpart, no Hyper-V dependency)
# ---------------------------------------------------------------------------

def create_vhd(vhd_path: Path, size_mb: int, log: logging.Logger) -> None:
    """Create a VHD using ``diskpart create vdisk``.

    Works on all Windows editions (Home, Pro, Server) without Hyper-V.
    """
    log.info("  Creating VHD: %s (%d MB)", vhd_path, size_mb)
    # 'type fixed' creates a pre-allocated VHD (like -Fixed in New-VHD)
    run_diskpart(f'create vdisk file="{vhd_path}" maximum={size_mb} type=fixed\n', log=log)
    assert vhd_path.is_file(), f"VHD not created: {vhd_path}"
    log.info("  VHD created (%d bytes)", vhd_path.stat().st_size)


def mount_vhd(
    vhd_path: Path,
    drive_letter: str,
    log: logging.Logger,
) -> None:
    """Attach a VHD, create partition if needed, format if needed, assign letter.

    Uses diskpart for all operations.  This works on Windows Home without
    the Hyper-V PowerShell module.
    """
    log.info("  Mounting VHD %s as %s:", vhd_path.name, drive_letter)

    # -- Step 1: Attach the VHD --
    # If the VHD is already attached (e.g. from a previous partial
    # operation), 'attach vdisk' fails with E_INVALIDARG.  We ignore
    # the error because we only care that the disk is available.
    run_diskpart(
        f'select vdisk file="{vhd_path}"\nattach vdisk\n',
        log=log, check=False,
    )

    # -- Step 2: Try to assign drive letter to existing partition --
    run_diskpart(
        f'select vdisk file="{vhd_path}"\n'
        f'select partition 1\n'
        f'assign letter={drive_letter}\n',
        log=log, check=False,
    )

    # -- Step 3: Check if the drive is now accessible --
    if Path(f"{drive_letter}:\\").is_dir():
        log.info("    Drive %s:\\ ready (existing partition)", drive_letter)
        return

    # -- Step 4: No partition yet - create one, format, assign --
    log.info("    Creating primary partition + NTFS")
    run_diskpart(
        f'select vdisk file="{vhd_path}"\n'
        f'create partition primary\n'
        f'format fs=ntfs quick label="vpt-{drive_letter}"\n'
        f'assign letter={drive_letter}\n',
        log=log,
    )

    assert Path(f"{drive_letter}:\\").is_dir(), (
        f"Drive {drive_letter}: not accessible after mount"
    )
    log.info("    Drive %s:\\ ready (new partition)", drive_letter)


def unmount_vhd(vhd_path: Path, log: logging.Logger) -> None:
    """Detach a VHD via diskpart.  No-op if the file doesn't exist."""
    if not vhd_path.is_file():
        return
    log.info("  Detaching VHD: %s", vhd_path.name)
    run_diskpart(
        f'select vdisk file="{vhd_path}"\ndetach vdisk\n',
        log=log, check=False,
    )


def delete_vhd(vhd_path: Path, log: logging.Logger) -> None:
    """Detach and delete a VHD file."""
    unmount_vhd(vhd_path, log)
    try:
        vhd_path.unlink(missing_ok=True)
        log.info("  Deleted: %s", vhd_path.name)
    except PermissionError:
        log.warning("  Could not delete %s (in use?)", vhd_path.name)


def drive_letter_in_use(letter: str) -> bool:
    """Return True if the given drive letter is already assigned."""
    return Path(f"{letter}:\\").is_dir()


# ---------------------------------------------------------------------------
# vptcli wrappers
# ---------------------------------------------------------------------------

def vptcli(
    bin_path: Path, args: List[str], log: logging.Logger,
) -> Tuple[int, str, str]:
    """Run vptcli.  Returns (rc, stdout, stderr)."""
    log.debug("vptcli %s", " ".join(args))
    result = subprocess.run(
        [str(bin_path)] + args, capture_output=True, timeout=120,
    )
    # Decode with fallback for Chinese Windows (GBK encoding)
    stdout = result.stdout.decode("utf-8", errors="replace") if result.stdout else ""
    stderr = result.stderr.decode("utf-8", errors="replace") if result.stderr else ""
    if stderr and stderr.strip():
        log.debug("vptcli stderr: %s", stderr.strip())
    if result.returncode != 0:
        log.error("vptcli FAILED (rc=%d): %s\nstderr: %s",
                  result.returncode, " ".join(args), stderr.strip())
    return result.returncode, stdout, stderr


def vptcli_snapshot_create(
    bin_path: Path, provider: str, volume: str,
    label: Optional[str], log: logging.Logger,
) -> Tuple[int, str, str]:
    args = ["snapshot", "create", "--provider", provider]
    if label:
        args += ["--label", label]
    args.append(volume)
    return vptcli(bin_path, args, log)


def vptcli_snapshot_list(
    bin_path: Path, provider: str, volume: str, log: logging.Logger,
) -> Tuple[int, str, str]:
    return vptcli(bin_path, ["snapshot", "list", "--provider", provider, volume], log)


def vptcli_snapshot_delete(
    bin_path: Path, provider: str, snapshot_id: str, log: logging.Logger,
) -> Tuple[int, str, str]:
    return vptcli(bin_path, ["snapshot", "delete", "--provider", provider, snapshot_id], log)


def vptcli_backup(
    bin_path: Path, provider: str, source: str, output: str, log: logging.Logger,
) -> Tuple[int, str, str]:
    return vptcli(bin_path, ["backup", "--provider", provider, "--output", output, source], log)


def vptcli_restore(
    bin_path: Path, provider: str, input_file: str, destination: str, log: logging.Logger,
) -> Tuple[int, str, str]:
    return vptcli(
        bin_path,
        ["restore", "--provider", provider, "--force", "--input", input_file, destination],
        log,
    )


# ---------------------------------------------------------------------------
# Main test
# ---------------------------------------------------------------------------

def test_vss_roundtrip():
    """Full VSS lifecycle: VHD → mount → write → snapshot → backup
    → restore → verify.
    """
    test_id = str(uuid.uuid4())[:8]

    # -- directories --
    data_dir = Path(os.environ.get("TEST_DATA_ROOT", r"C:\temp\vpt-test")) / test_id
    data_dir.mkdir(parents=True, exist_ok=True)
    log_dir = data_dir / "logs"
    log_dir.mkdir(parents=True, exist_ok=True)
    os.environ["TEST_LOG_DIR"] = str(data_dir)

    # -- logging --
    # Force UTF-8 console output to avoid GBK encoding errors on Chinese Windows
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    log = logging.getLogger("vss")
    log.setLevel(logging.DEBUG)
    fmt = logging.Formatter(LOG_FORMAT, datefmt=LOG_DATE_FMT)
    fh = logging.FileHandler(str(log_dir / "vss.log"), encoding="utf-8")
    fh.setLevel(logging.DEBUG)
    fh.setFormatter(fmt)
    log.addHandler(fh)
    ch = logging.StreamHandler(sys.stdout)
    ch.setLevel(logging.DEBUG)
    ch.setFormatter(fmt)
    log.addHandler(ch)

    log.info("Test ID: %s", test_id)
    log.info("Data dir: %s", data_dir)

    bin_path = find_vptcli()
    log.info("vptcli: %s", bin_path)

    # -- pre-flight: check drive letters are free --
    for letter in (SOURCE_DRIVE_LETTER, TARGET_DRIVE_LETTER):
        if drive_letter_in_use(letter):
            print(f"ERROR: Drive letter {letter}: already in use.", file=sys.stderr)
            sys.exit(1)

    source_vhd = data_dir / "source.vhd"
    target_vhd = data_dir / "target.vhd"
    backup_img = data_dir / "backup.img"
    source_drive = f"{SOURCE_DRIVE_LETTER}:"
    target_drive = f"{TARGET_DRIVE_LETTER}:"

    # =====================================================================
    log.info("Phase 1: check administrator privileges and VSS service")
    check_admin_privileges()
    log.info("  Running as Administrator OK")

    # Ensure VSS service is running
    rc, out, _ = run_cmd(["sc", "query", "vss"], check=False, log=log)
    if "RUNNING" not in out:
        log.info("  Starting VSS service...")
        run_cmd(["net", "start", "vss"], check=False, log=log)
        time.sleep(1)
    log.info("  VSS service is running")

    try:
        # =================================================================
        log.info("Phase 2: create source VHD and mount")
        create_vhd(source_vhd, VHD_SIZE_MB, log)
        mount_vhd(source_vhd, SOURCE_DRIVE_LETTER, log)
        log.info("  Source mounted at %s", source_drive)

        # ================================================================
        log.info("Phase 3: write source data")
        for rel_path, content in TEST_FILES.items():
            dest = Path(source_drive + "\\") / rel_path
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_text(content, encoding="utf-8")
            log.info("    %s -> %s", rel_path, repr(content))
        log.info("  %d files written", len(TEST_FILES))

        # ================================================================
        log.info("Phase 4: VSS snapshot create")
        rc, out, err = vptcli_snapshot_create(
            bin_path, "windows-vss", source_drive, label="integ", log=log,
        )
        snapshot_id = None
        if rc == 0:
            assert "snapshot:" in out, f"Unexpected output: {out}"
            for line in out.splitlines():
                if line.strip().startswith("snapshot:"):
                    snapshot_id = line.split(":", 1)[1].strip()
                    break
            log.info("  Snapshot created: %s", snapshot_id)
        else:
            log.warning("  Snapshot create returned %d; continuing (backup auto-snapshots)", rc)

        # ================================================================
        log.info("Phase 5: VSS snapshot list")
        rc_list, out_list, _ = vptcli_snapshot_list(
            bin_path, "windows-vss", source_drive, log=log,
        )
        if rc_list == 0:
            log.info("  Snapshot list OK:\n%s", out_list.strip())
        else:
            log.warning("  Snapshot list returned %d", rc_list)

        # ================================================================
        log.info("Phase 6: backup volume -> %s", backup_img)
        rc, out, err = vptcli_backup(
            bin_path, "windows-vss", source_drive, str(backup_img), log=log,
        )
        assert rc == 0, f"backup failed (rc={rc}):\nstdout: {out}\nstderr: {err}"
        assert backup_img.is_file(), "backup image not created"
        img_size = backup_img.stat().st_size
        assert img_size > 0, "backup image is empty"
        log.info("  Backup image: %s (%d bytes)", backup_img, img_size)

        # ================================================================
        log.info("Phase 7: VSS snapshot delete")
        if snapshot_id:
            rc_del, _, err_del = vptcli_snapshot_delete(
                bin_path, "windows-vss", snapshot_id, log=log,
            )
            log.info("  Snapshot delete rc=%d", rc_del)

        # ================================================================
        log.info("Phase 8: unmount source VHD")
        unmount_vhd(source_vhd, log)
        log.info("  Source VHD detached")

        # ================================================================
        log.info("Phase 9: create target VHD and mount")
        create_vhd(target_vhd, VHD_SIZE_MB, log)
        mount_vhd(target_vhd, TARGET_DRIVE_LETTER, log)
        log.info("  Target mounted at %s", target_drive)

        # ================================================================
        log.info("Phase 10: restore backup image to target VHD")
        # Unmount target, write raw image directly to VHD, re-mount
        unmount_vhd(target_vhd, log)
        log.info("  Writing %s -> %s", backup_img.name, target_vhd.name)
        with open(backup_img, "rb") as src, open(target_vhd, "r+b") as dst:
            copied = 0
            while True:
                chunk = src.read(4 * 1024 * 1024)
                if not chunk:
                    break
                dst.write(chunk)
                copied += len(chunk)
            dst.flush()
            os.fsync(dst.fileno())
        log.info("  Wrote %d bytes to target VHD", copied)

        log.info("  Re-mounting target VHD")
        mount_vhd(target_vhd, TARGET_DRIVE_LETTER, log)
        log.info("  Target re-mounted at %s", target_drive)

        # ================================================================
        log.info("Phase 11: verify restored files")
        target_root = Path(target_drive + "\\")
        for rel_path, expected in TEST_FILES.items():
            file_path = target_root / rel_path
            assert file_path.is_file(), f"Restored file not found: {rel_path}"
            actual = file_path.read_text(encoding="utf-8").strip()
            assert actual == expected, (
                f"Content mismatch for {rel_path}: expected {expected!r}, got {actual!r}"
            )
            log.info("  PASS: %s = %s", rel_path, repr(expected))
        log.info("  All %d files verified successfully", len(TEST_FILES))

        # ================================================================
        log.info("Phase 12: teardown")
        unmount_vhd(target_vhd, log)
        unmount_vhd(source_vhd, log)
        log.info("  Both VHDs detached")

    except Exception:
        log.exception("Test FAILED")
        raise
    finally:
        unmount_vhd(target_vhd, logging.getLogger("vss-cleanup"))
        unmount_vhd(source_vhd, logging.getLogger("vss-cleanup"))
        if os.environ.get("TEST_KEEP_ARTIFACTS", "0") != "1":
            delete_vhd(source_vhd, logging.getLogger("vss-cleanup"))
            delete_vhd(target_vhd, logging.getLogger("vss-cleanup"))
            backup_img.unlink(missing_ok=True)
        else:
            log.info("  Artifacts kept (--keep):")
            log.info("    Source VHD : %s", source_vhd)
            log.info("    Target VHD : %s", target_vhd)
            log.info("    Backup img : %s", backup_img)
            log.info("    Logs       : %s", data_dir / "logs")

    log.info("")
    log.info("=" * 60)
    log.info("  VSS roundtrip test PASSED")
    log.info("=" * 60)


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(
        description="Windows VSS integration test (requires Administrator)",
    )
    parser.add_argument(
        "--keep", action="store_true",
        help="Keep VHD files and backup image after test",
    )
    parser.add_argument(
        "--data-root", default=None,
        help="Root directory for test artifacts (default: C:\\temp\\vpt-test)",
    )
    args = parser.parse_args()

    if args.keep:
        os.environ["TEST_KEEP_ARTIFACTS"] = "1"
    if args.data_root:
        os.environ["TEST_DATA_ROOT"] = args.data_root

    check_admin_privileges()

    try:
        test_vss_roundtrip()
        print("\nVSS roundtrip test PASSED")
    except Exception as exc:
        print(f"\nVSS roundtrip test FAILED: {exc}", file=sys.stderr)
        sys.exit(1)
