# Accessibility Contracts, Diagnostics, and Validation Strategy (2026-02-25)

> **SUPERSEDED** — This document has been consolidated into
> `2026-02-25_subsystem_accessibility.md` (Cross-Cutting Subsystem: Accessibility).
> Retained for historical reference only. Do not use as authoritative.

**Status**: ~~Active / Project Goal~~ Superseded (2026-02-25)
**Scope**: Cross-cutting guarantees for WebView accessibility bridge, graph accessibility, focus/navigation, live announcements, and future viewer surfaces
**Companion docs**:
- ~~`2026-02-24_spatial_accessibility_plan.md`~~ → see `2026-02-25_subsystem_accessibility.md`
- `../research/2026-02-24_spatial_accessibility_research.md` (research basis)
- `../research/2026-02-24_diagnostics_research.md` (observability/registry design precedent)

---

## 1. Why This Exists

The spatial accessibility plan defines **features** (WebView bridge, Graph Reader, navigation, sonification).
This document defines **guarantees**:

- What must remain accessible as the system evolves
- How regressions are detected at runtime and in CI
- How future viewers/panes declare and prove accessibility support

Accessibility is a **project-level reliability requirement**, not a one-time UI deliverable.

---

## 2. Accessibility System Model (Diagnostics-Style)

Accessibility is treated as a first-class runtime subsystem with four layers:

1. **Accessibility Contracts (schema/invariants)**
- Declarative requirements for focus, tree integrity, update handling, action routing, and degradation behavior.

2. **Accessibility Runtime State**
- The live bridge/adapter state (queued updates, anchors, focus targets, mode state, counters).

3. **Accessibility Diagnostics**
- Runtime channels, health metrics, and invariant violations emitted through the diagnostics system.

4. **Accessibility Validation**
- Unit/integration/scenario tests + CI gates that enforce contract compliance over time.

This mirrors the diagnostics architecture principle: declarative contracts + runtime observability + deterministic validation.

---

## 3. Accessibility Contracts (Required Invariants)

These invariants are mandatory and should be encoded as explicit checks/tests/diagnostics.

### 3.1 Tree Integrity Invariants

1. **Stable IDs**
- Virtual accessibility node IDs must be deterministic across refreshes for the same semantic entity.
- Focused element identity must survive non-semantic refreshes.

2. **No orphan subtrees**
- No accessibility subtree may be injected without a valid registered parent/anchor in the current frame.

3. **No duplicate active roots**
- A surface/viewer contributes at most one active root subtree per frame per anchor.

4. **Parent/child consistency**
- Every child reference in emitted/injected nodes refers to a node present in the same update set or a stable pre-existing parent/anchor.

5. **Stale update safety**
- Updates for closed/removed webviews or panes are dropped deterministically and logged, never causing panics or memory growth.

### 3.2 Focus & Navigation Invariants

1. **Focus preservation on refresh**
- If a focused semantic target still exists after refresh, focus remains on that target.

2. **Predictable fallback focus**
- If the focused target disappears, focus falls back to a documented parent/sibling/root policy (never to arbitrary elements).

3. **Mode transitions preserve return path**
- Room ↔ Map transitions retain enough state to restore focus to the prior semantic location.

4. **F6 region cycle completeness**
- Top-level focus cycle is deterministic and visits all required regions in a stable order.

### 3.3 Action Routing Invariants

1. **Action delivery correctness**
- AccessKit action requests are routed to the owning subsystem (egui widget, Graph Reader, or WebView bridge target).

2. **No cross-surface misrouting**
- Actions intended for one webview/pane must not mutate another.

3. **Unsupported action behavior is explicit**
- Unsupported actions return/log a clear outcome; they are not silently ignored in a way that appears successful.

### 3.4 Degradation Invariants

1. **Graceful degradation is declared**
- If a capability is unavailable (e.g., WebView bridge disabled), the system emits diagnostics and exposes user-visible status.

2. **Degradation is non-silent**
- Repeated fallback/drop paths must be observable (counters/channels), not one-time logs only.

3. **Fallback remains usable**
- Core app navigation remains accessible to the maximum supported extent even when one subsystem degrades.

---

## 4. Accessibility Capability Registry (Future-Proofing)

To extend accessibility to future components, each surface/viewer should declare accessibility capabilities explicitly.

### 4.1 Capability Descriptor (concept)

Each viewer/surface (e.g., Servo webview, Wry webview, graph canvas, tool pane) should provide:

- `surface_id`
- `owner_source` (`core`, `viewer`, `mod`, etc.)
- `capabilities`:
  - `native_tree_bridge`
  - `virtual_tree`
  - `focus_sync`
  - `action_routing`
  - `live_regions`
  - `keyboard_navigation`
- `degradation_mode`:
  - `full`
  - `partial`
  - `none`
- `notes` / `reason` for unsupported capabilities

### 4.2 Why a Registry

- Prevents silent regressions when new viewers are added
- Enables diagnostics-pane health summaries by surface
- Lets CI assert minimum accessibility support for core surfaces
- Provides a contract point for mod-contributed panes/viewers

This can be implemented as an `AccessibilityRegistry` or folded into existing viewer/surface registries with an accessibility sub-structure.

---

## 5. Accessibility Diagnostics Integration (Required)

Accessibility must be observable via the diagnostics system, not just logs.

### 5.1 Required Diagnostic Channels (initial set)

Suggested channel families (core/runtime):

- `accessibility.bridge.webview_update_received`
- `accessibility.bridge.webview_update_queued`
- `accessibility.bridge.webview_update_injected`
- `accessibility.bridge.webview_update_dropped`
- `accessibility.bridge.webview_update_conversion_failed`
- `accessibility.bridge.webview_anchor_missing`
- `accessibility.bridge.webview_stale_update`
- `accessibility.focus.sync_succeeded`
- `accessibility.focus.sync_failed`
- `accessibility.action.routed`
- `accessibility.action.route_failed`
- `accessibility.graph.virtual_tree_rebuilt`
- `accessibility.graph.virtual_tree_throttled`
- `accessibility.graph.virtual_tree_invariant_failed`
- `accessibility.announcer.message_emitted`

Severity guidance:
- `*_failed`, `*_conversion_failed`, `*_invariant_failed` → `Error`
- `*_dropped`, `*_anchor_missing`, `*_stale_update`, `*_throttled` → `Warn`
- normal lifecycle channels (`received`, `queued`, `injected`, `routed`, `rebuilt`) → `Info`

### 5.2 Accessibility Health Summary (Pane/Diagnostics)

The diagnostics pane should expose an accessibility section with:

- WebView bridge status (`active`, `degraded`, `disabled`)
- Last update latency / queue depth
- Recent drop/conversion-failure counters
- Focus sync success/failure counts
- Active Graph Reader mode (`Off`, `Room`, `Map`)
- Capability coverage summary by surface/viewer

### 5.3 Invariant Violations as First-Class Events

Accessibility invariant failures should follow the same pattern as diagnostics invariants:
- explicit invariant IDs
- structured context (surface/viewer, target IDs, reason)
- session counts
- last occurrence timestamp

---

## 6. Validation Strategy (Tests + CI)

### 6.1 Test Categories

1. **Unit tests (deterministic)**
- Node ID derivation stability
- Semantic hierarchy ordering
- Room/Map tree builders
- WebView tree conversion compatibility (0.24 -> egui-compatible types)
- Fallback/degradation policy decisions

2. **Integration tests (headless/local state)**
- Focus preservation across tree refreshes
- Action routing to correct target
- F6 region cycle order
- Room ↔ Map return-path focus restoration

3. **Scenario tests (harness / diagnostics-backed)**
- WebView bridge receives updates and injects (or degrades with explicit diagnostics)
- Graph Reader updates emit tree rebuild/throttle channels
- Accessibility health remains green under typical workflows

4. **Manual smoke checks (screen reader)**
- Platform-specific smoke scripts (NVDA/Windows, Orca/Linux, VoiceOver/macOS when applicable)
- Required for milestone gates, not every PR

### 6.2 CI Gates (Project Goal)

Add a dedicated accessibility test lane with required checks for PRs touching:

- `shell/desktop/ui/**`
- `shell/desktop/workbench/**`
- `render/**`
- `app.rs`
- viewer integration / webview lifecycle code
- accessibility/diagnostics registries and adapters

Minimum CI requirements (phased):

Phase A (immediate):
- unit tests for ID stability + degradation policy
- compile-time guard that the WebView bridge fallback is observable (diagnostics/log path present)

Phase B (WebView bridge functional):
- integration test proving `received -> injected` path works
- no unexpected drop/conversion-failure diagnostics in the happy path scenario

Phase C (Graph Reader landed):
- deterministic linearization tests
- focus preservation tests
- F6 and mode-switch navigation tests

### 6.3 Golden Snapshot Policy

For virtual trees (Graph Reader, key tool panes), add golden snapshots:

- Tree shape
- Labels/descriptions
- Node IDs (or stable ID derivation traces)

Golden snapshots should be:
- small
- deterministic
- reviewed when changed (not auto-regenerated silently)

---

## 7. Degradation Policy (Explicit, Tested)

Accessibility support is allowed to degrade; silent undefined behavior is not.

### 7.1 Required Degradation States

For each surface/viewer:
- `Full`: complete expected support
- `Partial`: some capabilities unavailable (must enumerate which)
- `Unavailable`: no active accessibility bridge/tree

### 7.2 Required User/Developer Signals

When degraded/unavailable:
- diagnostics channels emitted
- diagnostics pane status reflects degradation
- log message is rate-limited (no spam)
- optional UI indicator for developers (debug/diagnostics mode)

### 7.3 Example: WebView Bridge Version Mismatch

Current known condition:
- Servo emits `accesskit 0.24`
- egui 0.33 consumes `accesskit 0.21`

Required behavior until fixed:
- queue updates
- convert or fail deterministically
- emit `accessibility.bridge.webview_update_conversion_failed` or equivalent
- report degraded bridge status
- avoid unbounded queue growth and panics

---

## 8. Ownership Boundaries (Who Guarantees What)

### 8.1 `EmbedderWindow` / Host Layer
- Forwards native accessibility updates/events to GUI
- Preserves source identity (`WebViewId`, window context)
- Does not silently drop updates without diagnostics

### 8.2 `Gui` / Bridge Layer
- Queues, anchors, converts, injects, and diagnoses bridge updates
- Enforces stale-update and anchor-missing policies
- Tracks bridge health state and counters

### 8.3 `GraphAccessKitAdapter` (future)
- Produces deterministic virtual tree + stable IDs
- Enforces Room/Map tree integrity invariants
- Emits rebuild/throttle diagnostics

### 8.4 Input/Navigation Layer
- Owns F6 cycles and Graph Reader command routing
- Preserves focus return-path semantics

### 8.5 Diagnostics Layer
- Records accessibility channels and invariant failures
- Exposes health summaries and forensic history

---

## 9. Implementation Sequence (Companion to Spatial Accessibility Plan)

This sequence should be treated as a prerequisite scaffold for future accessibility features.

1. **Accessibility observability baseline**
- Add channels + runtime counters + diagnostics pane status summary placeholders.

2. **WebView bridge plumbing hardening**
- Anchor mapping (`WebViewId -> egui::Id`)
- stale-update policy
- queue metrics

3. **Type compatibility / conversion layer**
- Resolve `accesskit` version split via compatibility conversion (or dependency alignment if feasible)
- restore real injection path

4. **Bridge invariants + tests**
- Add explicit invariant checks and CI tests for bridge happy path and degradation path

5. **Graph Reader implementation**
- Build `GraphAccessKitAdapter` on top of the above diagnostics/validation scaffolding

6. **Focus/navigation guarantees**
- Add F6/mode/focus preservation tests and diagnostics

7. **Announcer + sonification**
- Add observability and validation contracts as these subsystems land

---

## 10. Immediate Next Actions (Status Quo Alignment)

Based on current code:

1. Add a WebView accessibility bridge status struct/counters in `Gui` and expose diagnostics channels.
2. Add `WebViewId -> egui::Id` accessibility anchor registration path in tile/webview rendering.
3. Implement an `accesskit` compatibility conversion spike (`0.24 -> egui-compatible`) with unit tests.
4. Replace the current warn-and-drain fallback with:
- convert-and-inject on success
- structured failure diagnostics on conversion error

---

## 11. Done Definition for “Accessibility Is Guaranteed”

This project can claim future-facing accessibility guarantees only when all of the following are true:

- Accessibility contracts are documented and encoded as tests/invariants
- Accessibility diagnostics channels are part of the core diagnostics schema
- CI has required accessibility checks for UI/viewer changes
- Degradation modes are explicit, observable, and tested
- New viewers/surfaces must declare accessibility capability coverage

Until then, accessibility is an implementation effort; after that, it becomes a maintained system property.
