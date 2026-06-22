#!/bin/sh
set -eu

fail() {
  echo "FreeBSD VM validation failed: $*" >&2
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
  if [ -x /usr/local/sbin/visudo ]; then
    echo /usr/local/sbin/visudo
    return
  fi
  if [ -x /usr/sbin/visudo ]; then
    echo /usr/sbin/visudo
    return
  fi
  fail "missing required command: visudo"
}

repo_root="$(CDPATH= cd "$(dirname "$0")/../.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-${repo_root}/target}"
case "${target_dir}" in
  /*) ;;
  *) target_dir="${repo_root}/${target_dir}" ;;
esac

smoke_user="${RUNLANE_HELPER_SMOKE_USER:-runlane}"
helper_path="${RUNLANE_FREEBSD_HELPER_INSTALL_PATH:-/usr/local/libexec/runlane-helper}"
allowlist_path="${RUNLANE_FREEBSD_HELPER_ALLOWLIST_PATH:-/usr/local/etc/runlane/helper-allowlist.yaml}"
sudoers_path="${RUNLANE_FREEBSD_HELPER_SUDOERS_PATH:-/usr/local/etc/sudoers.d/runlane-helper}"
helper_artifact="${target_dir}/debug/runlane-helper"
tmpdir=""
had_helper=0
had_allowlist=0
had_sudoers=0
had_helper_dir=0
had_allowlist_dir=0
had_sudoers_dir=0

backup_file() {
  path="$1"
  backup="$2"
  flag_name="$3"

  if sudo test -e "${path}"; then
    sudo cp -p "${path}" "${backup}"
    eval "${flag_name}=1"
  fi
}

restore_file() {
  path="$1"
  backup="$2"
  had_file="$3"

  if [ "${had_file}" = "1" ]; then
    sudo install -d -o root -g wheel -m 0755 "$(dirname "${path}")"
    sudo cp -p "${backup}" "${path}"
  else
    sudo rm -f "${path}"
  fi
}

cleanup() {
  status=$?
  if [ -n "${tmpdir}" ]; then
    restore_file "${sudoers_path}" "${tmpdir}/sudoers.backup" "${had_sudoers}"
    restore_file "${helper_path}" "${tmpdir}/runlane-helper.backup" "${had_helper}"
    restore_file "${allowlist_path}" "${tmpdir}/helper-allowlist.backup" "${had_allowlist}"
    if [ "${had_helper_dir}" = "0" ]; then
      sudo rmdir "$(dirname "${helper_path}")" >/dev/null 2>&1 || true
    fi
    if [ "${had_allowlist_dir}" = "0" ]; then
      sudo rmdir "$(dirname "${allowlist_path}")" >/dev/null 2>&1 || true
    fi
    if [ "${had_sudoers_dir}" = "0" ]; then
      sudo rmdir "$(dirname "${sudoers_path}")" >/dev/null 2>&1 || true
    fi
    rm -rf "${tmpdir}"
  fi
  if [ "${status}" = "0" ]; then
    echo "teardown restored prior FreeBSD helper, allowlist, and sudoers state"
  fi
  exit "${status}"
}

trap cleanup EXIT INT TERM

[ "$(uname -s)" = "FreeBSD" ] || fail "this smoke must run on FreeBSD"

need_cmd cargo
need_cmd df
need_cmd grep
need_cmd id
need_cmd install
need_cmd mktemp
need_cmd procstat
need_cmd ps
need_cmd rustc
need_cmd rustfmt
need_cmd service
need_cmd sockstat
need_cmd sudo
visudo_cmd="$(find_visudo)"

id "${smoke_user}" >/dev/null 2>&1 ||
  fail "smoke user ${smoke_user} does not exist; create a dedicated agent user outside this script or set RUNLANE_HELPER_SMOKE_USER"

sudo -n true >/dev/null 2>&1 ||
  fail "current user cannot run sudo -n for helper installation and cleanup"

if sudo -n -u "${smoke_user}" -- sudo -n /bin/sh -c true >/dev/null 2>&1; then
  fail "smoke user ${smoke_user} can already run an arbitrary root shell through sudo; choose a narrower user"
fi

echo "os: $(uname -a)"
echo "rustc: $(rustc --version)"
echo "cargo: $(cargo --version)"
echo "rustfmt: $(rustfmt --version)"

cd "${repo_root}"
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo test -p runlane-server http -- --nocapture
cargo run -p runlane-agent -- collect-smoke --service sshd
cargo build -p runlane-helper

tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/runlane-freebsd-helper-smoke.XXXXXX")"
sudo test -d "$(dirname "${helper_path}")" && had_helper_dir=1
sudo test -d "$(dirname "${allowlist_path}")" && had_allowlist_dir=1
sudo test -d "$(dirname "${sudoers_path}")" && had_sudoers_dir=1
backup_file "${helper_path}" "${tmpdir}/runlane-helper.backup" had_helper
backup_file "${allowlist_path}" "${tmpdir}/helper-allowlist.backup" had_allowlist
backup_file "${sudoers_path}" "${tmpdir}/sudoers.backup" had_sudoers

sudo install -d -o root -g wheel -m 0755 "$(dirname "${helper_path}")"
sudo install -o root -g wheel -m 0755 "${helper_artifact}" "${helper_path}"
sudo install -d -o root -g wheel -m 0755 "$(dirname "${allowlist_path}")"
sudo install -o root -g wheel -m 0644 \
  "${repo_root}/examples/helper-smoke/allowlist.yaml" \
  "${allowlist_path}"
sudo install -d -o root -g wheel -m 0755 "$(dirname "${sudoers_path}")"
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

echo "freebsd vm validation ok; user=${smoke_user}; helper=${helper_path}"
