# Foundational Reset Implementation Plan

**Date**: 2026-03-06
**Status**: Active implementation plan
**Purpose**: Define the concrete spec changes, codebase changes, verification strategy, and unknown-surface discovery work required to implement the foundational reset.

**Related**:
- `2026-03-06_foundational_reset_architecture_vision.md`
- `2026-03-06_foundational_reset_migration_governance.md`
- `2026-03-06_foundational_reset_demolition_plan.md`
- `2026-02-22_registry_layer_plan.md`
- `2026-03-06_reducer_only_mutation_enforcement_plan.md`
- `../../../testing/test_guide.md`

---

## 1. Decision Summary

The foundational reset should be implemented as a sequence of authority transfers, not as a big-bang rewrite.

Each phase must do four things:

1. change active specs
2. change canonical code paths
3. remove or bridge legacy paths
4. add enforcement against regression

---

## 2. Current-State Baseline

As of 2026-03-06:

- reducer-owned durable graph mutation has started landing
- graph-apply and `GraphDelta` exist for a meaningful subset of durable graph writes
- undo boundary ownership is moving into reducer/app-owned surfaces
- semantic plurality is defined in active spec
- history no longer uses dummy traversal records
- a foundational architecture vision now exists

What is still missing:

- explicit `DomainState` / `WorkbenchState` / `RuntimeState` structure in code
- a first-class transaction model
- a non-overloaded command/planner model
- a complete demolition of prototype-era duplicate authority

---

## 3. Workstreams

The reset should proceed as five linked workstreams:

1. Spec authority cleanup
2. State-layer separation
3. Command/transaction/effect architecture
4. Durable mutation normalization
5. Unknown-surface discovery and enforcement

These workstreams overlap, but each must retain its own closure criteria.

---

## 4. Spec Authority Cleanup

### 4.1 Active specs that need direct updates

The following active docs should be updated or reconciled as part of the reset:

- [system_architecture_spec.md](c:\Users\mark_\OneDrive\code\rust\graphshell\design_docs\graphshell_docs\implementation_strategy\system\system_architecture_spec.md)
- [register_layer_spec.md](c:\Users\mark_\OneDrive\code\rust\graphshell\design_docs\graphshell_docs\implementation_strategy\system\register_layer_spec.md)
- [2026-02-22_registry_layer_plan.md](c:\Users\mark_\OneDrive\code\rust\graphshell\design_docs\graphshell_docs\implementation_strategy\system\2026-02-22_registry_layer_plan.md)
- [2026-03-06_reducer_only_mutation_enforcement_plan.md](c:\Users\mark_\OneDrive\code\rust\graphshell\design_docs\graphshell_docs\implementation_strategy\system\2026-03-06_reducer_only_mutation_enforcement_plan.md)
- [edge_traversal_spec.md](c:\Users\mark_\OneDrive\code\rust\graphshell\design_docs\graphshell_docs\implementation_strategy\subsystem_history\edge_traversal_spec.md)
- [semantic_tagging_and_knowledge_spec.md](c:\Users\mark_\OneDrive\code\rust\graphshell\design_docs\graphshell_docs\implementation_strategy\canvas\semantic_tagging_and_knowledge_spec.md)

Likely additional active docs needing terminology cleanup:

- frame/workbench specs
- focus/navigation specs
- pane opening/routing specs
- layout/physics specs where committed vs projected position semantics matter

Important overlap:

- [2026-02-22_registry_layer_plan.md](c:\Users\mark_\OneDrive\code\rust\graphshell\design_docs\graphshell_docs\implementation_strategy\system\2026-02-22_registry_layer_plan.md) already proposes splitting `GraphBrowserApp` and introducing cleaner registry boundaries.
- The foundational reset must reconcile with that plan rather than create a competing state-split story.

### 4.2 Required spec changes

#### S1. State model

Active system docs should explicitly describe:

- `DomainState`
- `WorkbenchState`
- `RuntimeState`

They should stop presenting a monolithic app blob as the ideal architecture.

#### S2. Intent model

Active architecture docs should stop presenting `GraphIntent` as the universal future-facing intent model.

They should describe the target pipeline:

- `AppCommand`
- `AppPlan`
- `AppTransaction`
- `AppEffect`

#### S3. Position model

Active docs must consistently distinguish:

- committed/authored position
- projected/runtime position

#### S4. Membership semantics

Active docs must resolve frame vs zone semantics:

- frame membership is canonical
- any zone concept is derived, not peer authority

#### S5. Promotion terminology

Active docs must narrow `promotion` to graph-backed enrollment only.

#### S6. Semantic plurality

Active docs must treat plural semantic membership as canonical and scalar reduction as compatibility only.

### 4.3 Spec-enforcement additions

Add or extend docs-parity checks for:

- banned stale promotion usages
- stale frame/zone peer-authority wording
- stale scalar-semantic authority wording
- stale monolithic-state wording once the new model is adopted

---

## 5. Codebase Change Plan

### 5.1 Phase A - State ownership map

Before large code movement, create an ownership map of current `GraphBrowserApp` fields:

- domain-owned
- workbench-owned
- runtime-owned
- unknown/unclassified

Primary file:

- [graph_app.rs](c:\Users\mark_\OneDrive\code\rust\graphshell\graph_app.rs)

Likely adjacent files:

- runtime/lifecycle modules
- workbench/tile modules
- persistence modules

Phase A receipt:

- [2026-03-06_foundational_reset_graphbrowserapp_field_ownership_map.md](c:\Users\mark_\OneDrive\code\rust\graphshell\design_docs\graphshell_docs\implementation_strategy\system\2026-03-06_foundational_reset_graphbrowserapp_field_ownership_map.md)

Current known unknowns from that map:

- mixed `views` container
- legacy global `camera`
- mixed `undo_stack` / `redo_stack`
- runtime-keyed `semantic_tags`

Closure condition:

- every mutable field is classified
- unknown fields are explicitly called out for follow-up

### 5.2 Phase B - Introduce explicit state structs

Introduce:

- `DomainState`
- `WorkbenchState`
- `RuntimeState`

Initially, `GraphBrowserApp` may contain them directly.

Goal:

- shift from semantic mixing to named ownership, even before deeper reducer changes

Closure condition:

- migrated fields live in explicit layer structs
- new code stops adding unowned top-level fields to the monolith

### 5.3 Phase C - Command/planner boundary

Introduce a top-level command/planning model without requiring a one-shot rewrite.

Suggested sequence:

1. Add `AppCommand` types for one narrow domain first.
2. Add planner output for route-open and pane-target decisions.
3. Use adapters from `GraphIntent` while migrating callsites.

Best initial domain:

- open/routing/pane-target policy

because it currently mixes graph/workbench/runtime semantics heavily.

Closure condition:

- at least one subsystem no longer treats `GraphIntent` as the top-level architecture truth

### 5.4 Phase D - First-class transaction model

Introduce `AppTransaction` as the preferred pure change unit.

Suggested first uses:

- undo/redo
- persistence append model
- history preview/replay summaries

Closure condition:

- at least one of undo or persistence uses transaction truth directly rather than ad hoc mixed snapshot semantics

### 5.5 Phase E - Continue durable mutation normalization

Continue current `GraphDelta` work:

- complete durable node metadata deltas
- normalize any durable edge metadata writes
- route replay/recovery through canonical apply paths

Likely files:

- [apply.rs](c:\Users\mark_\OneDrive\code\rust\graphshell\model\graph\apply.rs)
- [mod.rs](c:\Users\mark_\OneDrive\code\rust\graphshell\model\graph\mod.rs)
- [graph_app.rs](c:\Users\mark_\OneDrive\code\rust\graphshell\graph_app.rs)
- [mod.rs](c:\Users\mark_\OneDrive\code\rust\graphshell\services\persistence\mod.rs)

Closure condition:

- durable graph writes are overwhelmingly canonicalized
- remaining exceptions are explicitly documented and bounded

### 5.6 Phase F - Runtime isolation

Restrict runtime modules to:

- effect consumption
- projection updates
- runtime cache management

They should stop acting as semantic authority.

Likely files:

- lifecycle controllers
- GUI orchestration
- render/workbench runtime adapters

Closure condition:

- runtime modules no longer perform undocumented semantic or durable-state decisions

---

## 6. Unknown/Undocumented Codebase Accounting

This is mandatory work, not optional cleanup.

### 6.1 Repo-wide discovery passes

For each phase, run searches across:

- `graph_app.rs`
- `model/**`
- `services/**`
- `shell/**`
- `render/**`
- `scripts/**`
- `design_docs/**`

### 6.2 Search categories

Search for:

- retired function names
- retired field names
- retired terminology
- direct mutator access
- comments describing stale authority
- tests normalizing legacy assumptions

### 6.3 Required hidden-surface review zones

Always inspect these even if not mentioned in the main spec:

- test-only builders
- lifecycle adapters
- UI orchestration modules
- persistence recovery paths
- bootstrapping scripts
- diagnostics glue

### 6.4 Unknown-surface ledger

Each newly discovered surface must be recorded in the current implementation slice as one of:

- migrated
- bridged
- deferred with owner
- false-positive

No silent findings.

---

## 7. Enforcement Plan

### 7.1 Code enforcement

Use:

- visibility tightening
- boundary contract tests
- banned-token searches in tests
- compile verification
- targeted regression tests

### 7.2 Docs enforcement

Use:

- docs-parity scripts
- banned-term checks for active docs
- "canonical doc vs summary doc" discipline

### 7.3 Migration-specific enforcement to add

Recommended future checks:

1. ban new top-level mutable fields in `GraphBrowserApp` without ownership classification
2. ban retired terms in active docs once replaced
3. ban direct use of retired helpers after each demolition slice
4. add a state-ownership receipt or test fixture proving layer classification remains current

---

## 8. Sequencing

Recommended order:

1. land governance + demolition + implementation docs
2. create state ownership map
3. add explicit state-layer structs
4. continue `GraphDelta` normalization for durable writes
5. migrate one subsystem to `AppCommand` / planner semantics
6. introduce first-class transaction model
7. expand enforcement and delete bridges aggressively

Important rule:

- do not start multiple foundational authority migrations at once unless one is already frozen by spec and tests

---

## 9. Verification Strategy

Every major slice should provide:

1. symbol-search evidence
2. compile evidence
3. targeted test evidence
4. full-suite evidence where practical
5. docs-parity evidence

For this repo, the validated Windows path is:

- MozillaBuild-backed Cargo
- `MOZILLABUILD=C:\mozilla-build`
- `MOZTOOLS_PATH=C:\mozilla-build`
- `CARGO_TARGET_DIR=C:\t\graphshell-target`

Primary commands:

- `cargo check -p graphshell --all-targets`
- `cargo test -p graphshell --lib`

Targeted commands vary by slice.

---

## 10. Immediate Next Execution Steps

The best next implementation slice is:

1. produce a `GraphBrowserApp` field ownership map
2. introduce explicit `DomainState` / `WorkbenchState` / `RuntimeState` containers
3. update active system docs to reference those containers
4. add enforcement preventing new unowned state from accreting back into the monolith

That is the most leverage-positive step because it affects every later migration and reduces ambiguity before deeper API changes land.
