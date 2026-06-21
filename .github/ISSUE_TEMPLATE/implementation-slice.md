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
