#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE_DIR="${IMAGE_DIR:-/opt/volumeset}"
COPY_DIR="${COPY_DIR:-/opt/volumeset/copy}"
MOUNT_ROOT="${MOUNT_ROOT:-/mnt/volmnt}"
ASSERT_RESTORE_CONTENTS="${ASSERT_RESTORE_CONTENTS:-1}"
ASSERT_SNAPSHOT_CLEANUP="${ASSERT_SNAPSHOT_CLEANUP:-1}"
TEST_NAME="vpt-zfs"
IMAGE_PATH="${IMAGE_DIR}/${TEST_NAME}.img"
POOL_NAME="vptpool"
DATASET_NAME="${POOL_NAME}/data"
RESTORE_DATASET="${POOL_NAME}/restore"
DATASET_MOUNT="${MOUNT_ROOT}/${TEST_NAME}-data"
RESTORE_MOUNT="${MOUNT_ROOT}/${TEST_NAME}-restore"
SNAPSHOT_MOUNT="${MOUNT_ROOT}/${TEST_NAME}-snapshot"
STREAM_PATH="${COPY_DIR}/${TEST_NAME}.zfs"

cleanup() {
  set +e
  umount "${SNAPSHOT_MOUNT}" >/dev/null 2>&1 || true
  zpool destroy -f "${POOL_NAME}" >/dev/null 2>&1 || true
  rm -rf "${DATASET_MOUNT}" "${RESTORE_MOUNT}" "${SNAPSHOT_MOUNT}"
  rm -f "${IMAGE_PATH}" "${STREAM_PATH}"
}
trap cleanup EXIT

if [[ "${EUID}" -ne 0 ]]; then
  echo "run this script as root (for zpool creation and mounted dataset management)" >&2
  exit 1
fi

mkdir -p "${IMAGE_DIR}" "${COPY_DIR}" "${MOUNT_ROOT}"
truncate -s 2G "${IMAGE_PATH}"
zpool create -f "${POOL_NAME}" "${IMAGE_PATH}"
zfs create -o mountpoint="${DATASET_MOUNT}" "${DATASET_NAME}"
zfs create -o mountpoint="${RESTORE_MOUNT}" "${RESTORE_DATASET}"
mkdir -p "${SNAPSHOT_MOUNT}"
printf 'hello-from-zfs\n' > "${DATASET_MOUNT}/hello.txt"
zfs snapshot "${DATASET_NAME}@snap1"

cd "${ROOT_DIR}"
./target/release/vb-snapshot list --provider zfs "${DATASET_NAME}"
COPY_MOUNT_OUTPUT="$(./target/release/vb-copy-mount open --provider zfs --label copyview --target "${SNAPSHOT_MOUNT}" "${DATASET_NAME}")"
COPY_MOUNT_SNAPSHOT="$(awk '/^snapshot:/ {print $2}' <<<"${COPY_MOUNT_OUTPUT}")"
grep -q 'hello-from-zfs' "${SNAPSHOT_MOUNT}/hello.txt"
./target/release/vb-copy-mount close --provider zfs "${COPY_MOUNT_SNAPSHOT}" "${SNAPSHOT_MOUNT}"
./target/release/vb-backup --provider zfs --snapshot-source --output "${STREAM_PATH}" "${DATASET_NAME}@snap1"
./target/release/vb-restore --provider zfs --force --input "${STREAM_PATH}" "${RESTORE_DATASET}"

zfs list "${RESTORE_DATASET}" >/dev/null
if [[ "${ASSERT_RESTORE_CONTENTS}" == "1" ]]; then
  grep -q 'hello-from-zfs' "${RESTORE_MOUNT}/hello.txt"
fi

if [[ "${ASSERT_SNAPSHOT_CLEANUP}" == "1" ]]; then
  ./target/release/vb-snapshot delete --provider zfs "${DATASET_NAME}@snap1"
  SNAPSHOT_LIST_OUTPUT="$(./target/release/vb-snapshot list --provider zfs "${DATASET_NAME}")"
  if grep -Fq "${DATASET_NAME}@snap1" <<<"${SNAPSHOT_LIST_OUTPUT}"; then
    echo "zfs snapshot still listed after delete" >&2
    exit 1
  fi
fi

echo "zfs roundtrip ok"
