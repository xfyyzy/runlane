# Runlane

> A self-hosted AI operations control plane for layered Unix-like infrastructure.

Runlane models **system**, **platform**, and **application** operations as capability-scoped, resource-leased, impact-verified, auditable runs across heterogeneous fleets.

v0.1 dogfoods the **system layer** first across Linux, FreeBSD, and OpenBSD.

## Why Runlane exists

Many operators already use AI for server operations, but the workflow is often scattered:

- one repository per machine or per node batch;
- runbooks and remote state mixed together in ad-hoc Git repos;
- tasks manually pushed by the operator;
- temporary `passwordless sudo` / `doas` access is hard to grant and revoke safely;
- Telegram/Discord bot integrations work for one node group but are hard to promote fleet-wide;
- agents often run every possible check after a simple task, making safe operations slow;
- concurrent tasks are either serialized unnecessarily or allowed to collide;
- Linux, FreeBSD, and OpenBSD all need first-class support instead of being treated as edge cases.

Runlane keeps the useful part — versioned operational intent — but moves execution into a real control plane.

## Thesis

Do not treat AI agents as trusted shell users.

Treat them as uncertain processes that may request syscalls inside a capability-scoped, auditable kernel:

- Git stores desired operational intent: inventory, roles, runbooks, policies, allowlists.
- The server owns scheduling, approvals, audit, runtime state, and policy evaluation.
- Agents pull tasks; no inbound node port is required.
- Privileged operations go through narrow local helpers and short-lived signed capability leases.
- Logs and command output are untrusted evidence, not instructions.
- Verification is selected by operational layer and impact scope, not by blanket full-gate habit.
- Concurrency is controlled by dependencies and resource leases, not by natural-language caution.

## Operational layers

| Layer | Scope | v0.1 stance |
|---|---|---|
| `system` | OS, kernel, modules, system libraries, packages, users, privilege, firewall, filesystems, service manager | first dogfood target |
| `platform` | databases, middleware, gateways, queues, caches, observability stack | modeled now, packages later |
| `application` | business services, bots, workers, app config, releases | modeled now, user-space runbooks later |

`kind` describes the technical shape. `layer` describes operational meaning.

A `service` can be system, platform, or application depending on what depends on it.

## Product shape

```text
operator / bot / alertmanager
          |
          v
+----------------------+       GitOps sync        +----------------------+
| Runlane server       | <----------------------> | fleet repo           |
| - incidents          |                          | - inventory/         |
| - runbook scheduler  |                          | - roles/             |
| - resource leases    |                          | - runbooks/          |
| - verification plan  |                          | - policies/          |
| - approvals          |                          | - allowlists/        |
| - audit ledger       |                          +----------------------+
+----------+-----------+
           ^  pull over mTLS
           |
+----------+-----------+       optional sudo/doas       +----------------------+
| runlane-agent        | -----------------------------> | runlane-helper       |
| - local collectors   |    signed capability lease     | - root-only actions  |
| - platform backend   |                                | - local policy check |
| - local spool        |                                +----------------------+
+----------------------+
```

## First-class cross-platform support

Runlane is designed around platform capabilities, not Linux assumptions.

Linux, FreeBSD, and OpenBSD are first-class in v0.1. Solaris and illumos-style systems are not first-release targets, but the architecture must allow future backends without redesigning the runbook model.

| Driver family | Linux | FreeBSD | OpenBSD | Solaris/illumos later |
|---|---|---|---|---|
| ServiceManager | systemd, SysV later | rc.d/service | rcctl | SMF |
| LogProvider | journald/syslog | syslog files | syslog files | SMF logs/syslog |
| ProcessProvider | procfs/ps | procstat/ps | ps | proc tools |
| SocketProvider | ss/netstat | sockstat/netstat | fstat/netstat | netstat/pfiles |
| PrivilegeProvider | sudo-helper | sudo-helper | doas-helper | pfexec/RBAC/helper |

## What Git is responsible for

Runlane should not throw away existing Git-based operations. It should formalize them.

Git is the source of truth for **intent**:

```text
fleet/
├── inventory/
├── roles/
├── runbooks/
├── policies/
├── allowlists/
└── scripts/
```

The server is the source of truth for **runtime evidence**:

- heartbeats;
- observed facts;
- incident state;
- proposals;
- approvals;
- command output;
- action results;
- audit trail.

This split avoids one repo per node while preserving versioned operational architecture.

## Security model

1. **Pull, not push** — agents poll the server over mTLS. Nodes do not need inbound ports.
2. **Scoped capability leases** — privileged actions require signed, short-lived, non-replayable leases.
3. **Narrow helper** — `sudo`/`doas` can invoke only `runlane-helper`, not arbitrary shell.
4. **Logs are untrusted** — collected text is evidence, never instructions.
5. **Human approval is a runtime interrupt** — approval is an auditable state transition.

## Non-goals for v0.1

- Not a general-purpose agent framework.
- Not a Kubernetes-only operator.
- Not a replacement for Prometheus/Grafana/PagerDuty.
- Not an unrestricted remote shell.
- Not a dashboard for coding agents.
- Not an MCP marketplace.
- Not an application deployment platform first.

## Starter workspace

```text
crates/
├── runlane-core/      # shared domain types and state-machine vocabulary
├── runlane-agent/     # node-side pull worker, platform adapters later
├── runlane-server/    # control-plane API/scheduler/audit later
└── runlane-helper/    # narrow privileged helper, invoked by sudo/doas later
```

## Important docs

Read in this order before implementing:

1. [`AGENTS.md`](AGENTS.md)
2. [`docs/project-charter.md`](docs/project-charter.md)
3. [`docs/product-definition.md`](docs/product-definition.md)
4. [`docs/operational-layer-model.md`](docs/operational-layer-model.md)
5. [`docs/architecture.md`](docs/architecture.md)
6. [`docs/execution-semantics.md`](docs/execution-semantics.md)
7. [`docs/platform-model.md`](docs/platform-model.md)
8. [`docs/helper-contract.md`](docs/helper-contract.md)
9. [`docs/agent-protocol.md`](docs/agent-protocol.md)
10. [`docs/coding-agent-brief.md`](docs/coding-agent-brief.md)
11. [`docs/dogfood-system-scenarios.md`](docs/dogfood-system-scenarios.md)
12. [`docs/user-journey-v0.1.md`](docs/user-journey-v0.1.md)
13. [`docs/milestones/v0.1.md`](docs/milestones/v0.1.md)
14. [`docs/adr/0001-cross-platform-native-agent.md`](docs/adr/0001-cross-platform-native-agent.md)

## Development

Runlane currently requires **Rust stable 1.96.0 or newer**. The Cargo MSRV field is set to `rust-version = "1.96"`; `rust-toolchain.toml` uses the floating `stable` channel for local development.

```bash
rustc --version
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
```

Cross-platform validation must keep build and runtime baselines aligned. Linux
and FreeBSD release artifacts may be cross-built when the target sysroot and
test VM use the same current stable OS release. OpenBSD is the exception:
because the stable Rust toolchain does not currently provide an installable
`x86_64-unknown-openbsd` standard library through rustup, OpenBSD validation is
performed inside a native OpenBSD VM with a Rust toolchain that satisfies the
project MSRV. Do not treat nightly `-Zbuild-std`, an older OpenBSD Rust package,
or a Linux-hosted OpenBSD cross build as the default project path.

## License

BSD-2-Clause — see [`LICENSE`](LICENSE).
