# History Timeline and Temporal Navigation Spec

**Date**: 2026-02-28  
**Status**: Canonical subsystem contract  
**Priority**: Immediate implementation guidance

**Related**:
- `SUBSYSTEM_HISTORY.md`
- `../aspect_control/settings_and_control_surfaces_spec.md`
- `../subsystem_storage/storage_and_persistence_integrity_spec.md`

---

## 1. Purpose and Scope

This spec defines the canonical contract for the **History subsystem**.

It governs:

- traversal capture correctness
- timeline integrity
- archive fidelity
- preview and replay isolation
- temporal restoration semantics

---

## 2. Canonical Model

History is not just a tool pane.

It is the subsystem that owns temporal truth about:

- what traversal happened
- in what order
- what is active history versus dissolved/archive history
- how preview and replay interact with live state

---

## 3. Normative Core

### 3.1 Traversal Capture

- Traversal ordering must be correct and deterministic.
- Traversal append failures must be explicit.
- Edge association must be correct for recorded traversal.

### 3.2 Archive Integrity

- Dissolution must append to archive before removal from active state.
- Archive records are append-only.
- Export must not silently skip invalid data.

### 3.3 Preview and Replay Isolation

- Preview must not mutate live graph state.
- Preview must not write to WAL or persistence.
- Preview must not create or mutate live runtime instances.

### 3.4 Return to Present

- Exiting preview must restore live state deterministically.
- Cursor invalidation must fall back explicitly.

---

## 4. Planned Extensions

- explicit temporal replay controls
- richer timeline preview surfaces
- stronger timeline-to-workbench return-path behavior

---

## 5. Prospective Capabilities

- alternative time lenses over traversal history
- temporal diff and comparison workflows
- graph-space historical overlays

---

## 6. Acceptance Criteria

- Traversal and archive invariants are covered by tests or diagnostics.
- Preview isolation is explicit and enforced.
- Return-to-present semantics are deterministic.
- History surfaces remain observers of temporal truth, not ad hoc owners of unrelated app state.

