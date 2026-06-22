#!/bin/sh
set -eu

fail() {
  echo "OpenBSD VM validation failed: $*" >&2
  exit 1
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

repo_root="$(CDPATH= cd "$(dirname "$0")/../.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-${repo_root}/target}"
case "${target_dir}" in
  /*) ;;
  *) target_dir="${repo_root}/${target_dir}" ;;
esac

smoke_user="${RUNLANE_HELPER_SMOKE_USER:-runlane}"
helper_path="${RUNLANE_OPENBSD_HELPER_INSTALL_PATH:-/usr/local/libexec/runlane-helper}"
allowlist_path="${RUNLANE_OPENBSD_HELPER_ALLOWLIST_PATH:-/etc/runlane/helper-allowlist.yaml}"
doas_conf="${RUNLANE_OPENBSD_DOAS_CONF:-/etc/doas.conf}"
helper_artifact="${target_dir}/debug/runlane-helper"
tmpdir=""
had_helper=0
had_allowlist=0
had_doas_conf=0
had_helper_dir=0
had_allowlist_dir=0

backup_file() {
  path="$1"
  backup="$2"
  flag_name="$3"

  if doas test -e "${path}"; then
    doas cp -p "${path}" "${backup}"
    eval "${flag_name}=1"
  fi
}

restore_file() {
  path="$1"
  backup="$2"
  had_file="$3"

  if [ "${had_file}" = "1" ]; then
    doas install -d -o root -g wheel -m 0755 "$(dirname "${path}")"
    doas cp -p "${backup}" "${path}"
  else
    doas rm -f "${path}"
  fi
}

cleanup() {
  status=$?
  if [ -n "${tmpdir}" ]; then
    restore_file "${doas_conf}" "${tmpdir}/doas.conf.backup" "${had_doas_conf}"
    restore_file "${helper_path}" "${tmpdir}/runlane-helper.backup" "${had_helper}"
    restore_file "${allowlist_path}" "${tmpdir}/helper-allowlist.backup" "${had_allowlist}"
    if [ "${had_helper_dir}" = "0" ]; then
      doas rmdir "$(dirname "${helper_path}")" >/dev/null 2>&1 || true
    fi
    if [ "${had_allowlist_dir}" = "0" ]; then
      doas rmdir "$(dirname "${allowlist_path}")" >/dev/null 2>&1 || true
    fi
    rm -rf "${tmpdir}"
  fi
  if [ "${status}" = "0" ]; then
    echo "teardown restored prior OpenBSD helper, allowlist, and doas state"
  fi
  exit "${status}"
}

trap cleanup EXIT INT TERM

[ "$(uname -s)" = "OpenBSD" ] || fail "this smoke must run on OpenBSD"

need_cmd cargo
need_cmd df
need_cmd doas
need_cmd fstat
need_cmd grep
need_cmd id
need_cmd install
need_cmd mktemp
need_cmd ps
need_cmd rcctl
need_cmd rustc
need_cmd rustfmt

id "${smoke_user}" >/dev/null 2>&1 ||
  fail "smoke user ${smoke_user} does not exist; create a dedicated agent user outside this script or set RUNLANE_HELPER_SMOKE_USER"

doas -n true >/dev/null 2>&1 ||
  fail "current user cannot run doas -n for helper installation and cleanup"

if doas -n -u "${smoke_user}" doas -n /bin/sh -c true >/dev/null 2>&1; then
  fail "smoke user ${smoke_user} can already run an arbitrary root shell through doas; choose a narrower user"
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

tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/runlane-openbsd-helper-smoke.XXXXXX")"
doas test -d "$(dirname "${helper_path}")" && had_helper_dir=1
doas test -d "$(dirname "${allowlist_path}")" && had_allowlist_dir=1
backup_file "${helper_path}" "${tmpdir}/runlane-helper.backup" had_helper
backup_file "${allowlist_path}" "${tmpdir}/helper-allowlist.backup" had_allowlist
backup_file "${doas_conf}" "${tmpdir}/doas.conf.backup" had_doas_conf

doas install -d -o root -g wheel -m 0755 "$(dirname "${helper_path}")"
doas install -o root -g wheel -m 0755 "${helper_artifact}" "${helper_path}"
doas install -d -o root -g wheel -m 0755 "$(dirname "${allowlist_path}")"
doas install -o root -g wheel -m 0644 \
  "${repo_root}/examples/helper-smoke/allowlist.yaml" \
  "${allowlist_path}"

if [ "${had_doas_conf}" = "1" ]; then
  doas cat "${doas_conf}" >"${tmpdir}/doas.conf.new"
else
  : >"${tmpdir}/doas.conf.new"
fi
printf '\npermit nopass %s as root cmd %s\n' "${smoke_user}" "${helper_path}" >>"${tmpdir}/doas.conf.new"
doas -C "${tmpdir}/doas.conf.new" >/dev/null
doas install -o root -g wheel -m 0600 "${tmpdir}/doas.conf.new" "${doas_conf}"

if doas -n -u "${smoke_user}" doas -n /bin/sh -c true >/dev/null 2>&1; then
  fail "doas rule allowed arbitrary shell for ${smoke_user}"
fi

doas -n -u "${smoke_user}" doas -n "${helper_path}" preflight \
  --helper-binary "${helper_path}" \
  --allowlist-file "${allowlist_path}" \
  --expected-owner-uid 0 \
  --expected-mode 0755

doas -n -u "${smoke_user}" doas -n "${helper_path}" dry-run-smoke \
  --lease-file "${repo_root}/examples/helper-smoke/lease-valid.yaml" \
  --request-file "${repo_root}/examples/helper-smoke/request-restart.yaml" \
  --allowlist-file "${allowlist_path}" \
  --node-id prod-web-01 \
  --now 1780000000

invalid_log="${tmpdir}/invalid-signature.out"
if doas -n -u "${smoke_user}" doas -n "${helper_path}" dry-run-smoke \
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

echo "openbsd vm validation ok; user=${smoke_user}; helper=${helper_path}"
