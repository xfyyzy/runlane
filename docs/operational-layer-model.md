# Operational Layer Model

Runlane models operations through three layers. This is a product invariant, not only documentation vocabulary.

## The three layers

| Layer | Chinese | Typical change frequency | Examples | Default risk shape |
|---|---|---:|---|---|
| `system` | 系统层 | low | OS, kernel, boot config, kernel modules, system libraries, packages, users, privilege rules, firewall, routes, filesystems, mounts, ZFS datasets, service manager | high blast radius, slow recovery, may require reboot, affects all upper layers |
| `platform` | 平台层 | medium | PostgreSQL, MySQL, Redis, Nginx/Caddy as shared gateway, MQ, object storage, observability stack, service discovery | stateful, dependency-heavy, cluster consistency and data safety matter |
| `application` | 应用层 | high | business services, bots, workers, app configs, release artifacts, cron jobs owned by an app | frequent change, business-specific health, usually faster rollback |

The layer is about operational semantics, not the technical kind. A `service` can be system, platform, or application depending on why it exists and who depends on it.

Examples:

```yaml
# Nginx as shared ingress platform
resource:
  id: platform:gateway/nginx-main
  layer: platform
  kind: service

# Nginx as a private app sidecar or embedded service
resource:
  id: application:blog/nginx
  layer: application
  kind: service

# The local service manager itself
resource:
  id: system:node/prod-web-01/service-manager
  layer: system
  kind: service-manager
```

## Core design rule

`kind` describes the technical shape. `layer` describes operational meaning.

```text
Resource = Layer + Kind + Identity + Scope + Dependencies
```

Do not infer layer only from kind.

## Layer dependency direction

The default dependency direction is:

```text
Application -> Platform -> System
```

Meaning:

- application resources depend on platform and system resources;
- platform resources depend on system resources;
- system resources may affect everything above them;
- upper-layer changes should not implicitly mutate lower-layer resources.

This direction drives scheduling, approval, verification, and blast-radius estimation.

## Why this matters

### 1. Verification efficiency

Runlane must not run every check after every action.

Verification is selected from:

```text
Layer + ImpactSet + DependencyGraph + Policy -> VerificationPlan
```

A restart of an application service should not trigger a full package audit. A system package upgrade may need service linkage checks, platform health checks, and application canaries.

### 2. Concurrency control

Layer-aware resources make safe parallelism possible:

- unrelated application tasks can often run concurrently;
- platform tasks must respect shared state and cluster topology;
- system tasks often require node-level exclusive/drain/reboot leases.

### 3. Approval policy

Default approval posture differs by layer:

| Layer | Default posture |
|---|---|
| system | conservative; mutations often require approval |
| platform | conservative when stateful or cluster-wide; conditional for reload-only actions |
| application | allow automation for low-risk restarts/health checks; require approval for release/rollback/destructive data actions |

### 4. Cognitive receipts

The final run receipt must say not only what changed, but which layer changed and which upper layers may have been affected.

## Layer-aware resource examples

```yaml
resources:
  - id: system:node/prod-web-01/package-db
    layer: system
    kind: package-db
    node: prod-web-01

  - id: system:node/prod-web-01/firewall
    layer: system
    kind: firewall
    node: prod-web-01

  - id: platform:postgres/main
    layer: platform
    kind: database
    nodes:
      - db-01
      - db-02
    depends_on:
      - system:node/db-01/filesystem/var-lib-postgresql
      - system:node/db-02/filesystem/var-lib-postgresql

  - id: application:sports/api-gateway
    layer: application
    kind: service
    depends_on:
      - platform:postgres/main
      - platform:redis/cache
      - system:node/prod-app-01/service-manager
```

## Layer-aware runbook examples

### System layer: package upgrade

```yaml
name: system-package-upgrade
layer: system
resources:
  writes:
    - system:node/{{ node }}/package-db
  may_affect:
    - platform:on-node/{{ node }}
    - application:on-node/{{ node }}
leases:
  - resource: system:node/{{ node }}/package-db
    mode: exclusive
verification:
  strategy: layer_impact_scoped
  required:
    - package_db_consistent
    - changed_files_classified
    - affected_services_identified
    - restart_required_detected
  conditional:
    - service_health_for_affected_services
    - application_canary_for_critical_apps
approval: required
```

### Platform layer: database config reload

```yaml
name: postgres-config-reload
layer: platform
resources:
  writes:
    - platform:postgres/{{ instance }}/config
  may_affect:
    - application:depends-on/postgres/{{ instance }}
leases:
  - resource: platform:postgres/{{ instance }}
    mode: exclusive
verification:
  required:
    - config_syntax_valid
    - postgres_accepts_connections
    - replication_state_ok
    - dependent_app_canary_if_critical
approval: required
```

### Application layer: app service restart

```yaml
name: app-service-restart
layer: application
resources:
  writes:
    - application:{{ app }}/service
  reads:
    - application:{{ app }}/logs
verification:
  required:
    - app_service_active
    - app_health_endpoint_ok
    - recent_logs_no_startup_error
  skipped_with_reason:
    - check: package_audit
      reason: application restart did not mutate system package database
approval: conditional
```

## Scheduler implications

Each task declares `layer`, `reads`, `writes`, and `may_affect`.

The scheduler should:

1. block upper-layer mutation when a lower-layer `drain` or `reboot` lease is active on the same dependency path;
2. allow unrelated application-layer tasks to run concurrently;
3. serialize platform-layer tasks that share stateful resources;
4. require exclusive or drain leases for system-layer package/firewall/kernel/reboot operations;
5. audit why a task waited or ran concurrently.

## Verification implications

A verification planner must never choose checks only from action name. It must include layer and dependency context.

```text
service.restart + layer=application -> app health + app logs
service.restart + layer=platform    -> platform health + dependent app canary if critical
service.restart + layer=system      -> service manager health + upper-layer impact scan if shared service
```

## Policy implications

Policy should support layer selectors:

```yaml
approval:
  system:
    package_upgrade: required
    firewall_reload: required
    reboot: required
    read_only_diagnostics: auto
  platform:
    stateful_restart: required
    config_reload: conditional
  application:
    restart: auto_if_single_instance
    rollback: required
```

## v0.1 scope

v0.1 implements system-layer dogfood first, but the core domain model must already include `OperationalLayer` so platform/application packages can be added later without schema redesign.
