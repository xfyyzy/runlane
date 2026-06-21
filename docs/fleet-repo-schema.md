# Fleet Repository Schema

The fleet repository stores desired operational intent. It does not store
runtime evidence, approvals, command output, helper results, audit events, or
cognitive receipts.

Runtime truth belongs in the Runlane server ledger.

## Layout

```text
fleet/
├── inventory/
├── roles/
├── runbooks/
├── policies/
├── allowlists/
├── scripts/
└── overlays/
    ├── global/
    ├── os/
    ├── layer/
    ├── role/
    ├── environment/
    └── node/
```

## Overlay Order

Overlay resolution is deterministic:

```text
global -> OS -> layer -> role -> environment -> node
```

Later overlays override earlier overlays for the same key. This lets common
defaults remain reusable while node-specific risk posture can still be explicit.

## Inventory Schema

Node inventory describes desired identity, labels, requested capabilities, and
layer declarations.

```yaml
id: prod-web-01
hostname: prod-web-01.example.internal
os: linux
labels:
  runlane.io/env: prod
  runlane.io/role: web
layers:
  primary: system
  supports:
    - system
    - platform
    - application
capabilities:
  requested:
    - os.linux
    - service.systemd
policy:
  profile: production
```

Inventory does not include current service output, logs, command results, or
incident state.

## Role Schema

Roles group reusable desired intent.

```yaml
id: web
layers:
  primary: system
runbooks:
  enabled:
    - service-unhealthy
policies:
  profile: production
allowlists:
  enabled:
    - allow-sshd-restart
```

## Runbook Schema

Runbooks declare layer, parameters, resources, collection, proposal options,
leases, recovery actions, and verification. Runtime evidence is referenced by
id after collection; it is not committed back to the runbook file.

See `examples/runbooks/service-unhealthy.yaml`.

## Policy Schema

Policies declare approval, verification, lease, and helper requirements.

```yaml
id: production
approval:
  system:
    service.restart: required
verification:
  tier3:
    default: false
helper:
  require_signed_lease: true
  reject_replay: true
```

## Allowlist Schema

Allowlists declare local helper permission intent.

```yaml
id: allow-sshd-restart
action: service.restart
target_resource_id: system:node/prod-web-01/service/sshd
```

A signed lease must still bind to the node, action, target, approval, run,
expiry, nonce, and allowlist entry. The allowlist alone does not authorize an
action.

## Layer Declarations

Schemas must preserve all operational layers:

```yaml
layers:
  primary: system
  supports:
    - system
    - platform
    - application
```

`kind` remains the technical shape. `layer` remains operational meaning.

## Runtime Boundary

Do not write these high-churn runtime facts to the fleet repo:

- heartbeats;
- observed facts;
- incident states;
- evidence;
- proposals;
- approvals;
- resource lease decisions;
- helper output;
- verification results;
- audit events;
- cognitive receipts.

The server ledger owns those facts. Git owns desired intent.
