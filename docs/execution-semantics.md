# Runlane Execution Semantics

## Why execution semantics matter

Two practical failures motivate this design:

1. Agents often complete tasks reliably but run every possible check afterward. This is safe in theory but too slow for simple tasks.
2. Multiple tasks either run fully serially or collide because their dependencies and conflicts are not modeled.

Runlane must make both verification and concurrency explicit, and both must be layer-aware.

## Main objects

### OperationalLayer

Every resource, task, runbook, verification profile, and policy rule belongs to an operational layer:

- `system`
- `platform`
- `application`

Layer influences scheduling, approval, blast-radius analysis, and verification selection.

### Run

A `Run` is one execution of a runbook or manual task.

A run owns:

- goal;
- target layer;
- node or node group target;
- phases;
- task graph;
- evidence;
- proposals;
- approvals;
- resource leases;
- verification plan;
- audit events;
- cognitive receipt.

### Task

A `Task` is a schedulable unit inside a run.

Every task must declare:

- operational layer;
- required capabilities;
- resource reads;
- resource writes;
- expected impact;
- dependencies;
- verification requirements;
- timeout and budget;
- failure behavior.

### Resource

A `Resource` is anything tasks may contend over.

Examples:

```text
system:node/prod-web-01/package-db
system:node/prod-web-01/firewall
system:node/prod-web-01/reboot
system:node/prod-web-01/filesystem/var
platform:postgres/main
platform:redis/cache-01
platform:gateway/nginx-main
application:sports/api-gateway
application:daily-tech/worker
```

### ResourceLease

A `ResourceLease` grants a run temporary rights over a resource.

Lease modes:

| Mode | Meaning | Compatible with |
|---|---|---|
| `observe` | read-only evidence collection | other `observe`, sometimes `shared-mutate` |
| `intent` | planning/proposal without side effects | other non-mutating leases |
| `shared-mutate` | safe shared mutation class, rare | compatible resources only |
| `exclusive` | mutation requiring serialization | no other writer |
| `drain` | prepare for disruptive operation | blocks new work on affected dependency path |
| `reboot` | node-level disruptive operation | exclusive over the node |

Default rule: mutating operations are exclusive unless explicitly proven safe.

## Conflict model

Runlane should not rely on natural-language "be careful" instructions for concurrency.

A task can start only if:

1. all dependencies are satisfied;
2. required capabilities are available;
3. required resource leases are granted;
4. policy permits the operation;
5. the node is not in a conflicting drain/reboot state;
6. no lower-layer mutation invalidates the task's assumptions.

### Basic compatibility matrix

| Existing lease | New observe | New exclusive | New reboot |
|---|---:|---:|---:|
| observe | allow | usually allow with version marker | block |
| intent | allow | allow if no writer | block |
| exclusive | allow only if stale reads acceptable | block | block |
| drain | allow limited diagnostics | block | allow same run |
| reboot | block | block | block |

Evidence collected during concurrent mutation must be marked with a version or staleness warning.

## Layer-aware scheduling

Default dependency direction:

```text
Application -> Platform -> System
```

The scheduler should:

- allow unrelated application-layer tasks to run concurrently;
- serialize application tasks that mutate the same app/release/endpoint;
- serialize platform tasks that share a stateful instance or cluster;
- require exclusive/drain leases for system package, firewall, privilege, kernel, and reboot operations;
- block upper-layer mutation when a lower-layer drain/reboot lease affects its dependency path;
- audit why a task waited or why it was allowed to run concurrently.

## Verification must be impact-scoped

Runlane's verification model is not "always run everything".

It is:

```text
Layer + action -> impact scope -> verifier selection -> audit why this is enough
```

Every mutating task must declare an `ImpactSet`:

```yaml
impact:
  layer: application
  writes:
    - application:blog/service
  may_affect:
    - endpoint:https://blog.example.com/_health
  does_not_affect:
    - system:node/prod-web-01/package-db
    - system:node/prod-web-01/firewall
```

Every verifier must attach to one or more impacted resources:

```yaml
verify:
  required:
    - check: service_active
      resource: application:blog/service
    - check: http_health
      resource: endpoint:https://blog.example.com/_health
  skipped_with_reason:
    - check: package_audit
      reason: application restart did not modify package-db
    - check: full_disk_scan
      reason: no filesystem mutation occurred
```

## Verification tiers

### Tier 0: precondition checks

Cheap checks needed before action:

- target exists;
- required capability exists;
- lease valid;
- local allowlist permits target;
- syntax check if a config file will be reloaded;
- enough disk space if action writes files.

### Tier 1: direct impact checks

Checks directly tied to modified resources:

- service active after restart;
- process PID changed or stayed according to expected action;
- port listening;
- config syntax valid;
- file permissions match expected mode;
- helper result matches typed action.

### Tier 2: dependent checks

Checks for resources that depend on the changed resource:

- HTTP endpoint after web service restart;
- reverse proxy route after config reload;
- local socket consumer after daemon restart;
- scheduled job next-run after cron edit;
- application canary after platform mutation.

### Tier 3: broad/regression checks

Expensive checks that should run only when the impact is broad or policy requires them:

- full package audit;
- full filesystem scan;
- all service health checks on node;
- all runbooks dry-run;
- full security scan;
- fleet-wide drift scan.

Tier 3 is not forbidden. It is scheduled deliberately, not as the default after every small change.

## Examples

### Application service restart

Expected verification:

- app service active;
- app health endpoint ok;
- recent app logs have no startup errors.

Do not run by default:

- package audit;
- full disk scan;
- firewall audit;
- unrelated platform checks.

### Platform database config reload

Expected verification:

- config syntax;
- database accepts connections;
- replication or cluster state is healthy;
- dependent critical app canary if policy requires.

### System package upgrade

Expected verification is broader:

- package manager database lock;
- affected packages;
- services owning upgraded files;
- restart-required detection;
- service-specific checks;
- optional upper-layer canaries;
- optional node-level health.

This may justify Tier 3 checks.

## Runbook fields required for scheduling

A runbook step should support fields like:

```yaml
- id: restart_nginx
  layer: platform
  action: service.restart
  resources:
    reads:
      - platform:gateway/nginx-main/logs
    writes:
      - platform:gateway/nginx-main/service
    conflicts:
      - system:node/{{ node }}/reboot
      - system:node/{{ node }}/package-db
  requires:
    capabilities:
      any:
        - service.systemd
        - service.freebsd-rc
        - service.openbsd-rcctl
  verification:
    strategy: layer_impact_scoped
    required:
      - service_active: nginx
      - port_listening: 443
    max_duration: 30s
  approval: required
```

## Audit requirements

Every run must record:

- declared layer;
- declared impact scope;
- resource leases requested/granted/denied;
- selected verification checks;
- skipped checks and reasons;
- concurrency decisions when a task waits;
- approval decision;
- action result;
- final receipt.

The operator should be able to answer:

> Why did Runlane run these checks, and why did it not run the others?

and:

> Why did this task wait instead of running concurrently?
