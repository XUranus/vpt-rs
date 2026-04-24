#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE_DIR="${IMAGE_DIR:-/opt/volumeset}"
MOUNT_ROOT="${MOUNT_ROOT:-/mnt/volmnt}"
TEST_NAME="vpt-lvm"
IMAGE_PATH="${IMAGE_DIR}/${TEST_NAME}.img"
LOOP_DEVICE=""
VG_NAME="vptvg"
LV_NAME="data"
SNAP_NAME="integ-lvm"

cleanup() {
  set +e
  cargo run --manifest-path "${ROOT_DIR}/Cargo.toml" --bin vb-snapshot -- delete --provider lvm "/dev/${VG_NAME}/${SNAP_NAME}" >/dev/null 2>&1 || true
  lvremove -fy "/dev/${VG_NAME}/${LV_NAME}" >/dev/null 2>&1 || true
  vgremove -fy "${VG_NAME}" >/dev/null 2>&1 || true
  pvremove -fy "${LOOP_DEVICE}" >/dev/null 2>&1 || true
  if [[ -n "${LOOP_DEVICE}" ]]; then
    losetup -d "${LOOP_DEVICE}" >/dev/null 2>&1 || true
  fi
  rm -f "${IMAGE_PATH}"
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

cd "${ROOT_DIR}"
./target/release/vb-snapshot create --provider lvm --label "${SNAP_NAME}" /dev/${VG_NAME}/${LV_NAME}
./target/release/vb-snapshot list --provider lvm /dev/${VG_NAME}/${LV_NAME}
./target/release/vb-snapshot delete --provider lvm /dev/${VG_NAME}/${SNAP_NAME}

echo "lvm snapshot lifecycle ok"
