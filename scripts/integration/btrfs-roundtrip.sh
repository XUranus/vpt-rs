#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE_DIR="${IMAGE_DIR:-/opt/volumeset}"
COPY_DIR="${COPY_DIR:-/opt/volumeset/copy}"
MOUNT_ROOT="${MOUNT_ROOT:-/mnt/volmnt}"
TEST_NAME="vpt-btrfs"
IMAGE_PATH="${IMAGE_DIR}/${TEST_NAME}.img"
LOOP_DEVICE=""
MOUNT_PATH="${MOUNT_ROOT}/${TEST_NAME}"
RESTORE_ROOT="${MOUNT_PATH}/restore-root"
SOURCE_SUBVOL="${MOUNT_PATH}/source-subvol"
STREAM_PATH="${COPY_DIR}/${TEST_NAME}.stream"

cleanup() {
  set +e
  if mountpoint -q "${MOUNT_PATH}"; then
    umount "${MOUNT_PATH}"
  fi
  if [[ -n "${LOOP_DEVICE}" ]]; then
    losetup -d "${LOOP_DEVICE}"
  fi
  rm -f "${IMAGE_PATH}" "${STREAM_PATH}"
  rm -rf "${MOUNT_PATH}"
}
trap cleanup EXIT

if [[ "${EUID}" -ne 0 ]]; then
  echo "run this script as root (for loop devices and mounts)" >&2
  exit 1
fi

mkdir -p "${IMAGE_DIR}" "${COPY_DIR}" "${MOUNT_ROOT}"
truncate -s 2G "${IMAGE_PATH}"
LOOP_DEVICE="$(losetup --find --show "${IMAGE_PATH}")"
mkfs.btrfs -f "${LOOP_DEVICE}"
mkdir -p "${MOUNT_PATH}"
mount "${LOOP_DEVICE}" "${MOUNT_PATH}"

btrfs subvolume create "${SOURCE_SUBVOL}"
mkdir -p "${RESTORE_ROOT}"
printf 'hello-from-btrfs\n' > "${SOURCE_SUBVOL}/hello.txt"

cd "${ROOT_DIR}"
cargo run --bin vb-snapshot -- create --provider btrfs --label integ "${SOURCE_SUBVOL}"
cargo run --bin vb-snapshot -- list --provider btrfs "${SOURCE_SUBVOL}"
cargo run --bin vb-backup -- --provider btrfs --output "${STREAM_PATH}" "${SOURCE_SUBVOL}"
cargo run --bin vb-restore -- --provider btrfs --input "${STREAM_PATH}" "${RESTORE_ROOT}"

RESTORED_FILE="$(find "${RESTORE_ROOT}" -type f -name hello.txt | head -n 1)"
if [[ -z "${RESTORED_FILE}" ]]; then
  echo "restored file not found" >&2
  exit 1
fi

grep -q 'hello-from-btrfs' "${RESTORED_FILE}"
echo "btrfs roundtrip ok"
