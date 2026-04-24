#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE_DIR="${IMAGE_DIR:-/opt/volumeset}"
COPY_DIR="${COPY_DIR:-/opt/volumeset/copy}"
MOUNT_ROOT="${MOUNT_ROOT:-/mnt/volmnt}"
ASSERT_RESTORE_CONTENTS="${ASSERT_RESTORE_CONTENTS:-1}"
ASSERT_SNAPSHOT_CLEANUP="${ASSERT_SNAPSHOT_CLEANUP:-1}"
TEST_NAME="vpt-btrfs"
IMAGE_PATH="${IMAGE_DIR}/${TEST_NAME}.img"
LOOP_DEVICE=""
MOUNT_PATH="${MOUNT_ROOT}/${TEST_NAME}"
RESTORE_ROOT="${MOUNT_PATH}/restore-root"
SOURCE_SUBVOL="${MOUNT_PATH}/source-subvol"
STREAM_PATH="${COPY_DIR}/${TEST_NAME}.stream"
MANUAL_SNAPSHOT_PATH="${MOUNT_PATH}/.vb-snapshots/integ"
SNAPSHOT_MOUNT="${MOUNT_PATH}/snapshot-mount"

cleanup() {
  set +e
  umount "${SNAPSHOT_MOUNT}" >/dev/null 2>&1 || true
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
mkdir -p "${SNAPSHOT_MOUNT}"
printf 'hello-from-btrfs\n' > "${SOURCE_SUBVOL}/hello.txt"

cd "${ROOT_DIR}"
./target/release/vb-snapshot create --provider btrfs --label integ "${SOURCE_SUBVOL}"
./target/release/vb-snapshot list --provider btrfs "${SOURCE_SUBVOL}"
./target/release/vb-mount mount --provider btrfs --target "${SNAPSHOT_MOUNT}" "${MANUAL_SNAPSHOT_PATH}"
grep -q 'hello-from-btrfs' "${SNAPSHOT_MOUNT}/hello.txt"
./target/release/vb-mount unmount --provider btrfs "${SNAPSHOT_MOUNT}"
./target/release/vb-backup --provider btrfs --output "${STREAM_PATH}" "${SOURCE_SUBVOL}"
./target/release/vb-restore --provider btrfs --input "${STREAM_PATH}" "${RESTORE_ROOT}"

if [[ "${ASSERT_RESTORE_CONTENTS}" == "1" ]]; then
  RESTORED_FILE="$(find "${RESTORE_ROOT}" -type f -name hello.txt | head -n 1)"
  if [[ -z "${RESTORED_FILE}" ]]; then
    echo "restored file not found" >&2
    exit 1
  fi

  grep -q 'hello-from-btrfs' "${RESTORED_FILE}"
fi

if [[ "${ASSERT_SNAPSHOT_CLEANUP}" == "1" ]]; then
  if find "${MOUNT_PATH}/.vb-snapshots" -maxdepth 1 -mindepth 1 -name 'source-subvol-*' | grep -q .; then
    echo "temporary backup snapshot was not cleaned up" >&2
    exit 1
  fi

  ./target/release/vb-snapshot delete --provider btrfs "${MANUAL_SNAPSHOT_PATH}"
  SNAPSHOT_LIST_OUTPUT="$(./target/release/vb-snapshot list --provider btrfs "${SOURCE_SUBVOL}")"
  if grep -Fq "${MANUAL_SNAPSHOT_PATH}" <<<"${SNAPSHOT_LIST_OUTPUT}"; then
    echo "manual btrfs snapshot still listed after delete" >&2
    exit 1
  fi
fi

echo "btrfs roundtrip ok"
