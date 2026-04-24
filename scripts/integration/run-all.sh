#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

if [[ "${EUID}" -ne 0 ]]; then
  echo "run this script as root" >&2
  exit 1
fi

run_step() {
  local name="$1"
  shift
  echo "==> ${name}"
  "$@"
  echo
}

run_step "btrfs roundtrip" bash scripts/integration/btrfs-roundtrip.sh
run_step "lvm snapshot lifecycle" bash scripts/integration/lvm-snapshot.sh
run_step "zfs roundtrip" bash scripts/integration/zfs-roundtrip.sh

echo "all integration scripts completed"
