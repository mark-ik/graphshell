# Reducer-Only Mutation Enforcement Plan

**Date**: 2026-03-06
**Status**: Active planning slice
**Purpose**: Define the migration from the current trusted-writer boundary to compiler-enforced reducer-only graph mutation for live runtime and replay/recovery.

**Related**:
- `system_architecture_spec.md`
- `register_layer_spec.md`
- `../../subsystem_storage/storage_and_persistence_integrity_spec.md`
- `../../subsystem_history/edge_traversal_spec.md`
- `../../../TERMINOLOGY.md`

---

## 1. Problem Framing

Graphshell currently documents a single-write-path goal, but implementation still allows multiple trusted internal writers:

- reducer-driven runtime mutation (`GraphBrowserApp`)
- persistence replay/recovery mutation (`services/persistence`)

This is acceptable as an intermediate model, but it is not reducer-only compiler enforcement.

---

## 2. Current vs Target Boundary

### 2.1 Current boundary (trusted writers)

- Graph mutators are `pub(crate)`.
- Runtime/shell code is contract-tested to avoid direct topology mutation.
- Persistence replay/recovery may mutate graph directly as a trusted reconstruction writer.

### 2.2 Target boundary (reducer-only enforcement)

- Non-reducer modules cannot compile if they attempt graph mutation.
- Live runtime and replay/recovery both flow through the same reducer mutation application path.
- Persistence replays canonical state deltas, not ad hoc direct graph writes.

---

## 3. Why Pursue Reducer-Only Enforcement

Benefits:

1. One canonical mutation engine for runtime + replay.
2. Stronger determinism between live behavior and replay behavior.
3. Clearer ownership and simpler code audit posture.
4. Fewer out-of-band state mutation bugs.

Costs:

1. Non-trivial refactor of reducer/persistence seams.
2. Need to separate pure state mutation from side effects.
3. Temporary migration complexity while dual paths exist.

Decision guidance:

- If deterministic history/replay and strict invariants are core goals, reducer-only enforcement is recommended.
- If near-term delivery pressure dominates, trusted-writer model may remain temporarily with explicit boundaries and tests.

---

## 4. Canonical Migration Strategy

### Stage A - Introduce explicit state deltas

Define a pure mutation enum (for example `GraphMutation`) that represents graph-state deltas only.

Rules:

- No side effects in `GraphMutation`.
- Sufficiently expressive to represent current reducer graph writes and replay requirements.

### Stage B - Split planning vs apply

Reducer path changes from direct graph writes to:

1. plan: `GraphIntent -> Vec<GraphMutation>`
2. apply: `apply_graph_mutations(graph, mutations)`

Rules:

- `apply_graph_mutations` is the only mutator entry point.
- Side effects are emitted as separate effect records.

### Stage C - Route replay through the same apply path

Persistence replay should decode to `GraphMutation` (or equivalent canonical deltas) and invoke the same apply function used by live reducer execution.

Rules:

- Replay does not call topology mutators directly.
- Replay may suppress side-effect execution while still using the same state mutation path.

### Stage D - Enforce visibility/token boundary

Restrict direct graph mutators so only reducer apply code can call them.

Implementation options:

- module-private mutators (`pub(super)` + reducer module ownership)
- reducer-only capability token required by mutator APIs
- optional crate split for stronger compile boundaries

### Stage E - Remove trusted-writer exceptions

After replay uses reducer apply path and compile boundary is in place:

- remove trusted-writer exception wording
- update boundary docs to reducer-only enforcement language
- keep contract tests for shell/runtime no-direct-mutation guarantees

---

## 5. Side-Effect Isolation Contract

Reducer-only enforcement requires strict separation:

- state mutation path: pure, deterministic, replayable
- effect path: diagnostics, runtime orchestration, UI notifications, async operations

Replay mode behavior:

- apply state mutations
- suppress or adapt effect execution
- preserve deterministic state reconstruction

---

## 6. Acceptance Criteria

1. Non-reducer modules cannot compile when calling graph topology mutators.
2. Persistence replay/recovery no longer calls graph mutators directly.
3. Runtime reducer and replay both use one mutation application path.
4. Side effects are not embedded in pure graph mutation application.
5. Existing mutation-boundary contract tests remain green and are updated for the new boundary language.
6. Replay of persisted logs produces graph state equivalent to live-constructed state for the same mutation sequence.

---

## 7. Interim Policy (Until Full Migration)

Until Stage E completes, the trusted-writer model remains valid with explicit constraints:

- reducer runtime path is primary writer
- persistence replay/recovery is sanctioned reconstruction writer
- runtime/shell layers outside reducer and persistence must not call graph mutators directly

This interim policy must be documented as a temporary architecture state, not a final enforcement model.
