# CLAT: DomainState Core Extraction

**Date**: 2026-03-06
**Status**: Complete (archived receipt candidate)
**Pattern**: Component-Local Authority Transfer (CLAT)
**Purpose**: Perform the first narrow state-layer authority transfer by extracting the durable domain core from `GraphWorkspace`.

**Related**:
- `2026-03-06_foundational_reset_architecture_vision.md`
- `2026-03-06_foundational_reset_migration_governance.md`
- `2026-03-06_foundational_reset_demolition_plan.md`
- `2026-03-06_foundational_reset_implementation_plan.md`
- `2026-03-06_foundational_reset_graphbrowserapp_field_ownership_map.md`

---

## 1. Authority Transfer

This CLAT transfers exactly one boundary:

- from: durable domain fields embedded directly in `GraphWorkspace`
- to: explicit `DomainState`

Scope of this slice:

- `graph`
- `notes`
- `next_placeholder_id`

Out of scope:

- workbench state extraction
- runtime state extraction
- `views`
- global `camera`
- `undo_stack` / `redo_stack`
- `semantic_tags`

---

## 2. Discovery Receipt

Discovery classes run for this CLAT:

1. `graph_app.rs` field ownership and constructor paths
2. repo-wide `workspace.graph` / `workspace.notes` / `workspace.next_placeholder_id` search
3. active reset/system docs touching state-container execution

Current classification:

- `graph_app.rs`: `canonical`
- other runtime/test modules reading `workspace.graph`: `bridge`
- reset docs still describing phase-scale migration instead of CLAT execution: `stale-doc`

---

## 3. Minimal Code Change

This slice introduces:

- `DomainState`
- `GraphWorkspace { domain: DomainState, ... }`

Current bridge:

- `impl Deref<Target = DomainState> for GraphWorkspace`
- `impl DerefMut for GraphWorkspace`

Reason for bridge:

- it keeps repo-wide `workspace.graph` callsites buildable while the first authority container lands
- it prevents this first CLAT from ballooning into a repo-wide state-path rewrite

This bridge is temporary debt, not the desired end state.

---

## 4. Regression Guard

For this CLAT, the hard guard is structural:

- `GraphWorkspace` no longer owns `graph`, `notes`, or `next_placeholder_id` as direct fields
- constructors must initialize those values through `DomainState`

Follow-on enforcement for later CLATs:

- ban new direct durable-domain fields on `GraphWorkspace`
- remove the `Deref` bridge once subsystem-local state-path CLATs have migrated callsites

---

## 5. Canonical Doc Change

This CLAT updates the foundational reset package to treat execution as CLAT-driven rather than phase-driven.

Canonical authority doc for this slice:

- `2026-03-06_foundational_reset_implementation_plan.md`

Supporting receipt:

- `2026-03-06_foundational_reset_graphbrowserapp_field_ownership_map.md`

---

## 6. Completion Criteria

This CLAT is complete when:

1. `DomainState` exists in code.
2. `GraphWorkspace` stores the durable core in `domain: DomainState`.
3. constructors and load/reset paths initialize the durable core through `DomainState`.
4. the bridge path is explicitly documented.
5. compile + targeted tests pass.

Completion note (2026-03-07):

- `DomainState` exists in code and `GraphWorkspace` stores the durable core at `domain: DomainState`
- constructors and load/reset paths initialize the durable core through `DomainState`
- the temporary deref bridge is explicitly documented
- a bounded workbench follow-on migration off `workspace.graph` is complete and guarded by the trusted-writer contract test
- focused validation passed with `cargo test -q contract_only_trusted_writers_call_graph_topology_mutators -- --nocapture` and `cargo test -q tile_behavior -- --nocapture`

This CLAT is not complete when:

- the `Deref` bridge is removed
- repo-wide `workspace.graph` callsites are fully migrated

That is a later CLAT series, not part of this first extraction.
