# Runlane Helper Contract

The helper is Runlane's privileged local action boundary.

It is a narrow executable that accepts typed requests and signed capability
leases. It is not a shell gateway, command runner, or policy decision maker.

## Installation Model

The local OS grants the `runlane` agent user permission to invoke only the
helper executable as root.

Linux and FreeBSD sudoers concept:

```text
runlane ALL=(root) NOPASSWD: /usr/local/libexec/runlane-helper
```

OpenBSD doas concept:

```text
permit nopass runlane as root cmd /usr/local/libexec/runlane-helper
```

These rules do not allow arbitrary shell. They only allow invocation of the
helper, which still fails closed unless every lease and allowlist check passes.

## Lease Claims

A signed capability lease binds all of these fields:

```yaml
lease_id: lease-123
run_id: run-123
approval_id: approval-123
node_id: prod-web-01
action: service.restart
target:
  resource_id: system:node/prod-web-01/service/sshd
  subject: sshd
allowlist_entry_id: allow-sshd-restart
expires_at_unix_seconds: 1780000000
nonce: 4e4f7d0f-lease-nonce
```

The signature envelope carries:

```yaml
key_id: server-signing-key-1
signature: <opaque signature bytes>
claims: <claims above>
```

The helper cryptographic layer verifies the signature before claim validation.
The domain validator then checks the remaining fields deterministically.

## Local Allowlist Format

The helper has a local allowlist. A lease is not enough by itself.

```yaml
entries:
  - id: allow-sshd-restart
    action: service.restart
    target_resource_id: system:node/prod-web-01/service/sshd
```

The allowlist entry id in the lease must match an entry on the node, and that
entry must permit both the action and target resource.

## Helper Request Format

The request is typed data:

```yaml
lease_id: lease-123
action: service.restart
target:
  resource_id: system:node/prod-web-01/service/sshd
  subject: sshd
arguments:
  - name: service
    value: sshd
```

There is no shell command field. A script action, if allowed later, must use a
typed script id from the local allowlist and typed arguments.

## Helper Response Format

The response is structured:

```yaml
status: succeeded
message: service restart requested
```

The agent reports this result to the server. The server records it in the
append-only ledger and schedules verification from the run impact set.

## Fail-Closed Checks

The helper rejects the request if any of these checks fail:

1. signature is invalid;
2. lease is expired;
3. nonce has already been used;
4. lease node id does not match the local node identity;
5. request lease id does not match the signed lease;
6. request action does not match the signed lease;
7. request target does not match the signed lease;
8. local allowlist does not permit the action and target.

Invalid, expired, replayed, mismatched, and locally disallowed leases are
denied before any privileged side effect.

## Initial Typed Actions

v0.1 helper actions are intentionally small:

- `service.restart`;
- `service.reload`;
- `file.remove_from_allowlist`;
- `script.run_allowlisted`.

Adding a new action requires a typed request shape, local allowlist rule,
lease claim binding, helper implementation, verification plan, and tests.
