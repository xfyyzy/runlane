# Rust Workspace Structure

## Workspace

```text
runlane/
├── Cargo.toml
├── crates/
│   ├── runlane-core/
│   ├── runlane-agent/
│   ├── runlane-server/
│   └── runlane-helper/
├── docs/
│   ├── architecture.md
│   ├── workspace-structure.md
│   ├── adr/
│   └── milestones/
└── examples/
    ├── inventory/
    └── runbooks/
```

## Crate boundaries

### `runlane-core`

Pure domain model. No OS calls. No network. No DB.

Owns:

- state machine vocabulary;
- typed IDs;
- platform capability vocabulary;
- action and collector enums;
- lease claims model;
- evidence envelope types.

Rule: if it talks to the outside world, it does not belong here.

### `runlane-agent`

Node-side worker.

Owns:

- config loading;
- enrollment;
- mTLS client;
- pull loop;
- platform backend dispatch;
- result submission;
- local spool.

OS-specific code should be isolated under:

```text
crates/runlane-agent/src/platform/
├── mod.rs
├── linux.rs
├── freebsd.rs
└── openbsd.rs
```

### `runlane-server`

Control plane.

Owns:

- HTTP API;
- task scheduler;
- audit ledger;
- GitOps sync;
- policy evaluation;
- approvals;
- incident report generation;
- analyzer/LLM integration.

### `runlane-helper`

Privileged local helper.

Owns:

- lease verification;
- local allowlist validation;
- typed privileged action execution.

The helper should be the most conservative crate. Keep the command surface tiny.

## Dependency stance

Start boring:

- `axum` for server HTTP;
- `tokio` for async runtime;
- `rustls` for TLS;
- `sqlx` or `rusqlite` for storage;
- `serde`/`serde_yaml` for config;
- `clap` for CLI;
- `tracing` for logs;
- `time` for timestamps;
- `ed25519-dalek` or equivalent for leases.

Avoid early dependencies for:

- full workflow engines;
- Kubernetes clients;
- browser automation;
- MCP server frameworks;
- embedded scripting languages.

Those can come later if the core proves useful.

## Target binaries

```text
runlane-server      # control plane
runlane-agent       # node agent
runlane-helper      # narrow privileged helper
runlanectl          # optional future CLI; can initially be a server subcommand
```

For v0.1, `runlane-server` and `runlane-agent` can expose CLI subcommands directly. Add `runlanectl` only when command UX becomes crowded.

## Test strategy

- Unit-test core state transitions.
- Fixture-test platform command parsers.
- Integration-test server/agent pull loop on localhost.
- Contract-test helper lease verification.
- Use fake platform backends in CI.
- Run real Linux backend in CI.
- Run FreeBSD/OpenBSD backend checks manually or via VM later.

## Release targets

Initial release artifacts:

- `x86_64-unknown-linux-musl` for Linux;
- `aarch64-unknown-linux-musl` if needed;
- FreeBSD x86_64 native binary;
- OpenBSD x86_64 native binary.

Do not promise fully static BSD binaries until verified. BSD native distribution is acceptable.
