#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
usage: scripts/smoke/linux-helper-install.sh [--keep-installed]

Builds runlane-helper, installs it as a root-owned Linux helper, grants one
narrow sudoers rule for the smoke user, runs preflight and dry-run validation
through sudo, validates an invalid-signature rejection, then restores the prior
host state by default.

Environment:
  RUNLANE_HELPER_SMOKE_USER       Existing unprivileged user to test, default: runlane
  RUNLANE_HELPER_INSTALL_PATH     Installed helper path, default: /usr/local/libexec/runlane-helper
  RUNLANE_HELPER_ALLOWLIST_PATH   Installed allowlist path, default: /etc/runlane/helper-allowlist.yaml
  RUNLANE_HELPER_SUDOERS_PATH     Sudoers fragment path, default: /etc/sudoers.d/runlane-helper
  RUNLANE_HELPER_BUILD_TARGET     Optional cargo target triple

Options:
  --keep-installed                Leave installed files in place and print rollback commands
  -h, --help                      Show this help
USAGE
}

fail() {
  echo "linux helper install smoke failed: $*" >&2
  exit 1
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

find_visudo() {
  if command -v visudo >/dev/null 2>&1; then
    command -v visudo
    return
  fi
  if [[ -x /usr/sbin/visudo ]]; then
    echo /usr/sbin/visudo
    return
  fi
  if [[ -x /sbin/visudo ]]; then
    echo /sbin/visudo
    return
  fi
  fail "missing required command: visudo"
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
smoke_user="${RUNLANE_HELPER_SMOKE_USER:-runlane}"
helper_path="${RUNLANE_HELPER_INSTALL_PATH:-/usr/local/libexec/runlane-helper}"
allowlist_path="${RUNLANE_HELPER_ALLOWLIST_PATH:-/etc/runlane/helper-allowlist.yaml}"
sudoers_path="${RUNLANE_HELPER_SUDOERS_PATH:-/etc/sudoers.d/runlane-helper}"
build_target="${RUNLANE_HELPER_BUILD_TARGET:-}"
keep_installed=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --keep-installed)
      keep_installed=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      fail "unsupported option: $1"
      ;;
  esac
done

[[ "$(uname -s)" == "Linux" ]] || fail "this smoke must run on Linux"
[[ "${smoke_user}" =~ ^[A-Za-z_][A-Za-z0-9_-]*[$]?$ ]] ||
  fail "RUNLANE_HELPER_SMOKE_USER must be a simple local user name"
[[ "${helper_path}" != *[[:space:]]* ]] ||
  fail "RUNLANE_HELPER_INSTALL_PATH must not contain whitespace because it is embedded in sudoers"

need_cmd cargo
need_cmd grep
need_cmd id
need_cmd install
need_cmd sudo
need_cmd uname
visudo_cmd="$(find_visudo)"

id "${smoke_user}" >/dev/null 2>&1 ||
  fail "smoke user ${smoke_user} does not exist; create a dedicated agent user outside this script or set RUNLANE_HELPER_SMOKE_USER to an existing unprivileged user"

sudo -n true >/dev/null 2>&1 ||
  fail "current user cannot run sudo -n for installation and cleanup"

if sudo -n -u "${smoke_user}" -- sudo -n /bin/sh -c true >/dev/null 2>&1; then
  fail "smoke user ${smoke_user} can already run an arbitrary root shell through sudo; choose a narrower user"
fi

tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/runlane-linux-helper-smoke.XXXXXX")"
helper_backup="${tmpdir}/runlane-helper.backup"
allowlist_backup="${tmpdir}/helper-allowlist.backup"
sudoers_backup="${tmpdir}/sudoers.backup"
had_helper=0
had_allowlist=0
had_sudoers=0
had_helper_dir=0
had_allowlist_dir=0

backup_file() {
  local path="$1"
  local backup="$2"
  local flag_name="$3"

  if sudo test -e "${path}"; then
    sudo cp -a "${path}" "${backup}"
    printf -v "${flag_name}" 1
  fi
}

restore_file() {
  local path="$1"
  local backup="$2"
  local had_file="$3"

  if [[ "${had_file}" == "1" ]]; then
    sudo install -d -o root -g root -m 0755 "$(dirname "${path}")"
    sudo cp -a "${backup}" "${path}"
  else
    sudo rm -f "${path}"
  fi
}

print_rollback() {
  cat <<ROLLBACK
rollback:
  sudo rm -f '${sudoers_path}'
  sudo rm -f '${helper_path}'
  sudo rm -f '${allowlist_path}'
  sudo rmdir '$(dirname "${allowlist_path}")' 2>/dev/null || true
ROLLBACK
}

cleanup() {
  local status=$?
  if [[ "${keep_installed}" == "1" ]]; then
    print_rollback >&2
    rm -rf "${tmpdir}"
    exit "${status}"
  fi

  restore_file "${sudoers_path}" "${sudoers_backup}" "${had_sudoers}"
  restore_file "${helper_path}" "${helper_backup}" "${had_helper}"
  restore_file "${allowlist_path}" "${allowlist_backup}" "${had_allowlist}"
  if [[ "${had_helper_dir}" == "0" ]]; then
    sudo rmdir "$(dirname "${helper_path}")" >/dev/null 2>&1 || true
  fi
  if [[ "${had_allowlist_dir}" == "0" ]]; then
    sudo rmdir "$(dirname "${allowlist_path}")" >/dev/null 2>&1 || true
  fi
  rm -rf "${tmpdir}"

  if [[ "${status}" == "0" ]]; then
    echo "teardown restored prior helper, allowlist, and sudoers state"
  fi
  exit "${status}"
}

trap cleanup EXIT

sudo test -d "$(dirname "${helper_path}")" && had_helper_dir=1
sudo test -d "$(dirname "${allowlist_path}")" && had_allowlist_dir=1
backup_file "${helper_path}" "${helper_backup}" had_helper
backup_file "${allowlist_path}" "${allowlist_backup}" had_allowlist
backup_file "${sudoers_path}" "${sudoers_backup}" had_sudoers

cargo_args=(build -p runlane-helper --release)
helper_artifact="${repo_root}/target/release/runlane-helper"
if [[ -n "${build_target}" ]]; then
  cargo_args+=(--target "${build_target}")
  helper_artifact="${repo_root}/target/${build_target}/release/runlane-helper"
fi

(
  cd "${repo_root}"
  cargo "${cargo_args[@]}"
)

sudo install -D -o root -g root -m 0755 "${helper_artifact}" "${helper_path}"
sudo install -D -o root -g root -m 0644 \
  "${repo_root}/examples/helper-smoke/allowlist.yaml" \
  "${allowlist_path}"

printf '%s ALL=(root) NOPASSWD: %s\n' "${smoke_user}" "${helper_path}" |
  sudo tee "${sudoers_path}" >/dev/null
sudo chmod 0440 "${sudoers_path}"
sudo "${visudo_cmd}" -cf "${sudoers_path}" >/dev/null

if sudo -n -u "${smoke_user}" -- sudo -n /bin/sh -c true >/dev/null 2>&1; then
  fail "sudoers fragment allowed arbitrary shell for ${smoke_user}"
fi

sudo -n -u "${smoke_user}" -- sudo -n "${helper_path}" preflight \
  --helper-binary "${helper_path}" \
  --allowlist-file "${allowlist_path}" \
  --expected-owner-uid 0 \
  --expected-mode 0755

sudo -n -u "${smoke_user}" -- sudo -n "${helper_path}" dry-run-smoke \
  --lease-file "${repo_root}/examples/helper-smoke/lease-valid.yaml" \
  --request-file "${repo_root}/examples/helper-smoke/request-restart.yaml" \
  --allowlist-file "${allowlist_path}" \
  --node-id prod-web-01 \
  --now 1780000000

invalid_log="${tmpdir}/invalid-signature.out"
if sudo -n -u "${smoke_user}" -- sudo -n "${helper_path}" dry-run-smoke \
  --lease-file "${repo_root}/examples/helper-smoke/lease-invalid-signature.yaml" \
  --request-file "${repo_root}/examples/helper-smoke/request-restart.yaml" \
  --allowlist-file "${allowlist_path}" \
  --node-id prod-web-01 \
  --now 1780000000 >"${invalid_log}" 2>&1; then
  cat "${invalid_log}" >&2
  fail "invalid-signature fixture unexpectedly succeeded"
fi

grep -q 'InvalidSignature' "${invalid_log}" ||
  fail "invalid-signature rejection did not mention InvalidSignature"

echo "linux helper install smoke ok; user=${smoke_user}; helper=${helper_path}"
