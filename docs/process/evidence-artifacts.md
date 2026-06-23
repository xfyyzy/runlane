# Evidence Artifacts

Runlane validation evidence must be reviewable without committing raw logs,
private host paths, VM transcripts, Telegram identifiers, or other bulky runtime
truth to the source repository.

## Storage Policy

Use GitHub Actions artifacts for validation evidence.

The source repository stores:

- evidence policy and redaction rules;
- workflow definitions that can reproduce evidence;
- acceptance report references to workflow runs and artifact names;
- short human summaries of what evidence proves.

The source repository must not store:

- raw VM logs;
- raw helper install transcripts;
- raw Telegram Bot API payloads;
- private token, chat, user, host, VM image, or workstation identifiers;
- large generated output that belongs in an artifact.

## Workflow

The canonical evidence workflow is `.github/workflows/evidence.yml`.

Use it manually from GitHub Actions or the CLI:

```bash
gh workflow run Evidence --ref main -f smoke=all
gh workflow run Evidence --ref main -f smoke=safe
gh workflow run Evidence --ref main -f smoke=host-mutating-dry-runs
```

The workflow writes logs under `target/evidence/` and uploads them with
`actions/upload-artifact`. Each artifact includes command logs, run metadata,
the evidence policy snapshot, and the v0.1.2 acceptance report snapshot from the
checked-out commit. Artifact names use this shape:

```text
runlane-evidence-<smoke>-<github-run-id>
```

The default retention period is 90 days. Acceptance reports should record the
GitHub Actions run URL, artifact name, commit SHA, smoke set, and any redaction
or blocked-evidence notes.

## Smoke Sets

The workflow delegates to `cargo xtask smoke ...`.

Supported evidence sets:

| Evidence set | What runs | Host mutation |
|---|---|---|
| `all` | `cargo xtask smoke safe` plus host-mutating and VM smoke dry-runs | No real host mutation |
| `safe` | non-root local smoke suite | No real host mutation |
| `fleet` | fleet parse and GitOps ingest smoke | No |
| `control-plane` | server and agent demo boundary | No |
| `http` | loopback HTTP transport smoke | Starts and tears down a localhost server |
| `telegram-live-simulated` | Telegram adapter tests and simulated approval smoke | No secrets read |
| `e2e` | deterministic service-unhealthy and disk-pressure receipt paths | No |
| `host-mutating-dry-runs` | dry-run command/side-effect records for Linux helper, Linux dogfood, FreeBSD VM, and OpenBSD VM smokes | No real host mutation |

Real host-mutating Linux, FreeBSD, and OpenBSD evidence must come from an
operator-approved environment. Do not convert those runs into source-controlled
logs. When a real host-mutating run is needed, prefer an Actions environment or
self-hosted runner that can upload artifacts directly. If that environment is
not available, report the evidence as blocked and include the exact missing
runner/environment requirement.

Existing local or VM logs from before this policy cannot be honestly backfilled
into a GitHub Actions artifact from a GitHub-hosted runner, because that runner
cannot read private `/tmp` paths or one-host lab files. For those historical
runs, keep only redacted source-controlled summaries and run new artifact-backed
evidence when a reproducible artifact is required.

## Redaction Rules

Before linking an artifact in an acceptance report or PR, inspect the logs for:

- API tokens, bot tokens, cookies, passwords, and private keys;
- Telegram chat IDs, user IDs, bot usernames, and message payloads beyond
  intentionally redacted smoke output;
- private hostnames, IP addresses, VM image names, and local-only operator paths;
- sudoers/doas fragments that expose a real operator account broader than the
  smoke principal;
- raw command output that contains unrelated host data.

If redaction is needed, rerun the evidence workflow with redacted inputs or a
safer smoke mode. Do not patch application code to hide an environment evidence
problem.

## Reporting Format

Use this shape in acceptance reports and PRs:

```text
Evidence:
- workflow: Evidence
- run: https://github.com/<owner>/<repo>/actions/runs/<run-id>
- artifact: runlane-evidence-safe-<run-id>
- commit: <sha>
- smoke: safe
- redaction: inspected; no secrets or private host identifiers found
```

For blocked evidence:

```text
Blocked evidence:
- smoke: openbsd-vm-validation
- reason: no operator-approved OpenBSD artifact-producing runner is available
- repair path: provide a native OpenBSD runner/VM path that can upload GitHub
  Actions artifacts, then run `cargo xtask smoke openbsd-vm-validation
  --confirm-host-mutation`
```
