## Linked Issue

Closes #21

## What Changed

- Added a PR policy check.

## Verification

- [x] `cargo fmt --all -- --check`
- [x] `cargo check --workspace`
- [x] `cargo test --workspace`
- [x] `scripts/ci/check-pr-policy.sh docs/process/pr-policy-fixtures/passing.md`

## Self-Review

- [x] I confirmed this PR is on an issue branch.
- [ ] I checked only after CI.

## Docs Impact

- [x] Docs updated in this PR.

## Risks / Follow-Up

- Branch protection is configured separately.
