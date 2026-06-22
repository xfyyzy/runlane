# Server State

Runlane keeps desired operational intent in Git and runtime truth in server
state. The server must not write evidence, approvals, helper results,
verification outcomes, or receipts back into a fleet intent repository such as
`examples/fleet`.

## Local Layout

The current local server state layout is:

```text
runlane-state/
└── ledger/
    └── audit.yaml
```

`ledger/audit.yaml` is an append-only YAML document stream. Each document is one
`AuditEvent` with a monotonic sequence number. Loading the ledger validates the
sequence again; corrupt YAML or non-monotonic events fail explicitly.

## Ownership And Permissions

The state directory should be owned by the operating-system user that runs
`runlane-server`.

Recommended local permissions:

- state directory: readable/writable/searchable only by the server user;
- `ledger/`: readable/writable/searchable only by the server user;
- `ledger/audit.yaml`: readable/writable only by the server user.

The current Rust implementation creates the local directories and ledger file,
but deployment-specific ownership and mode enforcement belongs to installation
or service-management tooling until Runlane has a dedicated installer.

## Demo Persistence Smoke

Write the deterministic service-unhealthy demo ledger to local state:

```bash
rm -rf /tmp/runlane-state-demo
cargo run -p runlane-server -- state demo-write /tmp/runlane-state-demo examples/fleet
```

Render the receipt from the reloaded durable ledger, simulating a server restart:

```bash
cargo run -p runlane-server -- state receipt /tmp/runlane-state-demo run-demo-service-unhealthy
```

The receipt is reconstructed from `ledger/audit.yaml`. Missing required events
remain receipt-generation errors; the server does not produce partial success
receipts from incomplete durable state.
