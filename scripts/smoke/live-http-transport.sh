#!/usr/bin/env bash
set -euo pipefail

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    echo "repair: install $1 and rerun scripts/smoke/live-http-transport.sh" >&2
    exit 2
  fi
}

need_cmd cargo
need_cmd curl
need_cmd jq

addr="${RUNLANE_HTTP_SMOKE_ADDR:-127.0.0.1:17890}"
base_url="http://${addr}"
tmpdir="$(mktemp -d "${TMPDIR:-/tmp}/runlane-http-smoke.XXXXXX")"
server_log="${tmpdir}/server.log"
server_pid=""

cleanup() {
  if [[ -n "${server_pid}" ]] && kill -0 "${server_pid}" >/dev/null 2>&1; then
    kill "${server_pid}" >/dev/null 2>&1 || true
    wait "${server_pid}" >/dev/null 2>&1 || true
  fi
  rm -rf "${tmpdir}"
}

trap cleanup EXIT

cargo run -p runlane-server -- http demo-serve "${addr}" >"${server_log}" 2>&1 &
server_pid="$!"

for _ in $(seq 1 100); do
  if ! kill -0 "${server_pid}" >/dev/null 2>&1; then
    echo "runlane-server exited before accepting requests" >&2
    sed -n '1,160p' "${server_log}" >&2 || true
    exit 1
  fi
  status="$(curl -sS -o /dev/null -w '%{http_code}' "${base_url}/v1/enrollment/tokens" 2>/dev/null || true)"
  if [[ "${status}" != "000" ]]; then
    break
  fi
  sleep 0.1
done

request_json() {
  local name="$1"
  local expected_status="$2"
  local jq_filter="$3"
  local method="$4"
  local url="$5"
  local payload="$6"
  shift 6
  local body_file="${tmpdir}/${name}.json"
  local status

  status="$(
    curl -sS \
      -o "${body_file}" \
      -w '%{http_code}' \
      -X "${method}" \
      -H 'content-type: application/json' \
      "$@" \
      --data "${payload}" \
      "${url}"
  )"

  if [[ "${status}" != "${expected_status}" ]]; then
    echo "${name}: expected HTTP ${expected_status}, got ${status}" >&2
    cat "${body_file}" >&2 || true
    exit 1
  fi

  if ! jq -e "${jq_filter}" "${body_file}" >/dev/null; then
    echo "${name}: response did not match jq filter ${jq_filter}" >&2
    cat "${body_file}" >&2 || true
    exit 1
  fi
}

request_json \
  create-token \
  200 \
  '.status == "created" and .token_id == "token-smoke-node-01" and .node_id == "smoke-node-01"' \
  POST \
  "${base_url}/v1/enrollment/tokens" \
  '{"token_id":"token-smoke-node-01","token":"smoke-token-material","node_id":"smoke-node-01","platform_family":"linux","server_trust_root":"smoke-trust-root","expires_at_unix_seconds":300,"nonce":"smoke-enroll-nonce"}'

request_json \
  enroll \
  200 \
  '.node_id == "smoke-node-01" and .platform_family == "linux" and .certificate_fingerprint == "smoke-cert-fingerprint"' \
  POST \
  "${base_url}/v1/agent/enroll" \
  '{"token":"smoke-token-material","node_id":"smoke-node-01","platform_family":"linux","certificate_fingerprint":"smoke-cert-fingerprint","server_trust_root":"smoke-trust-root","now_unix_seconds":100}'

request_json \
  missing-identity \
  401 \
  '.kind == "missing_identity"' \
  POST \
  "${base_url}/v1/agent/pull" \
  '{"node_id":"prod-web-01","now_unix_seconds":101,"capability_report_version":"cap-smoke","last_seen_task_nonce":null}'

request_json \
  pull-first \
  200 \
  '.envelope.envelope_id == "env-1" and .payload.type == "collect_evidence" and .payload.capability == "service.systemd"' \
  POST \
  "${base_url}/v1/agent/pull" \
  '{"node_id":"prod-web-01","now_unix_seconds":101,"capability_report_version":"cap-smoke","last_seen_task_nonce":null}' \
  -H 'x-runlane-node-id: prod-web-01' \
  -H 'x-runlane-certificate-fingerprint: demo-cert-fingerprint'

request_json \
  submit-result \
  200 \
  '.status == "accepted" and .envelope_id == "env-1" and .task_id == "collect-service-status"' \
  POST \
  "${base_url}/v1/agent/result" \
  '{"envelope_id":"env-1","run_id":"run-1","task_id":"collect-service-status","node_id":"prod-web-01","nonce":"nonce-1","status":"succeeded","now_unix_seconds":102,"evidence":[{"source":"service_status","content_type":"text/plain","body":"sshd active from live HTTP smoke","truncated":false}],"audit_correlation_id":"audit-1"}' \
  -H 'x-runlane-node-id: prod-web-01' \
  -H 'x-runlane-certificate-fingerprint: demo-cert-fingerprint'

request_json \
  pull-second \
  200 \
  '.envelope.envelope_id == "env-2" and .payload.type == "collect_evidence"' \
  POST \
  "${base_url}/v1/agent/pull" \
  '{"node_id":"prod-web-01","now_unix_seconds":103,"capability_report_version":"cap-smoke","last_seen_task_nonce":"nonce-1"}' \
  -H 'x-runlane-node-id: prod-web-01' \
  -H 'x-runlane-certificate-fingerprint: demo-cert-fingerprint'

request_json \
  spool-replay \
  200 \
  '.status == "accepted" and .spool_id == "spool-live-http-smoke-1"' \
  POST \
  "${base_url}/v1/agent/spool/replay" \
  '{"spool_id":"spool-live-http-smoke-1","reason":"live smoke replay path","now_unix_seconds":104,"result":{"envelope_id":"env-2","run_id":"run-1","task_id":"collect-service-status-2","node_id":"prod-web-01","nonce":"nonce-2","status":"succeeded","now_unix_seconds":104,"evidence":[{"source":"service_status","content_type":"text/plain","body":"sshd active from live HTTP replay smoke","truncated":false}],"audit_correlation_id":"audit-2"}}' \
  -H 'x-runlane-node-id: prod-web-01' \
  -H 'x-runlane-certificate-fingerprint: demo-cert-fingerprint'

echo "live HTTP transport smoke ok; addr=${addr}"
