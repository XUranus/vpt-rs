#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE_DIR="${IMAGE_DIR:-/opt/volumeset}"
MOUNT_ROOT="${MOUNT_ROOT:-/mnt/volmnt}"
COPY_DIR="${COPY_DIR:-/opt/volumeset/copy}"
ASSERT_RESTORE_CONTENTS="${ASSERT_RESTORE_CONTENTS:-1}"
ASSERT_SNAPSHOT_CLEANUP="${ASSERT_SNAPSHOT_CLEANUP:-1}"
TEST_NAME="vpt-lvm"
IMAGE_PATH="${IMAGE_DIR}/${TEST_NAME}.img"
LOOP_DEVICE=""
VG_NAME="vptvg"
LV_NAME="source"
RESTORE_LV_NAME="restore"
SNAP_NAME="integ-lvm"
STREAM_PATH="${COPY_DIR}/${TEST_NAME}.img"
SOURCE_MOUNT="${MOUNT_ROOT}/${TEST_NAME}-source"
RESTORE_MOUNT="${MOUNT_ROOT}/${TEST_NAME}-restore"
SNAPSHOT_MOUNT="${MOUNT_ROOT}/${TEST_NAME}-snapshot"

cleanup() {
  set +e
  ./target/release/vb-snapshot delete --provider lvm "/dev/${VG_NAME}/${SNAP_NAME}" >/dev/null 2>&1 || true
  umount "${SOURCE_MOUNT}" >/dev/null 2>&1 || true
  umount "${RESTORE_MOUNT}" >/dev/null 2>&1 || true
  umount "${SNAPSHOT_MOUNT}" >/dev/null 2>&1 || true
  lvremove -fy "/dev/${VG_NAME}/${RESTORE_LV_NAME}" >/dev/null 2>&1 || true
  lvremove -fy "/dev/${VG_NAME}/${LV_NAME}" >/dev/null 2>&1 || true
  vgremove -fy "${VG_NAME}" >/dev/null 2>&1 || true
  pvremove -fy "${LOOP_DEVICE}" >/dev/null 2>&1 || true
  if [[ -n "${LOOP_DEVICE}" ]]; then
    losetup -d "${LOOP_DEVICE}" >/dev/null 2>&1 || true
  fi
  rm -f "${IMAGE_PATH}" "${STREAM_PATH}"
  rm -rf "${SOURCE_MOUNT}" "${RESTORE_MOUNT}" "${SNAPSHOT_MOUNT}"
}
trap cleanup EXIT

if [[ "${EUID}" -ne 0 ]]; then
  echo "run this script as root (for loop devices and LVM operations)" >&2
  exit 1
fi

mkdir -p "${IMAGE_DIR}" "${MOUNT_ROOT}"
truncate -s 2G "${IMAGE_PATH}"
LOOP_DEVICE="$(losetup --find --show "${IMAGE_PATH}")"
pvcreate -ff -y "${LOOP_DEVICE}"
vgcreate "${VG_NAME}" "${LOOP_DEVICE}"
lvcreate -L 512M -n "${LV_NAME}" "${VG_NAME}"
lvcreate -L 512M -n "${RESTORE_LV_NAME}" "${VG_NAME}"
mkfs.ext4 -F "/dev/${VG_NAME}/${LV_NAME}" >/dev/null
mkfs.ext4 -F "/dev/${VG_NAME}/${RESTORE_LV_NAME}" >/dev/null
mkdir -p "${COPY_DIR}" "${SOURCE_MOUNT}" "${RESTORE_MOUNT}" "${SNAPSHOT_MOUNT}"
mount "/dev/${VG_NAME}/${LV_NAME}" "${SOURCE_MOUNT}"
printf 'hello-from-lvm\n' > "${SOURCE_MOUNT}/hello.txt"
sync
umount "${SOURCE_MOUNT}"

cd "${ROOT_DIR}"
./target/release/vb-snapshot create --provider lvm --label "${SNAP_NAME}" /dev/${VG_NAME}/${LV_NAME}
./target/release/vb-snapshot list --provider lvm /dev/${VG_NAME}/${LV_NAME}
./target/release/vb-mount mount --provider lvm --target "${SNAPSHOT_MOUNT}" /dev/${VG_NAME}/${SNAP_NAME}
grep -q 'hello-from-lvm' "${SNAPSHOT_MOUNT}/hello.txt"
./target/release/vb-mount unmount --provider lvm "${SNAPSHOT_MOUNT}"
./target/release/vb-backup --provider lvm --output "${STREAM_PATH}" /dev/${VG_NAME}/${LV_NAME}
./target/release/vb-restore --provider lvm --force --input "${STREAM_PATH}" /dev/${VG_NAME}/${RESTORE_LV_NAME}
./target/release/vb-snapshot delete --provider lvm /dev/${VG_NAME}/${SNAP_NAME}

if [[ "${ASSERT_RESTORE_CONTENTS}" == "1" ]]; then
  mount "/dev/${VG_NAME}/${RESTORE_LV_NAME}" "${RESTORE_MOUNT}"
  grep -q 'hello-from-lvm' "${RESTORE_MOUNT}/hello.txt"
  umount "${RESTORE_MOUNT}"
fi

if [[ "${ASSERT_SNAPSHOT_CLEANUP}" == "1" ]]; then
  SNAPSHOT_LIST_OUTPUT="$(./target/release/vb-snapshot list --provider lvm /dev/${VG_NAME}/${LV_NAME})"
  if grep -Fq "/dev/${VG_NAME}/${SNAP_NAME}" <<<"${SNAPSHOT_LIST_OUTPUT}"; then
    echo "lvm snapshot still listed after delete" >&2
    exit 1
  fi
fi

echo "lvm roundtrip ok"
