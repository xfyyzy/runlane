# Coding Agent Brief: What to Build

This document is the handoff contract for coding agents implementing Runlane.

## Mission

Build Runlane: a self-hosted AI operations control plane for layered Unix-like infrastructure.

The goal is not to build a generic agent framework. The goal is to make real machine operations safer, faster, reusable, concurrent, and auditable.

## Non-negotiable requirements

1. Rust workspace targeting Rust stable 1.96.0 or newer (`rust-version = "1.96"`).
2. Single-file agent distribution where practical.
3. Pull-based agent model; no inbound node port required.
4. Server-agent mTLS with self-signed CA support.
5. Replay protection for task/action messages.
6. Logs and command output are untrusted evidence.
7. LLM output is structured proposal data, never executable command text.
8. Linux, FreeBSD, and OpenBSD are first-class v0.1 platforms.
9. Solaris/illumos must be architecturally addable later.
10. Privileged actions go through a narrow helper with signed capability leases and local allowlists.
11. Verification must be relevant to the changed resources and operational layer, not a blanket full-gate.
12. Concurrent tasks must use dependency and resource conflict control.
13. Dogfood starts at the OS/system layer.
14. The domain model must explicitly represent `system`, `platform`, and `application` layers even if v0.1 implements system-layer scenarios first.

## Documents to read before coding

Read these in order:

1. `AGENTS.md`
2. `README.md`
3. `docs/project-charter.md`
4. `docs/product-definition.md`
5. `docs/operational-layer-model.md`
6. `docs/architecture.md`
7. `docs/execution-semantics.md`
8. `docs/platform-model.md`
9. `docs/workspace-structure.md`
10. `docs/milestones/v0.1.md`
11. `docs/adr/0001-cross-platform-native-agent.md`

Do not implement from memory or from generic agent-platform assumptions.

## Implementation philosophy

Runlane's kernel objects matter more than UI or integrations.

Prioritize:

- `OperationalLayer`;
- `Run` state machine;
- `Task` DAG;
- `Resource` and `ResourceLease` model;
- `Capability` model;
- `ImpactSet` and scoped verification;
- platform backend trait;
- helper lease verification;
- audit ledger.

Deprioritize:

- web dashboards;
- generic chat UX;
- application deployment features;
- Kubernetes integrations;
- MCP marketplace features;
- broad plugin system;
- arbitrary remote shell.

## First implementation slice

A good first implementation slice is:

1. core domain types for layers, nodes, capabilities, resources, leases, tasks, and run states;
2. in-memory scheduler that respects dependencies and resource leases;
3. verification planner that selects checks based on `OperationalLayer + ImpactSet`;
4. fake platform backend for tests;
5. Linux/FreeBSD/OpenBSD backend skeletons reporting capabilities;
6. append-only audit event model.

Do not start with Telegram, Web UI, or LLM integration.

## Done means

For any feature, "done" means:

- behavior is represented in core domain types;
- there are tests for state transitions, scheduling behavior, or verification selection;
- platform-specific assumptions are isolated;
- layer-specific behavior is explicit;
- audit events record the important decision;
- failure modes fail closed;
- docs/examples are updated if semantics changed.

## Common wrong turns

### Wrong: global full verification after every task

This is slow and will make Runlane unpleasant to use.

Right: action declares layer and impact; verifier selects relevant checks; audit records skipped checks and reasons.

### Wrong: no concurrency until everything is safe

This wastes the control plane.

Right: model resources and leases so safe parallelism is possible.

### Wrong: Linux first, BSD later

This will bake systemd assumptions into the model.

Right: define platform capabilities first and implement native backends.

### Wrong: collapse system/platform/application into one resource kind

This loses the operator's real mental model.

Right: `kind` describes technical shape; `layer` describes operational meaning.

### Wrong: Telegram bot owns operations logic

This repeats the current pain.

Right: bot calls generic approval/incident APIs.

### Wrong: helper as root shell wrapper

This is too dangerous.

Right: helper accepts typed action requests and signed leases only.

## First dogfood runbook

Implement toward this shape:

```yaml
name: service-unhealthy
layer: system
collect:
  - service_status
  - recent_logs
  - disk_snapshot
  - process_snapshot
proposal:
  allowed_actions:
    - service.restart
verification:
  strategy: layer_impact_scoped
approval:
  required_for:
    - service.restart
concurrency:
  writes:
    - system:node/{{ node }}/service/{{ service }}
  conflicts:
    - system:node/{{ node }}/reboot
    - system:node/{{ node }}/package-db
```

This runbook should work semantically across Linux, FreeBSD, and OpenBSD, even though each backend uses native commands.
