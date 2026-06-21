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

Resources:

- `service:<name>`;
- `logs:<name>`;
- `port:<port>`;
- maybe `endpoint:<url>`.

Verification:

- service active;
- expected port listening;
- optional endpoint health.

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
