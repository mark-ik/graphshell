# Foundational Reset Implementation Plan

**Date**: 2026-03-06
**Status**: Active implementation plan (Phase B CLAT-1 landed; follow-on bridge reduction in progress)
**Purpose**: Define the concrete spec changes, codebase changes, verification strategy, and unknown-surface discovery work required to implement the foundational reset.

**Related**:
- `system_architecture_spec.md`
- `2026-03-06_foundational_reset_graphbrowserapp_field_ownership_map.md`
- `2026-02-22_registry_layer_plan.md`
- `2026-03-06_reducer_only_mutation_enforcement_plan.md`
- `../../../testing/test_guide.md`

---

## 1. Decision Summary

The foundational reset should be implemented as a sequence of authority transfers, not as a big-bang rewrite.

Execution unit:

- one Component-Local Authority Transfer (CLAT) at a time

Each CLAT must do four things:

1. change active specs
2. change canonical code paths
3. remove or bridge legacy paths
4. add enforcement against regression

The phase/workstream structure in this document is umbrella sequencing only. It should not be treated as permission to execute D1-D8 as giant parallel migrations.

---

## 2. Current-State Baseline

As of 2026-03-07:

- reducer-owned durable graph mutation has started landing
- graph-apply and `GraphDelta` exist for a meaningful subset of durable graph writes
- undo boundary ownership is moving into reducer/app-owned surfaces
- semantic plurality is defined in active spec
- history no longer uses dummy traversal records
- a precursor structural split already exists in active planning/docs: `GraphWorkspace` / `AppServices`
- `DomainState { graph, notes, next_placeholder_id }` is now extracted in code
- `GraphWorkspace { domain: DomainState, ... }` is now the active durable-core storage shape
- the first bounded bridge-reduction follow-on slice is landed for workbench graph reads, with a regression guard in the contract test

What is still missing:

- explicit `WorkbenchState` / `RuntimeState` structure in code
- a first-class transaction model
- a non-overloaded command/planner model
- a complete demolition of prototype-era duplicate authority

### 2.1 Code-Truth Assessment (2026-03-07)

At time of writing, `graph_app.rs` is still a ~13K-line monolith (`13,194` lines on 2026-03-07).
It currently contains, in one file:

- top-level types and state carriers
- the reducer and `GraphIntent`
- undo/redo logic
- persistence coordination
- webview lifecycle coordination
- history preview execution state
- workspace management
- dozens of `pending_*` staging fields

That is the real architecture of the app right now.

The active docs are useful only if they are treated as a way to drive code changes against that reality. They should not be mistaken for architectural closure by themselves.

The highest-leverage contradictions still shaping the product are below.

#### C1. `pending_*` staging is a manual command queue

`graph_app.rs` currently contains hundreds of `pending_` references and roughly dozens of distinct `pending_*` fields. In practice, those fields behave like a hand-managed command queue:

- a field is set in one place
- consumed in another place
- cleared in a third place

That gives the app queue-like failure modes:

- ordering ambiguity
- forgotten clears
- stale frame-to-frame state
- poor inspectability

Short-horizon implementation direction:

- collapse `pending_*` staging into a single explicit queue such as `Vec<AppCommand>`
- use one drain point per frame
- make ordering explicit in code before pursuing a fuller `AppPlan` / `AppTransaction` model

This is the single highest-leverage near-term simplification because it deletes bespoke staging surfaces while making frame behavior inspectable.

#### C2. `GraphIntent` is overloaded across incompatible categories

`GraphIntent` currently mixes several fundamentally different kinds of things:

- durable graph mutations
- workbench/view actions
- runtime notifications
- camera/input requests

The code already acknowledges this overload. `apply_workspace_only_intent` exists because not all `GraphIntent` variants are actually graph mutations.

Short-horizon implementation direction:

- split the current surface into separate enums with honest names, for example:
  - `GraphMutation`
  - `ViewAction`
  - `RuntimeEvent`
- allow the reducer/app boundary to accept all three during migration
- stop presenting runtime notifications and graph mutations as the same semantic kind

This does not require a full planner architecture first. It is a type-truth cleanup that reduces semantic lying in the current reducer surface.

#### C3. `DomainState` is real but still trapped inside the monolith

The extracted durable core is a real improvement:

- `Graph`
- notes
- placeholder identity support
- `GraphDelta` / apply support for part of the mutation surface

But the durable core still lives inside the same monolithic file, and `GraphBrowserApp` remains able to reach broadly across the durable model.

Short-horizon implementation direction:

- move `DomainState` into its own module
- define the domain-owned mutation subset alongside it
- keep visibility restricted so the parent module owns boundary crossings intentionally

This should be done as an ownership-enforcement move, not as a crate split.

#### C4. Undo still snapshots world-state instead of recording applied change

Current undo/redo state captures full snapshots of graph and related workbench state. That approach is expensive, hard to replay, and structurally separate from the delta-based mutation path already emerging elsewhere.

Short-horizon implementation direction:

- evolve undo to record applied graph deltas plus the needed workbench diff
- add inverse delta support for the topology/durable subset
- keep the snapshot fallback temporarily while the delta path hardens

This would bring undo and durable mutation closer to one shared data model without requiring a full event-sourcing rewrite.

#### C5. The `GraphWorkspace -> DomainState` deref bridge normalizes the boundary violation the reset is trying to remove

The temporary deref bridge means legacy code can still write `workspace.graph` even though the durable core now lives at `workspace.domain.graph`.

That makes the extraction materially incomplete until the bridge is deleted.

Short-horizon implementation direction:

- delete `Deref` / `DerefMut` on `GraphWorkspace`
- fix callsites mechanically
- use contract tests to prevent reintroduction

This is the enforcement step that turns CLAT-1 from structural relocation into a real ownership boundary.

#### C6. Documentation breadth is ahead of implemented architecture

The repository has a large amount of active system-level planning and governance material relative to the amount of code that has actually been decomposed.

The main risk is not that those docs are low quality. The risk is that they can create the feeling of architectural progress while the executable architecture remains concentrated in a single file.

Operational consequence for this plan:

- prioritize code diffs over new system-governance prose until the current monolith is materially reduced
- treat future architectural documentation as a receipt for landed code or an enabler for the next concrete CLAT, not as a substitute for code movement

### 2.2 Immediate Code-First Sequence

If reset work must choose between additional architecture prose and near-term code truth, the preferred order is:

1. delete `GraphWorkspace` `Deref` / `DerefMut` and mechanically fix remaining callsites
2. split `GraphIntent` into honest categories (`GraphMutation`, `ViewAction`, `RuntimeEvent`) without waiting for a full planner stack
3. replace the highest-volume `pending_*` staging fields with a single explicit command queue and one drain point
4. move `DomainState` into its own module with restricted visibility
5. move undo toward delta-based recording using the existing `GraphDelta` path, with snapshot fallback retained initially

These are preferred because they are concrete diffs that make the code more honest about what it already does.

Execution rule for this sequence:

- do not write new system-level governance docs in place of these code changes
- each step should land as bounded code movement with regression enforcement

---

## 3. Workstreams

The reset should proceed as five linked workstreams:

1. Spec authority cleanup
2. State-layer separation
3. Command/transaction/effect architecture
4. Durable mutation normalization
5. Unknown-surface discovery and enforcement

These workstreams overlap, but each must retain its own closure criteria.

Operational rule:

- workstreams are backlog buckets
- CLATs are the actual execution units

---

## 4. Spec Authority Cleanup

### 4.1 Active specs that need direct updates

The following active docs should be updated or reconciled as part of the reset:

- `system_architecture_spec.md`
- `register_layer_spec.md`
- `2026-02-22_registry_layer_plan.md`
- `2026-03-06_reducer_only_mutation_enforcement_plan.md`
- `2026-03-03_graphshell_address_scheme_implementation_plan.md`
- `../../../TERMINOLOGY.md`
- `../subsystem_history/edge_traversal_spec.md`
- `../canvas/semantic_tagging_and_knowledge_spec.md`

Likely additional active docs needing terminology cleanup:

- frame/workbench specs
- focus/navigation specs
- pane opening/routing specs
- layout/physics specs where committed vs projected position semantics matter

Important overlap:

- `2026-02-22_registry_layer_plan.md` already proposes splitting `GraphBrowserApp` and introducing cleaner registry boundaries.
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

Before Phase C starts in code, a dedicated planner spec should exist for:

- what `AppPlan` contains
- how planner output differs from register/runtime routing
- how `AppPlan` relates to reducer-owned transactions and post-apply effects
- which first feature lane will use it

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

- `graph_app.rs`

Likely adjacent files:

- runtime/lifecycle modules
- workbench/tile modules
- persistence modules

Phase A receipt:

- `2026-03-06_foundational_reset_graphbrowserapp_field_ownership_map.md`

Current known unknowns from that map:

- mixed `views` container
- legacy global `camera`
- mixed `undo_stack` / `redo_stack`
- runtime-keyed `semantic_tags`

Closure condition:

- every mutable field is classified
- unknown fields are explicitly called out for follow-up

Reconciliation rule:

- this ownership map refines the earlier `GraphWorkspace` / `AppServices` split
- it does not replace that split with a second incompatible story

### 5.2 Phase B - Introduce explicit state structs

Introduce:

- `DomainState`
- `WorkbenchState`
- `RuntimeState`

Initially, `GraphBrowserApp` may contain them directly.

Goal:

- shift from semantic mixing to named ownership, even before deeper reducer changes

Compatibility mapping to current active docs:

- `GraphWorkspace` should evolve toward `DomainState + WorkbenchState`
- `AppServices` should remain the runtime/service side of the boundary
- any runtime residue still sitting in `GraphWorkspace` becomes an explicit migration target rather than a second architecture model

Closure condition:

- migrated fields live in explicit layer structs
- new code stops adding unowned top-level fields to the monolith

Initial CLAT sequence for Phase B:

1. extract `DomainState { graph, notes, next_placeholder_id }`
2. extract workbench selection/view state
3. extract runtime webview/render/cache state
4. resolve `views`
5. resolve global `camera`

Current execution note (2026-03-07):

- CLAT-1 is complete for `DomainState { graph, notes, next_placeholder_id }`
- `GraphWorkspace` now stores the durable core at `domain: DomainState`
- the workbench consumer family has been migrated from `workspace.graph` to `workspace.domain.graph` for its bounded graph-read surface
- the trusted-writer contract test now includes a workbench-specific guard preventing reintroduction of `workspace.graph` in that migrated family
- focused validation passed with `cargo test -q contract_only_trusted_writers_call_graph_topology_mutators -- --nocapture` and `cargo test -q tile_behavior -- --nocapture`

### 5.3 Phase C - Command/planner boundary

Introduce a top-level command/planning model without requiring a one-shot rewrite.

Suggested sequence:

1. Add `AppCommand` types for one narrow domain first.
2. Add planner output for route-open and pane-target decisions.
3. Use adapters from `GraphIntent` while migrating callsites.

Best initial domain:

- open/routing/pane-target policy

because it currently mixes graph/workbench/runtime semantics heavily.

Authoring prerequisite:

- write a dedicated `AppPlan` spec before starting Phase C extraction so planner semantics do not remain an underspecified paragraph in the reset vision

Closure condition:

- at least one subsystem no longer treats `GraphIntent` as the top-level architecture truth

Bridge note:

- active system docs still use `GraphIntent` as the current carrier in several places
- during migration, `GraphIntent` should be treated as the active bridge carrier rather than incorrectly described as already retired

Named bridge ledger for the current `GraphIntent` bridge:

- Spec bridge locations:
  - `2026-02-21_lifecycle_intent_model.md`
  - `2026-02-22_registry_layer_plan.md`
  - `2026-03-05_cp4_p2p_sync_plan.md`
  - `coop_session_spec.md`
  - `register/action_registry_spec.md`
  - `register/SYSTEM_REGISTER.md`
- Code bridge locations:
  - `graph_app.rs`
  - `shell/desktop/ui/gui.rs`
  - `shell/desktop/ui/gui_frame.rs`
  - `shell/desktop/ui/gui_orchestration.rs`
  - `render/mod.rs`
- Removal condition:
  - at least one live feature lane enters through `AppCommand` / planner-first APIs instead of direct `GraphIntent` construction
  - no active system doc presents `GraphIntent` as the future-facing top-level architecture
  - boundary tests and repo searches prevent new direct bridge expansion without explicit classification

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

- `model/graph/apply.rs`
- `model/graph/mod.rs`
- `graph_app.rs`
- `services/persistence/mod.rs`

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

> **Execution note (2026-03-07):** Steps 1–4 below are complete. CLAT-1 has landed (`DomainState` extracted, workbench consumer family migrated, contract test guard in place). Current next execution steps are in §5.2 (Phase B follow-on CLATs) and §5.3 (Phase C prerequisites). This section is retained as a historical record of the sequencing rationale.

The best next implementation slice was:

1. treat the ownership map as a backlog of CLATs, not as one giant migration ticket
2. land `DomainState { graph, notes, next_placeholder_id }` as the first state-layer CLAT
3. update active system docs to reference CLAT execution where needed
4. add enforcement preventing new unowned durable-domain fields from accreting back into `GraphWorkspace`

That is the most leverage-positive next step because it proves the CLAT pattern on a narrow authority transfer before deeper API changes land.
