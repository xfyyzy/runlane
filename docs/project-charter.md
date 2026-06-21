# Runlane Project Charter

## Project name

Runlane

## Repository

`xfyyzy/runlane`

## Positioning

Runlane is a self-hosted AI operations control plane for layered Unix-like infrastructure.

It models system, platform, and application operations as capability-scoped, resource-leased, impact-verified, auditable runs across heterogeneous fleets.

## Why now

The seed workflow already exists in practice:

- AI agents are used to operate multiple machines;
- runbooks and state are stored in per-machine or per-node-batch repositories;
- Telegram bots already handle some monitoring, alerting, deployment, and log analysis;
- temporary passwordless sudo/doas is useful but hard to grant and revoke safely;
- Linux, FreeBSD, and OpenBSD all matter from day one.

The opportunity is to turn this scattered practice into a reusable control plane.

## Initial product wedge

System-layer operations across Linux, FreeBSD, and OpenBSD.

The first public story should not be "AI can operate anything". It should be:

> Runlane lets agents diagnose and safely execute system-level runbooks with scoped privilege, relevant verification, and conflict-aware scheduling.

## First-class design commitments

1. Layer-aware operations: system, platform, application.
2. Native OS backends: Linux, FreeBSD, OpenBSD in v0.1.
3. Extensible backend architecture for Solaris/illumos later.
4. Pull-based agents with no inbound node ports.
5. Narrow privileged helper instead of broad root agent.
6. Signed capability leases with replay protection.
7. Logs are untrusted evidence.
8. LLM output is structured proposal data, never shell.
9. Impact-scoped verification, not blanket full-gate execution.
10. Resource-lease scheduling, not unsafe parallelism or all-serial execution.
11. Git stores desired operational intent; server ledger stores runtime truth.
12. Chat integrations are adapters, not the product core.

## v0.1 success criteria

Runlane v0.1 is successful if it can demonstrate:

- three OS capability reports: Linux, FreeBSD, OpenBSD;
- a system-layer runbook such as service unhealthy or disk pressure;
- task scheduling with resource conflict decisions;
- verification planner that runs only relevant checks and records skipped checks;
- a privileged helper action protected by signed lease and local allowlist;
- an audit/cognitive receipt explaining the run.

## Explicit non-goals

- Do not build a generic agent framework.
- Do not start with a web dashboard.
- Do not build a Kubernetes-first operator.
- Do not make Telegram the operations core.
- Do not allow arbitrary remote shell as the recovery primitive.
- Do not model all operations as systemd services.
- Do not implement application deployment first.

## Intended implementation handoff

This repository is structured so coding agents can implement from documents rather than guessing product direction.

The implementation order should be:

1. core domain model;
2. resource lease scheduler;
3. impact-scoped verification planner;
4. platform backend skeletons;
5. audit ledger;
6. agent pull loop;
7. helper lease verification;
8. first system-layer dogfood runbook.
