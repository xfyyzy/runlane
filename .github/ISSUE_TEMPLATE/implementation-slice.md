---
name: Implementation slice
description: A coding-agent-ready implementation task
title: "slice: "
labels: ["implementation"]
body:
  - type: textarea
    id: objective
    attributes:
      label: Objective
      description: What should be implemented?
    validations:
      required: true
  - type: textarea
    id: docs
    attributes:
      label: Required reading
      description: Which docs must be read before implementation?
      value: |
        - AGENTS.md
        - docs/coding-agent-brief.md
        - docs/operational-layer-model.md
        - docs/execution-semantics.md
    validations:
      required: true
  - type: textarea
    id: acceptance
    attributes:
      label: Acceptance criteria
      description: What tests/docs/behavior prove this is done?
    validations:
      required: true
  - type: textarea
    id: workflow-contract
    attributes:
      label: Workflow contract
      description: Required branch, PR, verification, and merge path for this implementation slice.
      value: |
        This issue must be implemented through the repository PR workflow:

        1. Start from latest `main`.
        2. Create an issue branch named `issue-<number>-<short-slug>`.
        3. Commit only coherent semantic changes to that branch.
        4. Push the branch and open a PR linked with `Closes #<number>`.
        5. Fill the PR template with real verification output.
        6. Add a self-review note before requesting merge.
        7. Do not push directly to `main`.
        8. Do not close this issue manually unless the user explicitly directs it; let PR merge close it.
    validations:
      required: true
