# Runlane Platform Model

## Decision

Runlane v0.1 treats Linux, FreeBSD, and OpenBSD as first-class supported platforms.

Runlane should also be architected so additional Unix-like platforms can be added later, including Solaris and illumos distributions, without redesigning the system.

## Design stance

Do not design Runlane around the lowest common denominator POSIX shell.

Runlane should use native platform backends that expose normalized capabilities.

The control plane thinks in terms of:

- services;
- logs;
- processes;
- sockets;
- storage;
- users;
- packages;
- firewall;
- scheduled jobs;
- kernel/OS facts;
- privilege helpers.

Each platform backend maps those concepts to native commands, files, APIs, and semantics.

## Platform support tiers

### Tier 1: first-class in v0.1

- Linux;
- FreeBSD;
- OpenBSD.

Tier 1 requirements:

- agent binary can run on the OS;
- platform backend exists;
- service status collection exists;
- log collection exists;
- process/socket/disk snapshots exist;
- privilege helper installation path is documented;
- fixture tests exist for parsers;
- examples avoid treating this OS as an afterthought.

### Tier 2: designed-for but not implemented in v0.1

- Solaris;
- illumos distributions such as OmniOS, OpenIndiana, SmartOS-style environments where applicable;
- NetBSD/DragonFlyBSD may fit later but are not explicit v0.1 targets.

Tier 2 requirements:

- no core type or runbook schema should prevent adding them;
- platform capability identifiers should leave room for them;
- service/log/process abstractions should not assume systemd/rcctl only;
- privilege helper model should support non-sudo environments.

### Tier 3: out of scope unless explicitly added

- Windows;
- Kubernetes as a platform substrate;
- network appliances without a local agent;
- managed SaaS platforms.

## Backend families

A platform backend is composed of driver families:

| Driver family | Linux | FreeBSD | OpenBSD | Solaris/illumos later |
|---|---|---|---|---|
| ServiceManager | systemd, SysV later | rc.d/service | rcctl | SMF (`svcs`, `svcadm`) |
| LogProvider | journald, syslog files | syslog files | syslog files | SMF logs, syslog |
| ProcessProvider | procfs, ps | procstat, ps | ps | proc tools |
| SocketProvider | ss, netstat | sockstat, netstat | fstat, netstat | netstat/pfiles |
| StorageProvider | df, lsblk, mount | df, geom, zfs | df, mount | zfs, df, format |
| PackageProvider | apt/dnf/pacman later | pkg | pkg_info | pkg/pkgsrc/IPS depending distro |
| FirewallProvider | nft/iptables/ufw later | pf/ipfw | pf | ipfilter/pf variants |
| SchedulerProvider | cron/systemd timers | cron/periodic | cron | cron/SMF timers |
| PrivilegeProvider | sudo/helper | sudo/helper | doas/helper | sudo/pfexec/RBAC/helper |

The table is a design guide, not a promise that every driver is implemented in v0.1.

## Capability identifiers

Capabilities should be explicit and namespaced:

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

Runbooks must require capabilities rather than assuming commands.

## Collector command ownership

Agent platform backends own native collector command construction. A runbook,
an analyzer result, a log line, or user-supplied text may select typed
capabilities and resources, but it must not supply a shell command or command
fragment.

For v0.1, the backend collector specs cover:

- service status;
- recent logs;
- disk snapshot;
- process snapshot;
- listening sockets.

Service collectors accept a validated service identifier, not an arbitrary
argument string. The backend then maps that typed value into native commands:
Linux uses `systemctl`/`journalctl`, FreeBSD uses `service` and syslog files,
and OpenBSD uses `rcctl` and syslog files. Unsupported collectors or missing
typed inputs fail closed with structured reasons.

## System-layer dogfood surface

Runlane should initially focus on system resources:

```text
node
service
daemon
process
socket
port
filesystem
mount
disk
zfs-dataset
user
group
privilege-rule
package-db
firewall
route
certificate
scheduled-job
kernel-tunable
reboot
```

Application concepts such as deployments, database migrations, business jobs, queues, or product-specific health checks can be layered later as user-space runbooks.

## Portability rules for coding agents

When implementing Runlane, coding agents must follow these rules:

1. Do not add Linux-only fields to core domain types.
2. Do not make `systemd` the universal service abstraction.
3. Do not represent actions as shell strings.
4. Do not parse platform command output directly inside scheduler/server code.
5. Put platform parsing behind backend modules with fixture tests.
6. Every platform backend must report unsupported capabilities explicitly.
7. A runbook step with unsupported capabilities must fail closed with a clear reason.
8. New OS support means adding backend drivers, not changing the runbook model.
9. Native collector smoke should execute backend-owned commands through the
   agent, not through runbook-authored shell.

## Example platform capability report

```yaml
node_id: freebsd-edge-01
os:
  family: freebsd
  version: "14.2"
capabilities:
  - os.freebsd
  - service.freebsd-rc
  - logs.syslog-file
  - process.procstat
  - socket.sockstat
  - storage.df
  - storage.zfs
  - package.freebsd-pkg
  - firewall.pf
  - privilege.sudo-helper
unsupported:
  - service.systemd
  - logs.journald
```
