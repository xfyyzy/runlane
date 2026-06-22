# Rust Workspace Structure

## Toolchain

Runlane currently uses the latest stable Rust toolchain as the minimum supported Rust version.

- Current MSRV: Rust stable 1.96.0 (`rust-version = "1.96"`).
- Local toolchain file: `rust-toolchain.toml` with `channel = "stable"`.

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
- Current CI runs Ubuntu Rust `fmt`, `check`, and `test`; do not claim broader
  CI coverage without adding a workflow that executes it.
- Use fake platform backends and parser fixtures in CI.
- Run Linux native collector smoke, FreeBSD backend checks, and OpenBSD backend
  checks locally or via VM until dedicated CI workflows exist.
- Treat build/runtime version mismatch as an environment failure, not an
  application behavior. Do not accept a cross-built artifact as validated for a
  different OS release unless that compatibility target is explicitly recorded.

## Release targets

Initial release artifacts:

- `x86_64-unknown-linux-musl` for Linux;
- `aarch64-unknown-linux-musl` if needed;
- FreeBSD x86_64 binary built and tested against the same current stable
  FreeBSD release;
- OpenBSD x86_64 binary built and tested in a native OpenBSD environment.

Do not promise fully static BSD binaries until verified for the specific
release target. BSD native distribution is acceptable.

### OpenBSD validation exception

OpenBSD remains a first-class v0.1 platform, but it is not treated as a
Linux-hosted cross-compilation target by default.

The reason is toolchain-specific: stable Rust can list
`x86_64-unknown-openbsd`, but rustup does not currently provide an installable
standard library for that target in the same way it does for Linux and FreeBSD.
Using nightly `-Zbuild-std` would weaken the project's stable-toolchain rule,
and using an older OpenBSD Rust package would violate the workspace MSRV.

Therefore OpenBSD work must be validated in a native OpenBSD VM with a Rust
toolchain that satisfies `rust-version = "1.96"`. If the VM needs an
environment-specific exception to obtain that Rust version, record it as an
environment fact and do not present it as the general project path for
contributors.
