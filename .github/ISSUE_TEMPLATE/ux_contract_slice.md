---
name: UX Contract Slice
about: Track a single UX behavior as a contract-bound implementation slice
title: "UX: "
labels: ["status:queued", "ui", "ux"]
assignees: []
---

## Outcome

Describe the user-visible behavior in one sentence.

Example:
"Opening a node into a new pane renders content on first activation and assigns focus deterministically."

## Domain

Choose the primary UX domain:

- Navigation and Camera
- Selection and Manipulation
- Pane and Workbench Lifecycle
- Content Opening and Routing
- Command Surfaces
- Viewer Presentation
- Settings and Control Surfaces
- Search and Retrieval
- Feedback, Diagnostics, and Recovery
- Accessibility and Region Navigation

## Contract

### Trigger

- What initiates the behavior?

### Preconditions

- What must already be true?

### Semantic Result

- What changes in app meaning/state?

### Focus Result

- Who owns focus after the action?

### Visual Result

- What should the user visibly perceive?

### Degradation / Failure Result

- What happens if the ideal path is unavailable?

## Authority

### Primary Graphshell Owner

- Which Graphshell subsystem owns the behavior?

### Secondary Observers

- Which other subsystems observe or react?

### Framework Role

- What is the framework allowed to do?
- Prefer one of:
  - paint only
  - layout only
  - event source only

### Boundary Rule

- What must the framework *not* decide?

## Hotspots

List likely files/modules.

## Non-Goals

List what this issue must not absorb.

## Verification

### Tests

- Which unit / scenario / regression tests must prove this?

### Diagnostics

- What diagnostics receipt or observability signal should exist?

### Docs

- Which spec/register docs must be kept in parity?

## Related Issues

- Parent hub / lane
- Adjacent issues
- Follow-on issues

## Done Gate

- [ ] The contract is implemented on the correct Graphshell owner boundary.
- [ ] Framework responsibilities remain limited to the declared role.
- [ ] Required tests pass.
- [ ] Required diagnostics exist or are updated.
- [ ] Related docs are updated.

