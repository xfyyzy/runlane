#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
usage: scripts/smoke/linux-service-unhealthy-dogfood.sh

Creates a fixed, controlled failing systemd demo service, collects real Linux
host evidence, runs the service-unhealthy dogfood path in dry-run helper mode,
renders the receipt back from durable local state, and removes the demo service.

This script never restarts a production service. The only service target is:
  runlane-demo-unhealthy.service

Environment:
  RUNLANE_DOGFOOD_NODE_ID   Node id to record in audit events, default: prod-web-01
USAGE
}

fail() {
  echo "linux service-unhealthy dogfood failed: $*" >&2
  exit 1
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
service_name="runlane-demo-unhealthy.service"
unit_path="/etc/systemd/system/${service_name}"
node_id="${RUNLANE_DOGFOOD_NODE_ID:-prod-web-01}"
service_resource="system:node/${node_id}/service/${service_name}"
tmpdir=""
state_dir=""

cleanup() {
  local status=$?
  if command -v systemctl >/dev/null 2>&1; then
    sudo systemctl reset-failed "${service_name}" >/dev/null 2>&1 || true
    sudo rm -f "${unit_path}" >/dev/null 2>&1 || true
    sudo systemctl daemon-reload >/dev/null 2>&1 || true
  fi
  if [[ -n "${tmpdir}" ]]; then
    rm -rf "${tmpdir}"
  fi
  if [[ "${status}" == "0" ]]; then
    echo "teardown removed controlled demo service and durable smoke state"
  fi
  exit "${status}"
}

trap cleanup EXIT INT TERM

while [[ $# -gt 0 ]]; do
  case "$1" in
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
[[ "${node_id}" =~ ^[A-Za-z0-9._:-]+$ ]] ||
  fail "RUNLANE_DOGFOOD_NODE_ID must be a simple node id"

need_cmd cargo
need_cmd df
need_cmd grep
need_cmd journalctl
need_cmd mktemp
need_cmd ps
need_cmd ss
need_cmd sudo
need_cmd systemctl
need_cmd uname

sudo -n true >/dev/null 2>&1 ||
  fail "current user cannot run sudo -n for controlled demo service setup and cleanup"

if sudo test -e "${unit_path}" || systemctl cat "${service_name}" >/dev/null 2>&1; then
  fail "controlled demo service ${service_name} already exists; remove it before rerunning"
fi

tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/runlane-service-dogfood.XXXXXX")"
state_dir="${tmpdir}/state"

sudo tee "${unit_path}" >/dev/null <<'UNIT'
[Unit]
Description=Runlane controlled unhealthy dogfood demo service

[Service]
Type=oneshot
ExecStart=/bin/sh -c 'echo runlane controlled demo failure >&2; exit 1'
UNIT

sudo systemctl daemon-reload
if sudo systemctl start "${service_name}" >/dev/null 2>&1; then
  fail "controlled demo service unexpectedly started successfully"
fi
sudo systemctl is-failed --quiet "${service_name}" ||
  fail "controlled demo service did not enter failed state"

(
  cd "${repo_root}"
  cargo run -p runlane-agent -- collect-smoke --service "${service_name}"
  cargo run -p runlane-agent -- dogfood-service-unhealthy \
    --service "${service_name}" \
    --state-dir "${state_dir}" \
    --node-id "${node_id}"
  cargo run -p runlane -- receipt show run-real-host-service-unhealthy "${state_dir}" |
    tee "${tmpdir}/receipt.out"
)

grep -Fq "changed: ${service_resource}" \
  "${tmpdir}/receipt.out" ||
  fail "receipt did not record the controlled service resource"
grep -Fq "service_active:${service_resource}" \
  "${tmpdir}/receipt.out" ||
  fail "receipt did not record service_active verification"
grep -Fq 'skipped: firewall_audit' "${tmpdir}/receipt.out" ||
  fail "receipt did not record skipped checks with reasons"
grep -Fq 'residual_risk:' "${tmpdir}/receipt.out" ||
  fail "receipt did not record residual risk"
grep -Fq 'takeover:' "${tmpdir}/receipt.out" ||
  fail "receipt did not record takeover path"

echo "linux service-unhealthy dogfood ok; service=${service_name}; mode=real-host-dry-run; production_restart=false"
