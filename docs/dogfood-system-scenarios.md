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

Resources:

- `system:node/<node>/service/<service>`;
- `system:node/<node>/logs/<service>`;
- `system:node/<node>/filesystem`;
- `system:node/<node>/processes`.

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

Resources:

- `filesystem:<mount>`;
- allowlisted cleanup paths;
- logs.

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

- package state consistent;
- affected services restarted;
- node reboot only after drain/exclusive lease.

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
