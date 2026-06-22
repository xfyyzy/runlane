# Agent Local State

Runlane agents keep local configuration and enrolled identity metadata outside
the fleet Git intent repository.

This local state is node runtime truth. It is not committed to fleet intent,
and it is not a substitute for the server ledger.

## Files

The v0.1.1 local agent state boundary uses these files:

```text
/etc/runlane-agent/agent.yaml
/etc/runlane-agent/trust-root.pem
/var/lib/runlane-agent/identity.yaml
/var/lib/runlane-agent/client.crt
/var/lib/runlane-agent/client.key
/var/spool/runlane-agent/
```

The paths are examples. Operators can place the files elsewhere, but the paths
stored in `agent.yaml` must be absolute so startup does not depend on the
current working directory.

## Config Schema

`agent.yaml`:

```yaml
node_id: prod-web-01
server_url: https://runlane.example
server_trust_root_path: /etc/runlane-agent/trust-root.pem
identity_path: /var/lib/runlane-agent/identity.yaml
certificate_path: /var/lib/runlane-agent/client.crt
private_key_path: /var/lib/runlane-agent/client.key
spool_dir: /var/spool/runlane-agent
platform_family: linux
```

Supported `platform_family` values for v0.1 are:

- `linux`;
- `freebsd`;
- `openbsd`.

`server_url` must use `https://`. Plaintext agent startup is not treated as a
production-secure mode.

## Identity Schema

`identity.yaml` is written after enrollment or by the local CLI-safe identity
install command:

```yaml
node_id: prod-web-01
platform_family: linux
certificate_fingerprint: sha256:example
server_trust_root_path: /etc/runlane-agent/trust-root.pem
certificate_path: /var/lib/runlane-agent/client.crt
private_key_path: /var/lib/runlane-agent/client.key
enrolled_at_unix_seconds: 1780000000
expires_at_unix_seconds: null
```

The identity metadata must match the config. A mismatched node id, platform
family, trust root path, certificate path, or private key path fails closed.

## Permission Rules

The agent validates local files before startup:

- `agent.yaml` must be a regular file and must not be group/other writable;
- the server trust root and agent certificate must be regular files and must
  not be group/other writable;
- the private key and `identity.yaml` must be regular files with no group/other
  access;
- the spool path must be a directory and must not be group/other writable.

These checks are enforced where Unix permission bits are available. Linux,
FreeBSD, and OpenBSD are Unix targets, so this is part of the normal v0.1 agent
startup preflight.

## Clean-Machine Smoke

This smoke uses local temporary files and does not require private developer
machine state:

```bash
root="$(mktemp -d)"
mkdir -p "$root/etc" "$root/lib" "$root/spool"
printf 'trust-root\n' > "$root/etc/trust-root.pem"
printf 'client-cert\n' > "$root/lib/client.crt"
printf 'client-key\n' > "$root/lib/client.key"
chmod 0644 "$root/etc/trust-root.pem" "$root/lib/client.crt"
chmod 0600 "$root/lib/client.key"
chmod 0700 "$root/spool"

cargo run -p runlane-agent -- config init \
  --config "$root/etc/agent.yaml" \
  --node-id prod-web-01 \
  --server-url https://runlane.example \
  --trust-root-path "$root/etc/trust-root.pem" \
  --identity-path "$root/lib/identity.yaml" \
  --certificate-path "$root/lib/client.crt" \
  --private-key-path "$root/lib/client.key" \
  --spool-dir "$root/spool" \
  --platform-family linux

cargo run -p runlane-agent -- identity install \
  --config "$root/etc/agent.yaml" \
  --certificate-fingerprint sha256:demo \
  --enrolled-at 1780000000

cargo run -p runlane-agent -- config show \
  --config "$root/etc/agent.yaml"
cargo run -p runlane-agent -- config validate \
  --config "$root/etc/agent.yaml"
cargo run -p runlane-agent -- run \
  --config "$root/etc/agent.yaml"
```

Use the local platform family in the smoke. For example, run the same shape
with `--platform-family freebsd` on FreeBSD and `--platform-family openbsd` on
OpenBSD.

## Current Boundary

`runlane-agent identity install` is the current local persistence boundary for
enrolled identity metadata. It is useful for CLI-safe development and tests
until real enrollment transport writes the same local state.

It does not claim that plaintext HTTP is production-secure, and it does not
replace server-side enrollment, mTLS identity extraction, or certificate
lifecycle management.
