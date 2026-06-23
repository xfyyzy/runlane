# Verification Matrix

Runlane verification claims must say what actually ran. Do not use generic
"verified" language without command output, CI links, or a clear not-run reason.

## Claim Vocabulary

- **CI**: executed by a GitHub Actions workflow on the PR or branch.
- **local**: executed by the contributor on their development machine.
- **manual/VM**: executed on a named VM or target OS outside current CI.
- **not run**: intentionally not executed; include the reason.
- **blocked**: could not execute because a prerequisite was missing; include
  the failed prerequisite and repair path.

If a check is not run, leave the PR checkbox unchecked and state why.

## Current Required PR Checks

Current GitHub branch protection requires these checks before merging to
`main`:

| Check | Source | What it proves |
|---|---|---|
| `rust` | `.github/workflows/ci.yml` on Ubuntu | `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass on the current stable Rust toolchain available to the workflow. |
| `pr-policy` | `.github/workflows/pr-policy.yml` | PR body links an issue, contains required sections, and has at least one checked self-review item. |

Current CI does **not** build release artifacts, run Linux musl cross builds,
run FreeBSD VM smoke, or run OpenBSD native VM validation.

## Baseline Local Checks

Run these before opening a PR unless a preflight failure blocks them:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

For PR policy changes, also run:

```bash
scripts/ci/check-pr-policy.sh docs/process/pr-policy-fixtures/passing.md
if scripts/ci/check-pr-policy.sh docs/process/pr-policy-fixtures/failing-unchecked.md; then
  echo "unexpected pass"
  exit 1
else
  echo "expected failure"
fi
```

## Recommended Local Smoke

Use targeted smoke commands when the change touches the corresponding surface:

| Surface | Smoke command examples |
|---|---|
| Fleet parsing or examples | `cargo run -p runlane -- fleet validate examples/fleet` and `cargo run -p runlane -- server gitops sync examples/fleet` |
| Agent/server pull loop | `cargo run -p runlane-server -- demo-control-plane` and `cargo run -p runlane-agent -- demo-enroll-pull` |
| Agent/server HTTP transport | `cargo test -p runlane-server http -- --nocapture` and `scripts/smoke/live-http-transport.sh`, which starts `runlane-server http demo-serve` on loopback, exercises enrollment/pull/result/spool replay, validates JSON, checks a missing-identity fail-closed response, and tears the server down |
| Agent native collectors | `cargo test -p runlane-agent platform -- --nocapture` and `cargo run -p runlane-agent -- collect-smoke --service sshd` |
| Approval API/CLI | `cargo run -p runlane -- approval list`, `show`, `approve`, and `reject`; for Telegram adapter approval-channel evidence, run `scripts/smoke/telegram-approval-live-simulated.sh` and report whether the result was live Telegram, live-simulated, or blocked |
| Helper boundary | `cargo run -p runlane-helper -- --help`, `cargo run -p runlane-helper -- preflight --helper-binary target/debug/runlane-helper --allowlist-file examples/helper-smoke/allowlist.yaml --expected-owner-uid "$(id -u)" --expected-mode "$(stat -c %a target/debug/runlane-helper)"`, `cargo run -p runlane-helper -- dry-run-smoke --lease-file examples/helper-smoke/lease-valid.yaml --request-file examples/helper-smoke/request-restart.yaml --allowlist-file examples/helper-smoke/allowlist.yaml --node-id prod-web-01 --now 1780000000`, and `RUNLANE_HELPER_SMOKE_USER=runlane scripts/smoke/linux-helper-install.sh` when a real Linux sudo install boundary is in scope; include one rejection fixture when helper request logic changes |
| E2E receipt path | `cargo run -p runlane -- demo service-unhealthy examples/fleet`, `cargo run -p runlane -- receipt show run-demo-service-unhealthy examples/fleet`, `cargo run -p runlane -- demo disk-pressure examples/fleet`, and `cargo run -p runlane -- receipt show run-demo-disk-pressure examples/fleet` |
| Linux real-host service-unhealthy dogfood | `scripts/smoke/linux-service-unhealthy-dogfood.sh` on a Linux/systemd host with passwordless sudo for setup and cleanup; the script targets only `runlane-demo-unhealthy.service`, validates helper dry-run behavior, persists local state, and verifies `cargo run -p runlane -- receipt show run-real-host-service-unhealthy <state-dir>` |

## Cross-Build And VM Checks

Cross-platform validation must keep build and runtime baselines aligned.

Run these checks when the change touches platform backends, helper behavior,
agent/server binaries, release packaging, target-specific build configuration,
or cross-platform semantics.

| Target | When required | Evidence to report |
|---|---|---|
| Linux x86_64 musl | Release artifact or Linux static binary behavior changes | `scripts/release/linux-x86_64-musl.sh`; it checks the `x86_64-unknown-linux-musl` Rust target, builds the workspace in release mode, rejects artifacts with `PT_INTERP` or `DT_NEEDED`, and writes checksums plus `file` output under `target/release-evidence/` |
| Linux aarch64 musl | aarch64 release artifact or linker/build configuration changes | `cargo build --workspace --target aarch64-unknown-linux-musl --release` plus `codex-assert-static-elf` |
| FreeBSD x86_64 | FreeBSD backend/helper behavior or FreeBSD release artifact changes | FreeBSD release-aligned cross build using the current stable FreeBSD sysroot, static artifact check, and `scripts/smoke/freebsd-vm-validation.sh` inside a FreeBSD VM when runtime behavior changed; the VM smoke records OS/Rust versions, runs workspace fmt/check/test, server HTTP tests, agent `collect-smoke`, and FreeBSD sudo helper preflight/dry-run/rejection checks |
| OpenBSD x86_64 | OpenBSD backend/helper behavior or OpenBSD release validation | `scripts/smoke/openbsd-vm-validation.sh` inside a native OpenBSD VM; it records OS/Rust versions, runs workspace fmt/check/test, server HTTP tests, agent `collect-smoke`, and OpenBSD `doas` helper preflight/dry-run/rejection checks |

OpenBSD remains a first-class target, but the default project path is native VM
validation because stable Rust does not currently provide a rustup-installed
`x86_64-unknown-openbsd` standard library suitable for this project. Do not
describe a Linux-hosted OpenBSD cross build as completed unless a stable,
reproducible toolchain has actually been introduced and run.

## Reporting Format

In PRs and issue comments, prefer this shape:

```text
Verification:
- CI rust: passed, <Actions URL>
- CI pr-policy: passed, <Actions URL>
- Local: cargo fmt --all -- --check, cargo check --workspace, cargo clippy --workspace --all-targets -- -D warnings, cargo test --workspace
- Cross/VM: not run; docs-only change did not touch platform runtime or release artifacts
```

For failures:

```text
Blocked:
- OpenBSD VM validation not run because <specific preflight failure>.
- Repair path: <command or user decision needed>.
```

Do not turn a not-run or blocked check into a success claim.
