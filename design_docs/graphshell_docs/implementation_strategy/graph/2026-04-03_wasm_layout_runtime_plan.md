<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# WASM Layout Runtime Plan (2026-04-03)

**Status**: Active follow-on plan
**Scope**: Extracts the runtime-loadable layout lane from `2026-02-24_physics_engine_extensibility_plan.md` into a bounded execution plan for sandboxed WASM layouts, host-side dispatch, failure handling, and snapshot-safe fallback behavior.
**Related**:

- `2026-02-24_physics_engine_extensibility_plan.md`
- `2026-04-03_layout_backend_state_ownership_plan.md`
- `layout_algorithm_portfolio_spec.md`
- `multi_view_pane_spec.md`
- `../aspect_render/render_backend_contract_spec.md`
- `../system/2026-04-23_execution_isolation_and_worker_runtime_plan.md`

---

## Context

Graphshell already has the first half of the extensibility seam:

- built-in layout dispatch through `ActiveLayout`
- a concrete persisted wrapper in `ActiveLayoutState`
- registry/runtime plumbing that can resolve active per-view policy
- `ModType::Wasm` scaffolding on the mod side

What is still missing is the runtime-loaded layout path itself:

- no `WasmLayoutAdapter`
- no stable guest ABI
- no runtime watchdog/fallback policy
- no snapshot-safe handling for layouts that disappear between sessions

The new system-level execution-isolation plan also makes one constraint explicit:
today's in-process Extism bring-up shape is **transitional**, not the steady
state substrate for runtime-loaded layouts. This plan therefore needs to target
a worker-safe guest-runtime contract rather than silently treating direct
in-process plugin calls as the end-state.

This plan exists so the WASM lane stops living as speculative prose inside the umbrella physics note.

---

## Non-Goals

- replacing the current built-in native layout dispatcher
- redesigning scene-physics or the `parry2d` scene-enrichment lane
- taking full ownership of the upstream `Layout<S>` / `LayoutState` trait seam in the first slice
- assuming full free-camera 3D or a new render backend as a prerequisite

---

## Feature Target 1: Define the Stable Host/Guest ABI

### Target 1 Context

The umbrella note already established the architectural shape: a native host adapter implements the layout trait and delegates layout computation to a loaded WASM guest. That only becomes shippable once the request/result contract is explicit and versioned.

### Target 1 Tasks

1. Define a versioned `LayoutRequest` / `LayoutResult` schema for guest calls.
2. Require deterministic node and edge ordering before serialization so identical graph state yields identical guest input.
3. Keep guest authority narrow: compute positions and guest-owned opaque state, but do not mutate graph truth.
4. Version the ABI through a manifest capability string such as `layout-wasm-api:1`.
5. Separate stable serialized layout IDs from Rust enum variant names.

### Target 1 Validation Tests

- A sample guest can roundtrip a minimal graph through the ABI without host-side schema patching.
- Unknown ABI versions are rejected cleanly before a layout step runs.
- The same graph state serializes to the same guest payload ordering across runs.

---

## Feature Target 2: Land the Host Adapter and Runtime Wiring

### Target 2 Context

WASM layouts cannot implement Rust traits directly. The production seam must therefore be a host-owned adapter that translates between Graphshell runtime state and the guest ABI.

### Target 2 Tasks

1. Add a host-side `WasmLayoutAdapter` module that owns plugin lifetime, guest state bytes, and the layout-step call.
2. Back that adapter with the worker-safe `GuestRuntime` / `GuestSessionHandle` seam from `2026-04-23_execution_isolation_and_worker_runtime_plan.md` rather than direct host calls into a global plugin map.
3. Keep the caller-facing request/result semantics stable across native bring-up and browser-worker realizations even if the backend differs.
4. Route runtime-loaded layout resolution through the existing registry/runtime flow rather than a one-off loader path.
5. Ensure the render layer still sees one concrete view-owned layout state wrapper.
6. Keep built-in native variants and WASM variants selectable through the same user-facing layout selection surface.
7. Coordinate the persisted state shape with `2026-04-03_layout_backend_state_ownership_plan.md` instead of inventing a parallel carrier.

### Target 2 Validation Tests

- A WASM layout can be selected, stepped, and restored for one graph view without affecting other views.
- Unloading and reloading the same plugin preserves host stability and deterministic fallback behavior.
- Runtime-selected WASM layouts appear in the same resolved-layout diagnostics path as built-in layouts.
- The same host-side adapter contract works with a native worker-backed backend and a browser `DedicatedWorker` backend.

---

## Feature Target 3: Define Failure Handling and Degradation

### Target 3 Context

The source note called out the open risk directly: the happy path is specified, but malformed output, timeouts, guest panics, and missing plugins are not.

### Target 3 Tasks

1. Validate guest output for NaN, infinity, missing nodes, duplicate nodes, and grossly invalid coordinates.
2. Define a watchdog timeout and a deterministic fallback layout when a guest hangs or panics.
3. Surface per-pane failure state through diagnostics and a visible status indicator rather than silently swapping layouts.
4. Treat worker bootstrap failure, handshake failure, and unsupported-host realizations as explicit degraded states, not as silent fallback.
5. Treat missing or incompatible plugins during snapshot restore as degraded-but-loadable state, not as a fatal error.
6. Record enough diagnostic detail to distinguish load failure, ABI mismatch, validation failure, worker/bootstrap failure, and runtime timeout.

### Target 3 Validation Tests

- A malformed guest result falls back without crashing the host and emits a diagnosable failure reason.
- A timeout or panic reverts to the configured native fallback layout.
- Restoring a snapshot that references a missing WASM layout preserves the workspace and reports the missing layout ID.

---

## Feature Target 4: Bound the Operational Envelope

### Target 4 Context

Runtime-loaded layouts are valuable only if Graphshell defines what environments and graph sizes they are expected to run on.

### Target 4 Tasks

1. Define platform posture for desktop, mobile, and wasm-hosted builds instead of assuming every target supports guest execution equally.
2. Make the preferred backend explicit per host envelope:
   - desktop native: worker-backed guest runtime behind the shared message contract
   - wasm-hosted / browser: `DedicatedWorker`-backed guest runtime or explicit unsupported mode
   - mobile: selectively enabled only where lifecycle/capability constraints are acceptable
3. Set an initial node-count and frame-budget envelope for when WASM layouts remain enabled versus auto-downgrade.
4. Document capability restrictions for layout guests so they do not inherit broader mod privileges than they need.
5. Define hot-swap expectations: when plugin reload is allowed, when state is discarded, and when the host must restart the active layout.
6. Keep the first shipped guest contract compute-focused; broader scene or rendering authority is out of scope.

### Target 4 Validation Tests

- Platform gating is explicit and testable rather than hidden in ad hoc runtime checks.
- Oversized graphs degrade according to a documented policy.
- A plugin reload either preserves state safely or falls back deterministically with clear user feedback.

---

## Exit Condition

This plan is complete when Graphshell can select a sandboxed WASM layout through the normal layout selection path, step it through a host-owned adapter, validate and diagnose failures, and restore snapshots safely even when the referenced guest is no longer available.
This means the active path targets a worker-safe guest runtime with explicit
bootstrap/health/fallback receipts rather than freezing today's direct
in-process guest execution shape as the long-term contract.
