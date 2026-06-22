# AGENTS.md

This file defines how AI agents should work inside the Runlane repository.

It is for repository-maintenance agents and coding agents. It is not a runtime policy file for Runlane nodes. Runtime policy belongs in Runlane's domain model, fleet repositories, policy engine, helper allowlists, and audit ledger.

## 0. Purpose

Runlane is an operations control plane for real machines. A careless implementation agent can easily create the same class of risks that Runlane itself is designed to prevent: hidden fallback paths, broad privilege, unverified execution, stale documentation, and unbounded agent behavior.

This file turns the project's engineering taste into executable collaboration rules.

It distinguishes four layers of guidance:

1. **Engineering taste** — long-term values for uncertain decisions.
2. **Collaboration principles** — how agents work with humans and future agents.
3. **Project hard rules** — constraints that must not be silently bypassed.
4. **Execution mechanisms** — concrete ways to plan, change, verify, commit, and hand off work.

## 1. Rule hierarchy and conflict handling

Use this priority order when rules conflict:

1. The user's explicit decision in the current task.
2. Project hard rules in this file.
3. Execution mechanisms in this file.
4. Collaboration principles in this file.
5. Engineering taste in this file.

However, the following must never be silently overridden:

- fallback or degraded behavior;
- hidden errors;
- pollution of system/global environments;
- long-lived compatibility baggage;
- changes that affect the project's future architecture direction;
- changes that cannot be reproduced, verified, or handed off;
- broad privilege or arbitrary shell execution paths;
- Runlane semantics that collapse system/platform/application layers.

If a task needs one of those exceptions, explain the trade-off and ask for a decision before proceeding.

## 2. Engineering taste

### 2.1 Long-term maintenance beats short-term success

Prefer the path that keeps Runlane maintainable over the path that merely makes the current command pass.

Do not introduce:

- temporary bypasses;
- implicit fallback;
- untracked manual steps;
- machine-specific assumptions;
- compatibility layers without an explicit lifecycle;
- hidden state outside the repo or declared environment;
- success paths that a clean machine cannot reproduce.

### 2.2 Mechanism beats one-off results

The goal is not "this run succeeded". The goal is "the same class of run can succeed again through a reproducible mechanism".

Prefer:

- type-level invariants;
- schema validation;
- explicit state machines;
- preflight checks;
- automated tests;
- CI gates;
- audit events;
- reproducible scripts.

### 2.3 One path beats parallel paths

The same problem should have one preferred solution. When a better path is introduced, converge to it.

Do not keep old and new paths alive unless the user explicitly accepts the cost and a removal plan exists.

### 2.4 Explicit failure beats hidden resilience

Errors should fail early, loudly, and close to their cause.

Do not swallow errors, silently skip checks, downgrade features, or continue from an unreliable state.

### 2.5 Boundaries first

If a module, directory, interface, data flow, execution path, privilege boundary, or policy boundary does not exist yet, define the boundary before adding behavior.

If a boundary already exists, do not bypass it.

### 2.6 Mechanistic constraints beat carefulness

Do not rely on agent caution as the safety model. Encode safety in mechanisms:

- Rust types and enums;
- explicit `Result` errors;
- schema validation;
- capability and resource lease objects;
- allowlists;
- CI checks;
- preflight checks;
- single entrypoints;
- documented invariants.

## 3. Collaboration principles

### 3.1 Explain consequential changes before making them

Before making a consequential change, state:

- why it is needed;
- what will change;
- which old path disappears;
- what the new single path is;
- what risks exist;
- how the result will be verified;
- whether a user decision is needed.

Small, obviously safe edits can proceed directly.

### 3.2 Report abnormal states

If something is inconsistent, surprising, unsupported, or not explainable, report it.

Do not rationalize abnormal state just to keep moving. In particular:

- do not disguise environment problems as application problems;
- do not disguise design problems as implementation details;
- do not report success from partial evidence;
- do not hide risk in vague "handled" language.

### 3.3 Ask before changing future direction

Ask for a decision before changes that affect Runlane's future shape:

- architecture direction;
- runtime security model;
- resource/lease/verification semantics;
- data model;
- public API or CLI contract;
- build system;
- deployment path;
- major dependency stack;
- directory structure;
- removal of a declared capability or non-goal.

### 3.4 Technical debt enters the repayment path when found

If debt can be safely removed inside the current task, remove it.

If removal is risky or changes direction, report it and ask for a decision. Do not silently leave known debt only because it is not the primary task.

### 3.5 Handoff is part of the task

Every meaningful task should leave enough context for the next agent or human to continue:

- what changed;
- why it changed;
- key decisions;
- rejected alternatives;
- current state;
- verification results;
- remaining risks;
- next suggested step;
- commit or PR handles.

### 3.6 Deliverables must be verifiable and reproducible

Do not deliver only a conclusion. Provide the exact verification command or handle, expected result, and any environment assumptions.

## 4. Project hard rules

### 4.1 No unapproved downgrade or fallback

Never implement fallback, downgrade, or weaker behavior without explicit approval.

Examples of forbidden silent downgrade:

- using a weaker implementation because a dependency is missing;
- skipping a key check because a tool is unavailable;
- changing application logic to hide an environment problem;
- weakening tests to make CI pass;
- swallowing an unstable external API error;
- preserving duplicate old/new paths for convenience.

If the best path needs a missing tool, dependency, permission, or OS capability, identify it as an environment/preflight failure and provide the repair path.

### 4.2 Rust toolchain is explicit

Runlane currently targets Rust stable 1.96.0 or newer.

- `Cargo.toml` `rust-version` is the source of truth for MSRV.
- `rust-toolchain.toml` keeps local development on stable with required components.
- Do not pin an older toolchain or lower MSRV without a user decision.
- Do not make CI pass by disabling rustfmt, clippy, tests, or feature checks.

### 4.3 Python, if introduced, must use uv

Runlane is Rust-first. If Python scripts or tooling are introduced, they must use `uv` for virtual environments, dependency installation, and script execution.

Do not:

- install project dependencies with global `pip`;
- rely on undeclared system Python packages;
- depend on user-private Python configuration;
- make the current machine's accidental Python state part of the project.

### 4.4 Environment problems are not application problems

When a failure is caused by missing system packages, permissions, credentials, OS capabilities, toolchain versions, or external resources:

1. identify the missing prerequisite;
2. explain why the prerequisite is part of the best path;
3. provide an installation or repair path;
4. add a preflight check when the failure can be detected earlier;
5. do not alter application semantics to hide the environment issue.

### 4.5 Greenfield refactoring: converge, do not accrete

This project does not preserve historical baggage by default.

When refactoring:

- migrate callers to the new best path;
- delete old paths;
- delete old docs and prompts;
- remove duplicate implementations;
- avoid long-term compatibility layers;
- make the new path the only natural path.

Do not keep talking about removed paths in docs, comments, or errors unless migration users need a temporary release note.

### 4.6 Semantic changes must be committed promptly

When a change is semantically complete and verifiable, commit it.

Semantic commits belong on an issue branch, not directly on `main`. The normal
repository path is:

1. start from the latest `main`;
2. create `issue-<number>-<short-slug>`;
3. commit the coherent semantic change there;
4. open a PR linked to the issue;
5. let CI and review complete before merge.

A commit should be:

- coherent;
- explainable;
- independently verifiable;
- free of unrelated edits;
- accompanied by docs when behavior/process changed.

### 4.7 Long-running tasks must use tmux

Long-running tasks must run inside `tmux` and write logs to a stable path.

When starting one, record:

```text
tmux session:
log path:
command:
current state:
check/resume command:
```

After leaving an observable entrypoint, do not blindly poll forever. Let the user or next agent resume from the recorded handle.

### 4.8 Documentation must match reality

If behavior, environment requirements, execution flow, deployment flow, directory structure, public API, CLI behavior, or verification method changes, update docs in the same semantic change.

Docs should describe only the current valid path. Commands in docs must be executable and assumptions must be explicit.

### 4.9 Runlane product invariants must not be bypassed

Repository changes must preserve these product invariants unless the user explicitly changes project direction:

- system/platform/application layers remain first-class;
- `kind` describes technical shape; `layer` describes operational meaning;
- Git stores desired operational intent; server ledger stores runtime truth;
- agents pull tasks; nodes do not require inbound ports;
- privileged actions go through narrow helpers and signed capability leases;
- logs and command output are untrusted evidence;
- LLM output is proposal data, never executable shell;
- verification is layer- and impact-scoped;
- concurrency is controlled by dependencies and `ResourceLease`s;
- Linux, FreeBSD, and OpenBSD remain first-class v0.1 targets;
- chat integrations are adapters, not the product core.

## 5. Execution mechanisms

### 5.1 Task start checklist

Before changing files, quickly determine:

1. Does this affect future architecture direction?
2. Are boundaries clear?
3. Is a user decision required?
4. Are environment/toolchain prerequisites satisfied?
5. Is a preflight needed?
6. Could this create technical debt?
7. Is the task long-running?
8. Which docs must change?
9. What verification will prove the result?

If the answer blocks safe execution, report the risk and ask for a decision. Otherwise, establish the missing boundary and proceed.

### 5.2 Preflight mechanism

Use preflight checks when failure would require human intervention, expand damage, or indicate unmet environment assumptions.

Preflight failures must state:

1. failed item;
2. impact;
3. repair command or decision needed;
4. how to rerun.

### 5.3 Self-healing mechanism

Automatic repair is allowed when it is deterministic, low-risk, does not change project direction, does not hide the true error, is recorded, and can be verified.

After self-healing, report what was repaired and how it was verified.

### 5.4 Error handling mechanism

All errors should be explicit. Prefer moving failures earlier through:

- preflight checks;
- schema validation;
- type checks;
- dependency checks;
- permission checks;
- config checks;
- version checks;
- boundary checks.

Do not continue from failed or unknown state.

### 5.5 Boundary mechanism

For any new module, interface, data flow, policy surface, or execution path, define:

- who can call it;
- inputs;
- outputs;
- error representation;
- state owner;
- lifecycle owner;
- relationship to other modules;
- explicit non-responsibilities.

### 5.6 Single-path convergence mechanism

When multiple paths solve the same problem:

1. choose the long-term best path;
2. migrate callers;
3. delete the old path;
4. delete old docs and prompts;
5. update verification;
6. commit the coherent change.

If convergence is unsafe, explain why and ask for a decision.

### 5.7 Documentation mechanism

Whenever behavior or workflow changes, check at least:

- `README.md`;
- `CONTRIBUTING.md`;
- `AGENTS.md`;
- `docs/coding-agent-brief.md`;
- relevant architecture, execution, platform, and milestone docs;
- examples and issue templates.

### 5.8 Commit mechanism

Before committing:

1. confirm the work is on the intended `issue-<number>-<short-slug>` branch;
2. inspect `git diff` and `git status`;
3. verify no unrelated changes are included;
4. run relevant checks;
5. confirm docs are synced;
6. confirm no silent fallback or hidden failure was introduced;
7. write a semantic commit message.

### 5.9 Verification mechanism

For Rust changes, run at least:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
```

For documentation-only changes, still run the Rust checks unless the task explicitly forbids it or a preflight failure blocks them.

For GitHub changes, verify remote state and CI after pushing.

### 5.10 Handoff mechanism

A final handoff should include:

- goal;
- actual changes;
- files touched;
- decisions made;
- verification commands and results;
- commit/PR/issue/CI links;
- remaining risks;
- next recommended step.

## 6. Prohibited behaviors

Unless explicitly approved, agents must not:

- silently downgrade;
- silently fail;
- pollute global/system environments;
- keep unnecessary historical compatibility;
- introduce unreproducible manual steps;
- disguise environment problems as application problems;
- rationalize abnormal state;
- implement before defining required boundaries;
- rely on carefulness instead of mechanisms;
- change behavior without docs;
- report completion without verification;
- run long tasks outside tmux;
- mix unrelated semantic changes in one commit;
- sacrifice product direction for short-term convenience;
- add arbitrary shell execution paths;
- grant broad root privilege to agents;
- treat chat integrations as the operations kernel.

## 7. Summary

Runlane's repository rules mirror Runlane's product thesis:

> Prefer long-term mechanisms over one-off success.
> Fail explicitly.
> Keep boundaries sharp.
> Converge to a single best path.
> Verify and document every meaningful change.
> Treat agents as uncertain processes operating inside auditable, capability-scoped, resource-leased systems.
