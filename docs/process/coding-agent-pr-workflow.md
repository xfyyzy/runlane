# Coding Agent PR Workflow

This is the required contribution path for humans and coding agents working on
Runlane issues. It exists because Runlane itself depends on auditable,
reproducible work; repository maintenance should follow the same standard.

## Required Path

1. Start from the latest `main`.
2. Create an issue branch named `issue-<number>-<short-slug>`.
3. Make one coherent semantic change at a time.
4. Run the relevant verification commands.
5. Commit only scoped files on the issue branch.
6. Push the branch.
7. Open a PR whose body includes `Closes #<number>`.
8. Fill the PR template with real verification output.
9. Add self-review before requesting merge.
10. Merge only after required checks pass and review requirements are met.

Direct mutation of `main` is not the valid path for semantic changes.

## Branch Rules

Use this shape:

```text
issue-<number>-<short-slug>
```

Examples:

```text
issue-20-codify-coding-agent-pr-workflow
issue-24-persistent-server-ledger
issue-29-disk-pressure-dogfood
```

The branch name should identify exactly one issue. If a change is discovered
that belongs to another issue, finish or pause the current branch and create a
separate branch for the other issue.

## Commit Rules

Commits should be independently explainable and verifiable.

Before committing:

```bash
git status --short --branch
git diff --stat
git diff --check
```

Stage explicit paths when the worktree is mixed. Use `git add -A` only when the
entire worktree is known to belong to the same issue.

## PR Body Rules

Every PR must include:

- `Closes #<number>`;
- a concrete summary of what changed;
- real verification output or durable CI links;
- a self-review checklist with at least one checked item;
- docs impact;
- remaining risks or follow-up.

Generic statements such as "tests passed" are not enough unless they name the
commands or link to the CI run that executed them.

## Verification Rules

For Rust changes, run at least:

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
```

For documentation-only changes, run the same Rust checks unless a documented
preflight failure blocks them. When a task requires cross-platform or VM smoke,
record exactly which platforms were run and which were not.

## Self-Review Rules

Before merge, add a self-review note in the PR body or as a PR comment. It
must cover:

- whether the branch name matches the issue;
- whether the PR links the issue with a closing keyword;
- what verification actually ran;
- whether docs/examples changed when behavior or workflow changed;
- known risks or follow-up.

## Merge Rules

Do not manually close the linked issue after opening the PR. Use the `Closes
#<number>` keyword and let the merge close it.

After merge:

```bash
git switch main
git pull --ff-only
git branch --delete issue-<number>-<short-slug>
```

Delete the remote branch if GitHub did not do so automatically.

## Temporary Gap

This document is introduced before branch protection and the PR policy workflow
are active. During that transition, coding agents must follow the PR workflow
as closely as the current repository settings permit and document any gap in
the PR. After the PR policy and branch protection are enabled, this workflow is
enforced by repository settings in addition to process rules.
