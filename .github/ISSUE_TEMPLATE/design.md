---
name: Design discussion
description: Propose or refine Runlane product/architecture semantics
title: "design: "
labels: ["design"]
body:
  - type: textarea
    id: problem
    attributes:
      label: Problem
      description: What ambiguity or design gap does this address?
    validations:
      required: true
  - type: textarea
    id: proposal
    attributes:
      label: Proposal
      description: Describe the proposed model or decision.
    validations:
      required: true
  - type: textarea
    id: implications
    attributes:
      label: Implications
      description: How does this affect layers, capabilities, leases, verification, audit, or platform backends?
    validations:
      required: true
