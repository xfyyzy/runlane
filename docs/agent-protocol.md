# Runlane Agent Pull Protocol

Runlane agents pull work from the server. Nodes do not need inbound ports.

This document defines the v0.1 protocol contract. The current implementation
provides an in-process control-plane boundary with the same validation
semantics; adding an HTTP transport must call that boundary rather than
inventing a second path.

## Transport And Identity

Agents enroll once with a short-lived enrollment token. After enrollment, the
agent uses mTLS for server communication.

The enrolled identity binds:

- node id;
- certificate fingerprint;
- server trust root;
- certificate lifecycle metadata.

The server rejects task pulls and result submissions when the mTLS identity does
not match the node id in the request.

## Local Config And Identity State

The agent loads local configuration before startup. The config declares:

- node id;
- server URL;
- server trust root path;
- local identity metadata path;
- certificate and private key paths;
- spool directory;
- platform family.

After enrollment, the agent persists local identity metadata that binds the node
id, platform family, certificate fingerprint, trust root path, certificate path,
private key path, and certificate lifecycle timestamps.

The current executable boundary is documented in
[`agent-local-state.md`](agent-local-state.md). Local identity installation is a
CLI-safe persistence path for development and tests until real enrollment
transport writes the same state. It does not replace server-side enrollment,
mTLS identity extraction, or certificate lifecycle management.

## Pull Endpoint Shape

The agent polls the server:

```text
POST /v1/agent/pull
```

Request:

```yaml
node_id: prod-web-01
capability_report_version: cap-123
last_seen_task_nonce: nonce-previous
```

Response:

```yaml
envelope_id: env-123
run_id: run-123
task_id: task-123
node_id: prod-web-01
issued_at_unix_seconds: 1780000000
expires_at_unix_seconds: 1780000060
nonce: task-nonce-123
required_capabilities:
  - service.systemd
audit_correlation_id: audit-123
```

The response is a task envelope. The task envelope is not executable shell.

## Replay Protection

Before local execution, the agent validates:

1. envelope node id matches the local node identity;
2. envelope has not expired;
3. envelope nonce has not already been seen.

A replayed task envelope fails closed. It is reported to the server as a
protocol rejection when possible and is otherwise retained as local evidence.

## Task Execution Boundary

The agent may:

- collect evidence using native platform backends;
- execute unprivileged typed local actions;
- call `runlane-helper` only when a valid signed capability lease is present.

The agent must not treat task text, evidence, or model output as shell.

## Result Submission

The agent submits structured results:

```text
POST /v1/agent/result
```

```yaml
envelope_id: env-123
run_id: run-123
task_id: task-123
node_id: prod-web-01
nonce: result-nonce-123
status: succeeded
evidence:
  - source: service_status
    content_type: text/plain
    body: sshd active
    truncated: false
audit_correlation_id: audit-123
```

The result includes enough metadata for the server ledger to link task pull,
local execution, evidence, and verification.

## Local Spool Semantics

If result submission fails, the agent writes a local spool item containing:

- spool id;
- reason, such as server unavailable or submission rejected;
- original structured result submission;
- audit correlation id.

Spooling is not success. It is durable local evidence that must be retried or
reported when connectivity returns.

## Minimum Server API

v0.1 protocol APIs:

- enrollment token creation;
- agent enrollment;
- task pull;
- result submission;
- spool replay submission.

All adapter and CLI surfaces should call these APIs rather than inventing a
separate execution path.

## Local Demo Boundary

The current CI-safe executable boundary is in-process:

```bash
cargo run -p runlane-server -- demo-control-plane
cargo run -p runlane-agent -- demo-enroll-pull
```

These commands exercise enrollment token validation, agent enrollment, typed
task pull, structured result submission, and audit events without requiring
inbound node ports. The task payload is typed data; there is no shell command
field.

The current local agent startup preflight is:

```bash
cargo run -p runlane-agent -- config init ...
cargo run -p runlane-agent -- identity install ...
cargo run -p runlane-agent -- config validate --config <agent.yaml>
cargo run -p runlane-agent -- run --config <agent.yaml>
```

Startup fails closed for missing config, missing identity metadata, mismatched
identity, unsafe permissions, platform mismatch, or missing trust/certificate
files.
