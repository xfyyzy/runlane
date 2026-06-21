# Runlane Product Definition

## One-sentence definition

Runlane is a self-hosted AI operations control plane for layered Unix-like infrastructure.

## Short definition

Runlane coordinates agents that diagnose and safely change machines. It is built for operators who already use AI to manage real servers, but need the work to become reusable, concurrent, auditable, and cross-platform instead of scattered across per-node repositories, chat bots, and temporary privileged accounts.

Runlane models operations across three layers:

- **system layer**: OS/kernel/kernel config/modules/system libraries/packages/users/firewall/filesystems/service manager;
- **platform layer**: middleware, databases, gateways, queues, caches, observability stack;
- **application layer**: business services, bots, workers, app config, release artifacts.

v0.1 dogfoods the system layer first, while the schema and scheduler are layer-aware from day one.

## The problem Runlane solves

The target operator already has a working but fragmented AI operations workflow:

- each machine or node batch may have its own operations repository;
- those repositories contain runbooks and remote state;
- tasks are initiated manually;
- temporary `passwordless sudo` / `doas` access is hard to grant and revoke;
- Telegram bot integrations work for one cluster but are hard to reuse elsewhere;
- multiple OS families exist in the same fleet;
- agents often run either too many unrelated checks or too few checks;
- concurrent tasks are either serialized unnecessarily or allowed to collide;
- system, platform, and application operations have different change frequencies but are often treated as the same kind of task.

Runlane turns that workflow into a control plane.

## Core thesis

Runlane should not optimize for "the agent can do anything".

Runlane should optimize for:

1. the agent can only do what its capability lease permits;
2. every resource belongs to an operational layer;
3. every action has a declared impact scope;
4. every verification check is tied to the action's layer and impact scope;
5. concurrent work is scheduled through explicit resource leases;
6. every decision, skip, approval, and side effect is recorded.

## What Runlane does

### 1. Fleet intent management

Runlane consumes versioned operational intent from one or more fleet repositories:

- inventory;
- node groups;
- roles;
- runbooks;
- policies;
- operational layer declarations;
- platform capability mappings;
- local privileged-action allowlists;
- reusable scripts.

Git remains the source of truth for desired operational intent. Runtime truth belongs in Runlane's event store.

### 2. System-level diagnosis first

Runlane agents collect platform-native evidence:

- service/daemon state;
- logs;
- process snapshots;
- socket and port state;
- disk and filesystem state;
- memory and CPU pressure;
- package manager facts;
- user/group/privilege facts;
- scheduled job state;
- firewall/routing facts;
- certificate and time synchronization facts;
- OS/kernel facts.

### 3. Safe recovery execution

Runlane can execute typed recovery actions:

- restart/reload a service;
- apply an allowlisted cleanup script;
- rotate or remove an allowlisted file;
- reload firewall rules after syntax validation;
- apply a package update policy;
- disable/enable a scheduled job;
- revoke a temporary privilege lease;
- reboot a node only after a drain/exclusive lease.

The model must never generate arbitrary shell that becomes execution.

### 4. Relevance-scoped verification

Runlane does not blindly run every possible check after every change.

Every action declares:

- layer;
- what it reads;
- what it writes;
- what it may indirectly affect;
- what must be verified;
- what broader checks are intentionally not run and why.

The verifier chooses the smallest sufficient check set that protects the declared impact boundary.

### 5. Dependency- and conflict-aware concurrency

Runlane schedules multiple tasks through a resource lease model.

It can run tasks concurrently when their resource sets do not conflict, and it serializes or blocks tasks when they require the same exclusive resource or when a lower-layer mutation would invalidate upper-layer work.

### 6. Human approval and cognitive receipts

Human approval is a runtime interrupt, not a chat message side effect.

For each meaningful run, Runlane should produce a cognitive receipt:

- which operational layer changed;
- what changed;
- why it changed;
- what evidence supported the decision;
- what checks were run;
- what checks were skipped;
- what upper layers may be affected;
- what risks remain;
- how to rollback or take over manually.

## What Runlane does not do

Runlane is not:

- a generic agent framework;
- a generic MCP marketplace;
- a Kubernetes-only operator;
- a CI system;
- a deployment platform first;
- a chat bot with server commands;
- a remote shell;
- a replacement for Prometheus/Grafana/PagerDuty;
- a platform that assumes systemd or Linux.

Runlane may integrate with those systems, but they are not its identity.

## First dogfood domain

The first dogfood domain is system operations, not application deployment and not platform orchestration.

Good first scenarios:

- service unhealthy;
- disk pressure;
- log volume growth;
- failed cron job;
- zombie/high-resource process;
- port unexpectedly closed/open;
- expired or expiring certificate;
- package update requires service restart;
- firewall rule reload;
- temporary privileged operator lease revoke;
- reboot required after kernel/package change.

Avoid early scenarios that require deep application semantics, Kubernetes controllers, or business-level workflows.

## Product invariants

1. **No arbitrary shell as a recovery primitive.** Typed actions only.
2. **No model-to-command direct path.** Model output is proposal data.
3. **No broad permanent privileged agent.** Privilege is a helper + lease + local allowlist.
4. **No hidden global full-gate requirement.** Verification must be relevant to layer and impact.
5. **No unmodeled concurrency.** Mutating tasks require resource leases.
6. **No Linux-first semantics.** Linux, FreeBSD, and OpenBSD are first-class in v0.1.
7. **No chat-specific business logic.** Telegram/Discord/Feishu are adapters.
8. **No runtime state hidden in Git commits.** Runtime event truth belongs in the ledger.
9. **No layer collapse.** System, platform, and application operations must remain distinguishable in the domain model.
