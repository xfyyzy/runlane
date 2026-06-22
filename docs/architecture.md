# Runlane Architecture

## Core tension

Runlane must preserve the best property of the current workflow — Git-versioned operational intent and reusable runbooks — while eliminating the parts that do not scale:

- repo-per-machine sprawl;
- manually initiated tasks;
- broad temporary `passwordless sudo` / `doas`;
- bot logic coupled to a specific node group;
- Linux-only assumptions;
- blanket full-gate verification after simple tasks;
- unsafe or under-modeled parallel operations.

The architecture answer is a layered control plane with explicit ownership.

| Concern | Owner | Reason |
|---|---|---|
| desired inventory, roles, runbooks, policies, allowlists | Git fleet repo | reviewable, branchable, reusable |
| live node facts, heartbeats, incidents, approvals, command output | server event ledger | high-churn runtime truth should not require Git commits |
| local OS interaction | agent platform backend | OS-specific behavior stays near the node |
| privileged execution | helper + local policy + signed lease | no broad AI-controlled sudo/doas |
| safe concurrency | scheduler + resource leases | avoids all-serial and unsafe parallel work |
| efficient safety checks | verification planner | checks are tied to layer and impact |

## Operational layers

Runlane's domain model is layer-aware:

```text
system      -> OS/kernel/packages/users/firewall/filesystems/service manager
platform    -> databases/middleware/gateways/queues/caches/observability
application -> business services/bots/workers/app config/releases
```

Default dependency direction:

```text
Application -> Platform -> System
```

v0.1 implements system-layer dogfood first. Platform and application layers are modeled now so that future packages do not require schema redesign.

See `docs/operational-layer-model.md` for the normative model.

## Agent OS lens

Runlane is an operations-specific Agent OS kernel, not a generic agent framework.

| OS concept | Runlane concept |
|---|---|
| process | runbook run / incident run |
| syscall | collector/action invocation |
| device driver | Linux/FreeBSD/OpenBSD backend |
| file descriptor | capability lease |
| scheduler | server task scheduler |
| lock manager | resource lease manager |
| signal/interrupt | approve/reject/cancel/escalate |
| audit log | append-only event ledger |
| package manager | runbook/role registry in Git |
| kernel/user mode | policy/server/helper vs LLM proposal text |

The LLM is user space. The server and helper enforce kernel semantics.

## Main components

### 1. Server

Responsibilities:

- node enrollment and certificate lifecycle;
- GitOps sync from fleet repos;
- inventory, role, and operational-layer resolution;
- incident creation from alerts, bots, CLI, or API;
- runbook scheduling and state transitions;
- resource lease arbitration;
- impact-scoped verification planning;
- policy evaluation;
- approval handling;
- LLM proposal generation with structured output;
- append-only audit ledger;
- task queue for agent pull;
- artifact storage for evidence, logs, reports.

Initial implementation can be one Rust binary with internal modules:

```text
runlane-server
├── api
├── audit
├── authn
├── enrollment
├── gitops
├── inventory
├── leases
├── llm
├── policy
├── scheduler
├── storage
└── verification
```

### 2. Agent

Responsibilities:

- enroll with server;
- maintain mTLS identity;
- poll for tasks;
- detect local platform capabilities;
- collect facts and logs;
- execute unprivileged actions;
- call privileged helper only when a valid capability lease is present;
- locally spool results when server is temporarily unavailable;
- never trust task text as shell code.

The agent should be a single-file distribution per target platform where possible.

### 3. Helper

The helper is a narrow privileged executable, not a shell gateway.

Responsibilities:

- verify signed capability leases;
- reject replayed/expired leases;
- verify local allowlist;
- execute a small set of typed privileged actions;
- emit structured results;
- refuse arbitrary shell strings.

v0.1 helper action candidates:

- `service.restart { name }`
- `service.reload { name }`
- `file.remove_from_allowlist { path }`
- `script.run_allowlisted { id, args }`

### 4. Fleet repo

The fleet repo is a portable operational source of intent:

```text
fleet/
├── inventory/
├── roles/
├── runbooks/
├── policies/
├── allowlists/
└── scripts/
```

The server syncs it and resolves overlays:

1. global defaults;
2. OS defaults;
3. operational layer defaults;
4. role defaults;
5. environment defaults;
6. node override.

This replaces repo-per-node without losing role reuse.

## Runtime state machine

A minimal incident run should follow this lifecycle:

```text
created
  -> planned
  -> collecting_evidence
  -> evidence_collected
  -> proposal_generated
  -> waiting_for_approval
  -> approved | rejected | cancelled
  -> executing
  -> verifying
  -> resolved | failed | escalated
  -> reviewed
```

Every transition writes an audit event.

## Data model sketch

First-class objects:

- `OperationalLayer`
- `Node`
- `NodeGroup`
- `Role`
- `PlatformCapability`
- `Resource`
- `ResourceLease`
- `ImpactSet`
- `VerificationPlan`
- `Runbook`
- `Policy`
- `Incident`
- `Run`
- `Task`
- `Evidence`
- `Proposal`
- `Approval`
- `CapabilityLease`
- `ActionExecution`
- `AuditEvent`

## Cross-platform platform trait

Use a trait-first design:

```rust
trait PlatformBackend {
    fn os(&self) -> OperatingSystem;
    fn detect_capabilities(&self) -> Vec<PlatformCapability>;
    fn collect_service_status(&self, service: &str) -> Result<ServiceStatus>;
    fn collect_logs(&self, query: LogQuery) -> Result<Evidence>;
    fn collect_process_snapshot(&self) -> Result<Evidence>;
    fn collect_disk_snapshot(&self) -> Result<Evidence>;
}
```

Backends must expose capability availability explicitly. A runbook step can then say:

```yaml
requires:
  any:
    - service.systemd
    - service.freebsd-rc
    - service.openbsd-rcctl
```

No step should silently assume systemd.

## Privilege model

### Current pain

The current ad-hoc model requires distributing temporary passwordless sudo/doas identities. This creates two hard problems:

- broad power is granted to a runtime that may be driven by model output;
- revocation is manual and easy to forget.

### Proposed model

Install a permanent but narrow helper rule once:

Linux/FreeBSD sudoers concept:

```text
runlane ALL=(root) NOPASSWD: /usr/local/libexec/runlane-helper
```

OpenBSD doas concept:

```text
permit nopass runlane as root cmd /usr/local/libexec/runlane-helper
```

This does **not** grant arbitrary root shell. It only allows invoking the helper, which still requires:

1. a signed server lease;
2. an unexpired nonce;
3. matching node identity;
4. matching local allowlist;
5. typed arguments that pass validation.

This turns passwordless sudo/doas from a broad delegation into a small local kernel syscall surface.

## LLM boundary

The LLM can produce:

- diagnosis;
- ranked hypotheses;
- evidence citations;
- proposed next collectors;
- proposed actions from an enum.

The LLM cannot produce:

- raw shell to execute;
- policy decisions;
- approval decisions;
- local helper bypasses;
- untyped mutations.

All LLM output must pass schema validation and policy evaluation.

## Notification channels

Telegram/Discord/Feishu should be adapters, not the control plane.

A chat approval should call the same approval API as the Web UI or CLI. This makes bot functionality portable across nodes and roles.

The v0.1 Telegram approval adapter is intentionally narrow:

- map a Telegram `chat_id` + `user_id` to a Runlane audit actor;
- list and show pending approvals;
- approve or reject by calling the same approval store methods used by CLI;
- fail closed for unknown Telegram identities;
- reject non-approval commands instead of interpreting chat text as operations.

It must not schedule tasks, execute helper actions, parse runbooks, own policy
logic, or carry node/runbook business behavior. Live Telegram credentials are
outside CI; adapter tests use deterministic command and identity fixtures.

## Storage choice

v0.1 can use SQLite for single-operator deployment:

- easy self-hosting;
- append-only event table;
- JSON evidence blobs;
- later migration path to Postgres.

Do not use Git as high-churn runtime event storage. Export summaries to Git if desired.

## Key design risks

| Risk | Mitigation |
|---|---|
| turns into generic agent framework | keep incident/runbook/system-ops domain narrow |
| Linux assumptions leak in | platform capability matrix in tests and docs |
| helper becomes arbitrary shell | typed actions only; no shell strings |
| model prompt injection through logs | evidence envelopes; schema-only proposal output |
| GitOps becomes too complex | start with read-only sync and explicit reload |
| approvals become chat-specific | approval API first; bots are adapters |
| verification becomes slow full-gate | layer + impact-scoped verification planner |
| parallel tasks collide | resource lease scheduler |
| layers collapse into service names | require `OperationalLayer` on resources/tasks/runbooks |
