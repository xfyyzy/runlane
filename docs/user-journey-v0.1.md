# Runlane v0.1 User Journey

This document defines the first end-to-end Runlane operator journey.

It exists to keep implementation work converged on a demonstrable product
slice instead of a set of correct but disconnected kernel objects.

## Journey Summary

The v0.1 journey is:

```text
Declare -> Observe -> Propose -> Approve -> Lease -> Execute -> Verify -> Remember
```

The first demo success path is:

1. Initialize the server runtime.
2. Initialize a fleet-intent repository.
3. Enroll Linux, FreeBSD, and OpenBSD agents.
4. Run native capability reports for all enrolled nodes.
5. Trigger the `service-unhealthy` system-layer runbook.
6. Collect platform-native evidence.
7. Generate a structured proposal.
8. Approve `service.restart`.
9. Issue a signed capability lease.
10. Execute the typed helper action.
11. Run impact-scoped verification.
12. Emit a cognitive receipt.

This is the first aha moment: an agent can diagnose and safely change a real
system service without becoming a trusted remote shell user.

## Ownership Boundaries

Git stores desired operational intent:

- inventory;
- roles;
- runbooks;
- policies;
- allowlists;
- reusable helper script declarations.

The server ledger stores runtime truth:

- node enrollment state;
- capability reports;
- incidents;
- evidence;
- proposals;
- approvals;
- lease grants and denials;
- helper execution results;
- verification outcomes;
- cognitive receipts.

Agents pull work from the server. Nodes do not require inbound ports.

Chat, alerting, and bot integrations are adapters. They can create incidents,
request approvals, and display receipts, but they do not own scheduling,
leases, helper authorization, or audit semantics.

## First-Run Journey

### 1. Server Initialization

Operator intent:

```bash
runlane-server init
runlane-server serve
```

Minimum behavior:

- create local server state;
- create or load a self-signed CA;
- expose enrollment, pull, result, and approval APIs;
- initialize an append-only event ledger;
- fail explicitly if required storage or key material is missing.

The current local durable state boundary stores append-only audit events under
`runlane-state/ledger/audit.yaml`. See [`server-state.md`](server-state.md).

### 2. Fleet Intent Initialization

Operator intent:

```bash
runlane fleet init ./fleet
runlane fleet validate ./fleet
runlane fleet sync ./fleet
```

Minimum repository shape:

```text
fleet/
├── inventory/
├── roles/
├── runbooks/
├── policies/
├── allowlists/
└── scripts/
```

The fleet repository declares desired intent only. It must not become the
place where high-churn evidence, command output, approvals, or runtime audit
events are stored.

### 3. Node Enrollment

Operator intent:

```bash
runlane-server enrollment create --node prod-web-01 --os linux
runlane-agent config init --config /etc/runlane-agent/agent.yaml ...
runlane-agent enroll --server https://runlane.example --token <token>
runlane-agent run --config /etc/runlane-agent/agent.yaml
```

Minimum behavior:

- enrollment binds a node identity to an OS family and server trust root;
- agent identity uses mTLS after enrollment;
- enrollment tokens are short-lived and auditable;
- failed enrollment leaves no half-trusted node identity.
- agent startup fails closed when local config, identity metadata, trust root,
  certificate, private key, spool directory, permissions, or platform family do
  not match the enrolled node.

The current CLI-safe local state boundary is documented in
[`agent-local-state.md`](agent-local-state.md). Until real enrollment transport
writes local identity state, `runlane-agent identity install` persists the same
metadata shape for development and tests without claiming to replace mTLS
enrollment.

### 4. Capability Baseline

After enrollment, each agent reports native capabilities.

Linux example:

```yaml
node_id: prod-web-01
os:
  family: linux
capabilities:
  - os.linux
  - service.systemd
  - logs.journald
  - process.procfs
  - socket.ss
  - storage.df
  - privilege.sudo-helper
unsupported:
  - service.freebsd-rc
  - service.openbsd-rcctl
```

FreeBSD example:

```yaml
node_id: freebsd-edge-01
os:
  family: freebsd
capabilities:
  - os.freebsd
  - service.freebsd-rc
  - logs.syslog-file
  - process.procstat
  - socket.sockstat
  - storage.df
  - privilege.sudo-helper
unsupported:
  - service.systemd
  - logs.journald
```

OpenBSD example:

```yaml
node_id: openbsd-edge-01
os:
  family: openbsd
capabilities:
  - os.openbsd
  - service.openbsd-rcctl
  - logs.syslog-file
  - process.ps
  - socket.fstat
  - storage.df
  - privilege.doas-helper
unsupported:
  - service.systemd
  - service.freebsd-rc
```

Unsupported capabilities fail closed. They are not silently downgraded to shell
commands.

## First-Incident Journey

The first incident uses `examples/runbooks/service-unhealthy.yaml`.

### 1. Declare

The operator or an adapter creates a system-layer incident:

```bash
runlane incident create \
  --runbook service-unhealthy \
  --node prod-web-01 \
  --param service=sshd
```

The incident records:

- runbook id and version;
- target node;
- operational layer;
- parameters;
- source adapter or operator identity;
- initial audit event.

### 2. Observe

The server schedules read-only collection tasks.

For `service-unhealthy`, the agent collects:

- service status;
- recent logs;
- disk snapshot;
- process snapshot.

The platform backend chooses native commands or APIs. The runbook model does
not assume systemd.

### 3. Propose

Collected evidence is untrusted input. It can support a proposal, but it cannot
be executed.

The proposal is structured data:

```yaml
proposal:
  action: service.restart
  layer: system
  target:
    node: prod-web-01
    service: sshd
  required_lease:
    resource: system:node/prod-web-01/service/sshd
    mode: exclusive
  impact:
    writes:
      - system:node/prod-web-01/service/sshd
    may_affect:
      - platform:on-node/prod-web-01
      - application:on-node/prod-web-01
    does_not_affect:
      - system:node/prod-web-01/package-db
      - system:node/prod-web-01/firewall
  verification:
    required:
      - service_active
    skipped_with_reason:
      - check: package_audit
        reason: service restart did not mutate package database
      - check: firewall_audit
        reason: service restart did not mutate firewall rules
```

### 4. Approve

The approval surface must show:

- action;
- layer;
- target resources;
- required lease mode;
- expected impact;
- verification plan;
- skipped checks and reasons;
- evidence summary;
- residual risk.

Approving a proposal is a runtime state transition. It is not a chat side
effect.

The current CLI-safe boundary exposes the deterministic demo approval through:

```bash
cargo run -p runlane -- approval list
cargo run -p runlane -- approval show approval-demo-1
cargo run -p runlane -- approval approve approval-demo-1
cargo run -p runlane -- approval reject approval-demo-1
```

Approval binds the stored proposal action. The approve command does not accept
a replacement action, target, impact set, or lease mode.

### 5. Lease

The server issues a short-lived signed capability lease scoped to:

- node identity;
- action kind;
- target service;
- run id;
- approval id;
- expiry;
- nonce;
- local helper allowlist entry.

The helper rejects expired, replayed, mismatched, or unsupported leases.

### 6. Execute

The agent calls the helper with a typed action request:

```yaml
action: service.restart
with:
  service: sshd
lease: <signed capability lease>
```

The helper must not accept arbitrary shell. Linux and FreeBSD use the
sudo-helper path. OpenBSD uses the doas-helper path.

### 7. Verify

The verifier selects checks from the declared impact boundary.

For `service.restart`, v0.1 requires:

- service active after action;
- helper result matches the requested typed action;
- skipped package and firewall checks have explicit reasons.

Tier 3 checks are not forbidden, but they must be selected deliberately by
policy or broad impact. They are not the default for a service restart.

### 8. Remember

The final cognitive receipt includes:

- run id;
- operator or adapter source;
- target node and operational layer;
- evidence used;
- proposal;
- approval;
- lease;
- action result;
- verification result;
- skipped checks;
- residual risk;
- manual takeover and rollback notes.

The receipt is stored in the server ledger, not committed back to the fleet
intent repository.

The current deterministic E2E path exercises the full sequence and emits the
receipt from audit events:

```bash
cargo run -p runlane -- demo service-unhealthy examples/fleet
cargo run -p runlane -- receipt show run-demo-service-unhealthy examples/fleet
```

## Minimal v0.1 CLI And API Surface

### CLI

Required operator-facing commands:

```text
runlane-server init
runlane-server serve
runlane-server enrollment create
runlane fleet init
runlane fleet validate
runlane fleet sync
runlane-agent config init
runlane-agent config validate
runlane-agent enroll
runlane-agent run
runlane incident create
runlane incident show
runlane approval list
runlane approval show
runlane approval approve
runlane approval reject
runlane demo service-unhealthy
runlane receipt show
```

The exact flag shape can evolve, but these commands define the minimum user
journey surface.

### APIs

Required server APIs:

- enrollment token creation;
- agent enrollment;
- agent task pull;
- agent result submission;
- incident creation;
- approval decision;
- receipt retrieval.

Adapters call these APIs. They do not receive special execution semantics.

## Demo Success Path

The v0.1 demo is successful when it can show:

1. Linux, FreeBSD, and OpenBSD agents enrolled with no inbound node ports.
2. Each node reports native capabilities and unsupported capabilities.
3. A `service-unhealthy` incident is created for one node.
4. The agent collects service, log, disk, and process evidence.
5. The server produces a structured `service.restart` proposal.
6. The approval view exposes lease, impact, verification, and skipped checks.
7. A signed lease allows exactly one typed helper action.
8. The helper executes `service.restart` through sudo or doas.
9. Verification runs only checks relevant to the declared impact.
10. The final receipt explains what happened and why.

## Explicit Non-Goals

v0.1 does not include:

- Web UI first;
- arbitrary remote shell;
- Kubernetes-first workflows;
- application deployment as the first product slice;
- chat-specific operations logic;
- broad root agent privileges;
- storing runtime evidence in Git;
- generic plugin or MCP marketplace behavior.

## Follow-Up Issue Map

This journey depends on the existing v0.1 implementation issues:

- core domain model: #1;
- resource-lease scheduler: #2;
- impact-scoped verification planner: #3;
- platform backend capability reports: #4;
- signed capability lease and helper contract: #5;
- audit event model and receipt schema: #6;
- agent pull-loop protocol: #7;
- service-unhealthy dogfood runbook: #8;
- fleet repo schema and overlay resolution: #9.

No additional implementation issue is required by this journey at this time.
If later implementation exposes a missing kernel object or CLI/API contract,
create or update a focused issue before adding an unbounded implementation
path.
