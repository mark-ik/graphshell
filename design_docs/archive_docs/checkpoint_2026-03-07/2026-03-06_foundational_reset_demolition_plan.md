# Foundational Reset Demolition Plan

**Date**: 2026-03-06
**Status**: Active demolition ledger
**Purpose**: Enumerate the prototype foundations that must be retired, bridged, or deleted to realize the foundational reset without leaving partial authority behind.

**Related**:
- `2026-03-06_foundational_reset_architecture_vision.md`
- `2026-03-06_foundational_reset_migration_governance.md`
- `2026-03-06_foundational_reset_implementation_plan.md`
- `2026-03-06_reducer_only_mutation_enforcement_plan.md`

---

## 1. Decision Summary

The reset will fail if it only adds new abstractions.

This plan identifies the existing foundations that must be dismantled or tightly bridged:

- monolithic app-state authority
- universal `GraphIntent` overload
- mixed durable/projection position semantics
- ambiguous pane/frame/zone membership models
- overloaded promotion semantics
- scalar semantic authority in places where plural semantics are canonical
- runtime convenience helpers that bypass canonical ownership

Execution note:

- the demolition items in this ledger are too coarse to execute directly
- each item should decompose into one or more CLATs
- ledger status should advance through finished CLATs, not through vague phase intent

---

## 2. Demolition Status Vocabulary

Each item carries one status:

- `active-legacy` — old model still live and authoritative in places
- `bridge-active` — a named bridge exists, but migration is still underway and the old path remains materially active
- `bridged` — a named bridge exists and the old path is tightly constrained; no broad expansion should remain
- `canonicalized` — new model is authoritative; old model may remain only as a local compatibility shell
- `deleted` — old model removed from active code/docs

---

## 3. Demolition Ledger

### D1. Monolithic app-state authority

**Status**: `active-legacy`

**Current reality**:

- `GraphBrowserApp` still acts as domain state, workbench state, runtime state, reducer, and side-effect coordinator simultaneously.

**Target**:

- explicit `DomainState`
- explicit `WorkbenchState`
- explicit `RuntimeState`

**Bridge allowed**:

- `GraphBrowserApp` may temporarily become a container over the three states

**Linked implementation phase**: `Phase B - Introduce explicit state structs`

**Demolition owner**: state-layer separation / system architecture

**Delete when**:

- fields are no longer semantically mixed without layer ownership
- new code refers to layer-specific state rather than using `GraphBrowserApp` as undifferentiated truth

**Required evidence**:

- state-layer doc updated
- field ownership map exists
- new layer-specific access paths used in migrated slices

### D2. Universal `GraphIntent` as semantic catch-all

**Status**: `bridge-active`

**Current reality**:

- graph, workbench, routing, and runtime policy changes are all pushed through one catch-all intent vocabulary

**Target**:

- `AppCommand`
- `AppPlan`
- `AppTransaction`
- `AppEffect`

**Bridge allowed**:

- `GraphIntent` may remain as an internal adapter layer during staged migration

**Linked implementation phase**: `Phase C - Command/planner boundary`

**Demolition owner**: command-planner migration / reducer boundary

**Current bridge locations**:

- Spec bridge locations:
  - `2026-02-21_lifecycle_intent_model.md`
  - `2026-02-22_registry_layer_plan.md`
  - `2026-03-05_cp4_p2p_sync_plan.md`
  - `coop_session_spec.md`
  - `register/action_registry_spec.md`
  - `register/SYSTEM_REGISTER.md`
- Code bridge locations:
  - `graph_app.rs` (`GraphIntent`, `GraphReducerIntent`, reducer entrypoints)
  - `shell/desktop/ui/gui.rs`
  - `shell/desktop/ui/gui_frame.rs`
  - `shell/desktop/ui/gui_orchestration.rs`
  - `render/mod.rs`

**Delete when**:

- new feature work uses the new command/planning model
- `GraphIntent` is no longer the canonical user/system intent surface

**Required evidence**:

- command/planner doc
- adapter boundary tests
- no new specs presenting `GraphIntent` as universal architecture truth

### D3. Mixed durable/projection position semantics

**Status**: `bridge-active`

**Current reality**:

- node position semantics have historically mixed authored durable layout with render/physics churn

**Target**:

- `committed_position`
- `projected_position`

**Bridge allowed**:

- temporary projection reads from legacy position consumers while callsites migrate

**Linked implementation phase**: `Reducer plan Stage G` plus foundational reset runtime/state normalization

**Demolition owner**: graph projection normalization / render boundary

**Delete when**:

- durable writes target committed position only
- runtime/physics churn targets projected position only
- old ambiguous position semantics no longer appear in active docs/callsites

**Required evidence**:

- split fields live in code
- docs updated
- banned-token or boundary checks prevent silent re-merging of semantics

### D4. Frame vs zone dual membership semantics

**Status**: `active-legacy`

**Current reality**:

- active specs still show competition between frame membership and zone-style models

**Target**:

- frame membership is the only canonical workbench membership truth
- any surviving zone concept is explicitly derived visualization/layout grouping

**Bridge allowed**:

- doc-only transitional references while terminology cleanup is landing

**Linked implementation phase**: `Spec authority cleanup`

**Demolition owner**: terminology cleanup / workbench semantics

**Delete when**:

- no active spec describes zone as peer authority to frame membership
- runtime/workbench code does not use zone semantics as durable membership truth

**Required evidence**:

- terminology cleanup across affected specs
- docs-parity guard for stale zone semantics if needed

### D5. Overloaded promotion semantics

**Status**: `active-legacy`

**Current reality**:

- promotion is used for graph enrollment in some places and structural pane movement in others

**Target**:

- `promotion` means graph-backed durable enrollment only
- structural movement gets separate terms such as `hoist`, `tile`, `dock`, `detach`

**Bridge allowed**:

- adapter comments or aliases while code/docs are renamed

**Linked implementation phase**: `Spec authority cleanup`

**Demolition owner**: terminology cleanup / pane-workbench semantics

**Delete when**:

- active specs no longer use promotion ambiguously
- command/model names separate graph enrollment from pane movement

**Required evidence**:

- terminology cleanup
- renamed command/model surfaces where touched

### D6. Scalar semantic authority

**Status**: `bridge-active`

**Current reality**:

- semantic UX/specs want plural class membership
- some code paths still expect scalar semantic authority or reduction-first semantics

**Target**:

- plural semantic truth is canonical
- scalar primary class is derived compatibility data only

**Bridge allowed**:

- `primary_class` / `primary_code` compatibility fields for scalar consumers

**Linked implementation phase**: `Spec authority cleanup` plus semantic-model follow-on extraction

**Demolition owner**: semantic model migration

**Delete when**:

- new semantic algorithms consume plural classes
- scalar-only storage is gone
- legacy scalar-only assumptions in docs/tests are removed

**Required evidence**:

- canonical semantic spec
- callsite inventory for scalar consumers
- progressive deletion of scalar-authority assumptions

### D7. Runtime mutation/helpers bypassing canonical authority

**Status**: `bridge-active`

**Current reality**:

- progress has already been made on reducer-owned durable mutation and undo boundaries
- remaining risk is helper creep and undocumented bypasses in runtime/lifecycle/orchestration code

**Target**:

- one canonical durable mutation path
- runtime code consumes effects, projections, and sanctioned builders only

**Bridge allowed**:

- tightly scoped adapters during migration

**Linked implementation phase**: `Phase E - Continue durable mutation normalization` and `Phase F - Runtime isolation`

**Demolition owner**: reducer/apply boundary hardening

**Delete when**:

- raw durable mutation helpers are private or gone
- trusted-writer tests cover the remaining boundary

**Required evidence**:

- boundary tests
- grep/banned-token enforcement
- replay path uses canonical apply engine

### D8. Mixed transaction models

**Status**: `active-legacy`

**Current reality**:

- undo snapshots, persistence log entries, replay semantics, and reducer execution are related but not one first-class transaction model

**Target**:

- `AppTransaction` as the canonical pure change unit

**Bridge allowed**:

- snapshot and replay adapters while transaction adoption is staged

**Linked implementation phase**: `Phase D - First-class transaction model`

**Demolition owner**: transaction-model migration

**Delete when**:

- undo/persistence/replay/diagnostics all consume the same transaction truth surface or explicit projections from it

**Required evidence**:

- transaction model spec
- undo/replay/persistence migration plan

---

## 4. Spec Demolition Checklist

The following active-spec changes are required by the reset:

1. Overview/system docs must stop presenting monolithic app state as canonical architecture.
2. Active specs must stop using `GraphIntent` as a universal architecture explanation once the command/planner model is introduced.
3. Frame/zone contradictions must be resolved in active docs.
4. Promotion must be narrowed to one meaning across active docs.
5. Semantic docs must treat plural class membership as canonical and scalar views as compatibility only.
6. History docs must continue treating traversal events as first-class and reject placeholder/dummy traversal semantics.

Completion rule:

- if an active doc still describes the retired model as current, the demolition item is not complete.

---

## 5. Codebase Demolition Checklist

The following code patterns are demolition targets:

1. Mixed state fields with no declared layer ownership.
2. Catch-all intent surfaces serving graph/workbench/runtime semantics interchangeably.
3. Direct durable mutation helpers reachable from non-canonical runtime code.
4. Scalar semantic storage used as primary truth.
5. Ambiguous position writes that do not distinguish committed vs projected semantics.
6. Workbench membership logic split across competing abstractions.

Completion rule:

- if reviewers can still ask "which of these is the real truth?", demolition is incomplete.

---

## 6. Unknown-Surface Demolition Protocol

Unknown or undocumented surfaces are expected in a prototype. They must be explicitly pulled into the demolition process.

Required protocol per slice:

1. Run repo-wide searches for retired APIs/terms.
2. Review adjacent helper/orchestration/test files.
3. Classify all hits.
4. Add newly found legacy foundations to this demolition ledger if they are not already listed.

If a new hidden authority center is discovered, it becomes a demolition item immediately rather than being deferred into "misc cleanup."

---

## 7. Deletion Evidence

An item can move to `deleted` only if:

1. Active docs no longer present it as current.
2. Canonical code path exists and is used.
3. Old path is removed from active runtime code.
4. Boundary checks prevent reintroduction.
5. Compile and targeted tests pass after deletion.

Deletion is architectural completion, not optional cleanup.
