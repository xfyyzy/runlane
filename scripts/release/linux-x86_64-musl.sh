#!/usr/bin/env bash
set -euo pipefail

target="x86_64-unknown-linux-musl"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target_dir="${repo_root}/target/${target}/release"
evidence_dir="${repo_root}/target/release-evidence"
evidence_file="${evidence_dir}/linux-x86_64-musl-artifacts.txt"
checksum_file="${evidence_dir}/linux-x86_64-musl.sha256"
artifacts=(runlane runlane-agent runlane-server runlane-helper)

fail() {
  echo "linux x86_64 musl release failed: $*" >&2
  exit 1
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

need_cmd cargo
need_cmd date
need_cmd file
need_cmd grep
need_cmd mkdir
need_cmd mv
need_cmd readelf
need_cmd rustc
need_cmd rustup
need_cmd sha256sum

if ! rustup target list --installed | grep -qx "${target}"; then
  fail "missing Rust target ${target}; repair: rustup target add ${target}"
fi

(
  cd "${repo_root}"
  cargo build --workspace --target "${target}" --release
)

mkdir -p "${evidence_dir}"
evidence_tmp="${evidence_file}.tmp"
checksum_tmp="${checksum_file}.tmp"

{
  echo "# Runlane Linux x86_64 musl release artifacts"
  echo
  echo "generated_at_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "target: ${target}"
  echo "rustc: $(rustc --version)"
  echo "cargo: $(cargo --version)"
  echo "build_command: cargo build --workspace --target ${target} --release"
  echo "static_assertion: each artifact has no PT_INTERP program header and no DT_NEEDED dynamic entry"
  echo "checksum_file: target/release-evidence/linux-x86_64-musl.sha256"
  echo
} >"${evidence_tmp}"

: >"${checksum_tmp}"

for artifact in "${artifacts[@]}"; do
  artifact_path="${target_dir}/${artifact}"
  relative_path="target/${target}/release/${artifact}"
  [[ -x "${artifact_path}" ]] || fail "missing executable artifact ${relative_path}"

  program_headers="$(readelf -lW "${artifact_path}" 2>&1)" ||
    fail "readelf program header check failed for ${relative_path}"
  if grep -q 'INTERP' <<<"${program_headers}"; then
    fail "${relative_path} has a PT_INTERP program header"
  fi

  dynamic_entries="$(readelf -dW "${artifact_path}" 2>&1)" ||
    fail "readelf dynamic section check failed for ${relative_path}"
  if grep -q 'NEEDED' <<<"${dynamic_entries}"; then
    fail "${relative_path} has DT_NEEDED dynamic dependencies"
  fi

  read -r checksum _ < <(sha256sum "${artifact_path}")
  printf '%s  %s\n' "${checksum}" "${relative_path}" >>"${checksum_tmp}"

  {
    echo "artifact: ${relative_path}"
    echo "sha256: ${checksum}"
    echo "file: ${relative_path}: $(file -b "${artifact_path}")"
    echo "static_check: no PT_INTERP; no DT_NEEDED"
    echo
  } >>"${evidence_tmp}"
done

mv "${evidence_tmp}" "${evidence_file}"
mv "${checksum_tmp}" "${checksum_file}"

echo "linux x86_64 musl release artifacts ok; evidence=target/release-evidence/linux-x86_64-musl-artifacts.txt"
