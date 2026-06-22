# Contributing to Runlane

Runlane is currently in project-definition and v0.1 kernel-design stage.

Before implementing, read:

1. `AGENTS.md`
2. `docs/process/coding-agent-pr-workflow.md`
3. `docs/project-charter.md`
4. `docs/product-definition.md`
5. `docs/operational-layer-model.md`
6. `docs/execution-semantics.md`
7. `docs/platform-model.md`
8. `docs/verification-matrix.md`
9. `docs/coding-agent-brief.md`

`AGENTS.md` defines the repository execution rules for human-supervised agents: no silent fallback, explicit failure, single-path convergence, prompt commits, tmux for long-running tasks, and verifiable handoff.

## Required PR workflow

Runtime, documentation, process, CI, and issue-template changes must use the
issue branch and PR workflow:

```bash
git switch main
git pull --ff-only
git switch -c issue-<number>-<short-slug>
```

Commit only coherent semantic changes on that issue branch, push it, and open a
PR that includes `Closes #<number>`. Fill the PR template with real command
output, a self-review checklist, docs impact, and remaining risks. Direct
mutation of `main` is not the valid contribution path.

## Toolchain requirement

Runlane currently requires **Rust stable 1.96.0 or newer**.

- `Cargo.toml` is the source of truth for the minimum supported Rust version: `rust-version = "1.96"`.
- `rust-toolchain.toml` uses the floating `stable` channel so contributors naturally build with the current stable toolchain.

## Development checks

```bash
rustc --version
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
```

Current CI runs the Rust checks above on Ubuntu and runs the PR body policy
check. It does not run release cross-builds or BSD VM smoke. Use
`docs/verification-matrix.md` to decide which local, manual, cross-build, or VM
checks apply to a change, and report checks as run, not run, or blocked with
real command output or CI links.

## Design rules

- Do not add Linux/systemd-only assumptions to core domain types.
- Do not represent privileged actions as arbitrary shell strings.
- Do not build chat integrations before core scheduler, leases, and verification semantics.
- Do not run blanket full verification by default; verification must be impact-scoped.
- Do not collapse system/platform/application layers into one resource kind.
- Platform-specific parsing belongs in backend modules with fixtures.
- Any feature that changes scheduling, leases, capabilities, or verification must update docs.

## Commit style

Use concise conventional-style messages:

```text
feat(core): add operational layer model
docs: define resource lease scheduling
chore(ci): add rust workspace checks
```
