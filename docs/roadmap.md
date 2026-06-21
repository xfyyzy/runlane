# Runlane Roadmap

## v0.1: System Layer Kernel

Goal: prove Runlane's kernel semantics on real system-layer operations across Linux, FreeBSD, and OpenBSD.

Deliverables:

- core domain model with operational layers;
- resource lease scheduler;
- impact-scoped verification planner;
- platform capability reports for Linux/FreeBSD/OpenBSD;
- append-only audit event model;
- agent pull-loop skeleton;
- helper lease verification design/implementation;
- one system-layer dogfood runbook.

## v0.2: Platform Packages

Goal: introduce platform-layer packages without bloating core.

Candidate packages:

- PostgreSQL;
- Redis;
- Nginx/Caddy as shared gateway;
- cron/scheduled-job package if not fully system-layer;
- observability integration package.

## v0.3: Application Runbook Packages

Goal: allow application teams to define app-specific health, release, rollback, and canary semantics as user-space runbooks.

## Later

- Solaris/illumos backend exploration;
- richer UI;
- hosted option;
- enterprise audit export;
- team RBAC;
- integration marketplace.
