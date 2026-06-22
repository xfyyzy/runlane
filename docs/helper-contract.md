# Runlane Helper Contract

The helper is Runlane's privileged local action boundary.

It is a narrow executable that accepts typed requests and signed capability
leases. It is not a shell gateway, command runner, or policy decision maker.

## Installation Model

The local OS grants the `runlane` agent user permission to invoke only the
helper executable as root.

Install the helper as a root-owned executable. It must not be group- or
world-writable, and it must not be a setuid shell wrapper.

Linux sudo path:

```bash
sudo install -o root -g root -m 0755 \
  target/x86_64-unknown-linux-musl/release/runlane-helper \
  /usr/local/libexec/runlane-helper
printf 'runlane ALL=(root) NOPASSWD: /usr/local/libexec/runlane-helper\n' |
  sudo tee /etc/sudoers.d/runlane-helper
sudo chmod 0440 /etc/sudoers.d/runlane-helper
sudo visudo -cf /etc/sudoers.d/runlane-helper
```

FreeBSD sudo path:

```bash
sudo install -o root -g wheel -m 0755 \
  target/x86_64-unknown-freebsd/release/runlane-helper \
  /usr/local/libexec/runlane-helper
printf 'runlane ALL=(root) NOPASSWD: /usr/local/libexec/runlane-helper\n' |
  sudo tee /usr/local/etc/sudoers.d/runlane-helper
sudo chmod 0440 /usr/local/etc/sudoers.d/runlane-helper
sudo visudo -cf /usr/local/etc/sudoers.d/runlane-helper
```

OpenBSD doas path:

```bash
doas install -o root -g wheel -m 0755 \
  /path/to/runlane-helper \
  /usr/local/libexec/runlane-helper
printf 'permit nopass runlane as root cmd /usr/local/libexec/runlane-helper\n' |
  doas tee -a /etc/doas.conf
```

The resulting policy rule shape is:

Linux and FreeBSD sudoers:

```text
runlane ALL=(root) NOPASSWD: /usr/local/libexec/runlane-helper
```

OpenBSD doas:

```text
permit nopass runlane as root cmd /usr/local/libexec/runlane-helper
```

These rules do not allow arbitrary shell. They only allow invocation of the
helper, which still fails closed unless every lease and allowlist check passes.

Run preflight after installation through the installed helper path. On Linux
and FreeBSD, use `sudo -n`; on OpenBSD, use `doas -n`.

```bash
sudo -n /usr/local/libexec/runlane-helper preflight \
  --helper-binary /usr/local/libexec/runlane-helper \
  --allowlist-file /etc/runlane/helper-allowlist.yaml \
  --expected-owner-uid 0 \
  --expected-mode 0755
```

Preflight checks that:

1. the helper path exists and is a regular executable file;
2. the helper owner uid and mode match the declared expectation;
3. the helper is not group- or world-writable;
4. the allowlist file is readable and parses as a helper allowlist;
5. this helper build exposes dry-run validation support.

The executable entrypoint is explicit:

```bash
runlane-helper execute \
  --lease-file lease.yaml \
  --request-file request.yaml \
  --allowlist-file allowlist.yaml \
  --node-id prod-web-01 \
  --now 1780000000 \
  --dry-run
```

`--dry-run` validates the full typed boundary and returns structured success
without mutating the developer machine. Non-dry-run host mutation is not
implemented until real service restart execution is introduced deliberately.

A reproducible local smoke uses fixtures under `examples/helper-smoke`:

```bash
cargo run -p runlane-helper -- dry-run-smoke \
  --lease-file examples/helper-smoke/lease-valid.yaml \
  --request-file examples/helper-smoke/request-restart.yaml \
  --allowlist-file examples/helper-smoke/allowlist.yaml \
  --node-id prod-web-01 \
  --now 1780000000
```

The rejection path should fail before any action execution:

```bash
cargo run -p runlane-helper -- dry-run-smoke \
  --lease-file examples/helper-smoke/lease-invalid-signature.yaml \
  --request-file examples/helper-smoke/request-restart.yaml \
  --allowlist-file examples/helper-smoke/allowlist.yaml \
  --node-id prod-web-01 \
  --now 1780000000
```

## Lease Claims

A signed capability lease binds all of these fields:

```yaml
lease_id: lease-123
run_id: run-123
approval_id: approval-123
node_id: prod-web-01
action: service.restart
target_resource_id: system:node/prod-web-01/service/sshd
target_subject: sshd
allowlist_entry_id: allow-sshd-restart
expires_at_unix_seconds: 1780000000
nonce: 4e4f7d0f-lease-nonce
```

The signature envelope carries:

```yaml
key_id: server-signing-key-1
signature: <opaque signature bytes>
signature_status: valid
claims: <claims above>
seen_nonces: []
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
target_resource_id: system:node/prod-web-01/service/sshd
target_subject: sshd
arguments:
  service: sshd
```

There is no shell command field. A script action, if allowed later, must use a
typed script id from the local allowlist and typed arguments.

## Helper Response Format

The response is structured:

```yaml
status: succeeded
action: service.restart
target: system:node/prod-web-01/service/sshd
dry_run: true
message: validated typed service.restart without mutating host
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
