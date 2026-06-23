# System-layer Dogfood Scenarios

Runlane should dogfood system operations first.

## Scenario selection rules

Choose scenarios that are:

- common across Linux, FreeBSD, and OpenBSD;
- dangerous enough to need audit/approval;
- small enough for scoped verification;
- not dependent on application-specific semantics;
- useful on the operator's own machines.

## Recommended v0.1 scenarios

### 1. Service unhealthy

Goal: diagnose and optionally restart/reload a daemon.

This is the first v0.1 executable dogfood target. The core planner turns
`examples/runbooks/service-unhealthy.yaml` and a node capability report into a
typed run plan for Linux, FreeBSD, and OpenBSD.

The planner selects the native service manager for the reported OS:

- Linux: `service.systemd`;
- FreeBSD: `service.freebsd-rc`;
- OpenBSD: `service.openbsd-rcctl`.

It fails closed when the required native service manager, log reader, process
snapshot, storage snapshot, or signed helper capability is missing. OpenBSD is
not modeled as a systemd target.

The agent backend now exposes CI-safe collector specs and parser fixtures for:

- service status;
- recent logs;
- disk snapshot;
- process snapshot;
- listening sockets.

Command construction is owned by the platform backend. Runbooks and analyzer
output select typed capabilities and resources; they do not supply shell
commands.

The agent can also execute the same collector specs against the local native
backend:

```bash
cargo run -p runlane-agent -- collect-smoke --service sshd
```

This smoke exercises service status, recent logs, disk, process, and socket
collectors. Linux, FreeBSD, and OpenBSD use different native commands, but the
runbook-facing contract remains typed capabilities and resources.

The deterministic analyzer consumes evidence envelopes and emits typed proposal
data: hypothesis, evidence references, proposed actions, confidence, and
approval requirements. Prompt-injection-like log text remains untrusted
evidence and cannot introduce helper actions or shell commands.

Resources:

- `system:node/<node>/service/<service>`;
- `system:node/<node>/logs/<service>`;
- `system:node/<node>/filesystem`;
- `system:node/<node>/processes`;
- `system:node/<node>/sockets`.

The CI-safe demo runs the service-unhealthy journey end to end from the fleet
fixture and emits the ledger-derived receipt:

```bash
cargo run -p runlane -- demo service-unhealthy examples/fleet
cargo run -p runlane -- receipt show run-demo-service-unhealthy examples/fleet
```

The Linux real-host dogfood smoke bridges from that fixture path to native host
evidence without restarting a production service:

```bash
cargo xtask smoke linux-service-unhealthy-dogfood --confirm-host-mutation
```

The smoke runner invokes `scripts/smoke/linux-service-unhealthy-dogfood.sh`.
The script creates only the fixed controlled systemd unit
`runlane-demo-unhealthy.service`, intentionally starts it into a failed state,
runs the native Linux collectors, validates the typed helper request in dry-run
mode, persists the audit ledger to a temporary local state directory, renders
the receipt back through `runlane receipt show`, and removes the demo unit on
exit. It must not be repointed at an arbitrary production service.

The underlying commands are:

```bash
rm -rf /tmp/runlane-service-dogfood-state
cargo run -p runlane-agent -- collect-smoke --service runlane-demo-unhealthy.service
cargo run -p runlane-agent -- dogfood-service-unhealthy \
  --service runlane-demo-unhealthy.service \
  --state-dir /tmp/runlane-service-dogfood-state \
  --node-id prod-web-01
cargo run -p runlane -- receipt show \
  run-real-host-service-unhealthy \
  /tmp/runlane-service-dogfood-state
```

`dogfood-service-unhealthy` currently requires Linux/systemd because this
specific smoke is a controlled real-host service failure. FreeBSD and OpenBSD
remain first-class targets through their native VM validation scripts and native
collector smoke paths; this Linux smoke is not a cross-platform downgrade.

Restart lease:

- `exclusive` on `system:node/<node>/service/<service>`;
- approval required before restart/reload;
- typed helper action only, not raw shell text.

Verification:

- service active;
- helper result matches request;
- package audit skipped with reason because package-db is not mutated;
- firewall audit skipped with reason because firewall rules are not mutated;
- dependent platform/application checks remain possible through the impact set.

### 2. Disk pressure

Goal: identify disk pressure and propose safe cleanup.

This is the second executable system-layer dogfood target. The demo runbook is
`examples/fleet/runbooks/disk-pressure.yaml`.

Resources:

- `filesystem:<mount>`;
- allowlisted cleanup paths;
- logs.

The analyzer may only propose `file.remove_from_allowlist` for a cleanup path
declared in the local helper allowlist. The helper action is typed data; it is
not `rm`, a shell command, or free-form cleanup text.

The CI-safe demo runs the disk-pressure journey end to end from the fleet
fixture and emits the ledger-derived receipt:

```bash
cargo run -p runlane -- demo disk-pressure examples/fleet
cargo run -p runlane -- receipt show run-demo-disk-pressure examples/fleet
```

Verification:

- free space improved;
- cleaned paths match allowlist;
- no unrelated deletion.

### 3. Failed scheduled job

Goal: diagnose cron/periodic/systemd timer failure.

Resources:

- `scheduled-job:<id>`;
- job logs;
- output artifact path.

Verification:

- next run is scheduled;
- manual dry-run if allowlisted;
- last error is cleared or explained.

### 4. Package update requires service restart

Goal: detect updated package with affected daemon.

Resources:

- `package-db:node`;
- affected `service:*`;
- `reboot:node` if kernel update.

Verification:

- package state matches declared policy;
- package database is consistent;
- affected services are identified and checked;
- reboot-required state is detected when the package impact may affect reboot;
- node reboot happens only after a drain/reboot lease and verifies node health.

### 5. Firewall rule reload

Goal: validate and apply firewall rule changes.

Resources:

- `firewall:node`;
- affected `port:*`;
- config file.

Verification:

- syntax check before reload;
- active rules inspected after reload;
- affected ports verified.

### 6. Temporary privilege lease cleanup

Goal: revoke temporary sudo/doas or helper access after task completion.

Resources:

- `privilege-rule:<user>`;
- helper allowlist;
- user/group database.

Verification:

- rule absent or expired;
- helper denies stale lease;
- audit receipt generated.

## Scenarios to delay

Delay these until the system layer is solid:

- full application deployment;
- database migrations;
- Kubernetes controllers;
- business workflow automation;
- multi-tenant SaaS UI;
- arbitrary script marketplace;
- generalized MCP integration.
