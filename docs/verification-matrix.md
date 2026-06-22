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
| `rust` | `.github/workflows/ci.yml` on Ubuntu | `cargo fmt --all -- --check`, `cargo check --workspace`, and `cargo test --workspace` pass on the current stable Rust toolchain available to the workflow. |
| `pr-policy` | `.github/workflows/pr-policy.yml` | PR body links an issue, contains required sections, and has at least one checked self-review item. |

Current CI does **not** build release artifacts, run Linux musl cross builds,
run FreeBSD VM smoke, or run OpenBSD native VM validation.

## Baseline Local Checks

Run these before opening a PR unless a preflight failure blocks them:

```bash
cargo fmt --all -- --check
cargo check --workspace
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
| Agent/server HTTP transport | `cargo test -p runlane-server http -- --nocapture` and, when a live smoke is useful, `cargo run -p runlane-server -- http demo-serve 127.0.0.1:17890` |
| Approval API/CLI | `cargo run -p runlane -- approval list`, `show`, `approve`, and `reject` |
| Helper boundary | `cargo run -p runlane-helper -- --help` plus a dry-run accept/reject fixture when helper request logic changes |
| E2E receipt path | `cargo run -p runlane -- demo service-unhealthy examples/fleet` and `cargo run -p runlane -- receipt show run-demo-service-unhealthy examples/fleet` |

## Cross-Build And VM Checks

Cross-platform validation must keep build and runtime baselines aligned.

Run these checks when the change touches platform backends, helper behavior,
agent/server binaries, release packaging, target-specific build configuration,
or cross-platform semantics.

| Target | When required | Evidence to report |
|---|---|---|
| Linux x86_64 musl | Release artifact or Linux static binary behavior changes | `cargo build --workspace --target x86_64-unknown-linux-musl --release` plus `codex-assert-static-elf` for produced binaries |
| Linux aarch64 musl | aarch64 release artifact or linker/build configuration changes | `cargo build --workspace --target aarch64-unknown-linux-musl --release` plus `codex-assert-static-elf` |
| FreeBSD x86_64 | FreeBSD backend/helper behavior or FreeBSD release artifact changes | FreeBSD release-aligned cross build using the current stable FreeBSD sysroot, static artifact check, and FreeBSD VM smoke when runtime behavior changed |
| OpenBSD x86_64 | OpenBSD backend/helper behavior or OpenBSD release validation | Native OpenBSD VM `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace`, and relevant binary smoke |

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
- Local: cargo fmt --all -- --check, cargo check --workspace, cargo test --workspace
- Cross/VM: not run; docs-only change did not touch platform runtime or release artifacts
```

For failures:

```text
Blocked:
- OpenBSD VM validation not run because <specific preflight failure>.
- Repair path: <command or user decision needed>.
```

Do not turn a not-run or blocked check into a success claim.
