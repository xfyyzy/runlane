# Contributing to Runlane

Runlane is currently in project-definition and v0.1 kernel-design stage.

Before implementing, read:

1. `docs/project-charter.md`
2. `docs/product-definition.md`
3. `docs/operational-layer-model.md`
4. `docs/execution-semantics.md`
5. `docs/platform-model.md`
6. `docs/coding-agent-brief.md`

## Development checks

```bash
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
