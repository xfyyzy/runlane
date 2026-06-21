# ADR 0001: Cross-platform native agents are a core requirement

## Status

Accepted for initial design.

## Context

The target operator runs a mixed fleet: Linux, FreeBSD, and OpenBSD. Existing operations are repo/runbook driven and increasingly assisted by agents, but the workflow is fragmented across node-specific or batch-specific repositories.

Many automation tools assume Linux, systemd, containers, or Kubernetes. That assumption would make Runlane less useful for the intended operator and would distort the design early.

The operator may also use less common Unix-like systems later, including Solaris and illumos distributions. These are not v0.1 first-class targets, but the architecture must not block them.

## Decision

Runlane will treat Linux, FreeBSD, and OpenBSD as first-class platforms from v0.1.

This means:

1. Platform operations are expressed through capability traits.
2. Runbooks select capabilities, not OS-specific commands directly.
3. Each agent reports platform capability availability to the server.
4. Linux/systemd behavior cannot be the default semantic model for all systems.
5. Privileged helper installation supports sudo and doas flows.
6. Tests use fixture outputs from all three OS families.
7. Additional Unix-like platforms can be added by implementing backend driver families rather than changing the core runbook model.

## Consequences

### Positive

- The project is differentiated from Linux-only agent operations tools.
- The user's real fleet can dogfood the system early.
- Runbooks become more portable and role-oriented.
- OS differences are explicit instead of hidden in shell snippets.
- Solaris/illumos support can be added later without turning the project inside out.

### Negative

- v0.1 scope is harder.
- Some collectors/actions need per-OS implementations.
- CI for BSD targets is more complex.
- Documentation must avoid Linux-only examples.
- Core abstractions must be reviewed for accidental systemd/Linux leakage.

## Design rule

Every operational capability must answer:

- Is this available on Linux?
- Is this available on FreeBSD?
- Is this available on OpenBSD?
- If not, what capability flag prevents accidental use?
- Would this abstraction still make sense for Solaris/illumos later?

## Initial platform capability examples

```text
os.linux
os.freebsd
os.openbsd
os.solaris
os.illumos

service.systemd
service.freebsd-rc
service.openbsd-rcctl
service.smf

logs.journald
logs.syslog-file
logs.smf

process.procfs
process.procstat
process.ps

socket.ss
socket.sockstat
socket.fstat

privilege.sudo-helper
privilege.doas-helper
privilege.pfexec-helper
```

See also: `docs/platform-model.md`.
