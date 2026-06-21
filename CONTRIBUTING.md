# Contributing to Runlane

Runlane is currently in project-definition and v0.1 kernel-design stage.

Before implementing, read:

1. `AGENTS.md`
2. `docs/project-charter.md`
3. `docs/product-definition.md`
4. `docs/operational-layer-model.md`
5. `docs/execution-semantics.md`
6. `docs/platform-model.md`
7. `docs/coding-agent-brief.md`

`AGENTS.md` defines the repository execution rules for human-supervised agents: no silent fallback, explicit failure, single-path convergence, prompt commits, tmux for long-running tasks, and verifiable handoff.

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
