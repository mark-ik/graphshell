# Cross-Cutting Subsystem: Accessibility (2026-02-25)

**Status**: Active / Project Goal
**Subsystem label**: `accessibility`
**Long form**: Accessibility Subsystem
**Scope**: WebView accessibility bridge, graph accessibility (Graph Reader), focus/navigation, live announcements, sonification, and future viewer surfaces
**Subsystem type**: Cross-Cutting Runtime Subsystem (see `TERMINOLOGY.md`)
**Peer subsystems**: `diagnostics` (Diagnostics), `security` (Security & Access Control), `storage` (Persistence & Data Integrity), `history` (Traversal & Temporal Integrity)
**Doc role**: Canonical subsystem implementation guide (summarizes guarantees/roadmap and links to detailed plans; avoid duplicating updates across accessibility docs)
**Consolidates**:
- `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-24_spatial_accessibility_plan.md` (now superseded — feature phases preserved in §8)
- `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-25_accessibility_contracts_diagnostics_and_validation_strategy.md` (now superseded — contract framing preserved in §§3-7)
**Research basis**: `../research/2026-02-24_spatial_accessibility_research.md`

---

## 1. Why This Exists

Accessibility is a **project-level reliability requirement**, not a one-time UI deliverable.

The feature plan defines **what to build** (WebView bridge, Graph Reader, navigation, sonification). This document defines **what must remain true** as the system evolves — contracts, observability, validation, and extensibility gates that prevent silent regressions.

Without subsystem-level guarantees, every new `TileKind` variant, every Wry/Servo backend change, and every mod-contributed pane becomes a silent accessibility regression vector.

---

## 2. Subsystem Model (Four Layers)

| Layer | Accessibility Instantiation |
|---|---|
| **Contracts** | Tree integrity, focus/navigation, action routing, degradation invariants (§3) |
| **Runtime State** | Bridge state: queued updates, anchors (`WebViewId → egui::Id`), focus targets, graph reader mode, counters |
| **Diagnostics** | `accessibility.*` channel family + health summary in Diagnostic Inspector (§5) |
| **Validation** | Unit/integration/scenario tests + CI gates + golden snapshots (§6) |

---

## 3. Accessibility Contracts (Required Invariants)

These invariants are mandatory and must be encoded as explicit checks/tests/diagnostics.

### 3.1 Tree Integrity Invariants

1. **Stable IDs** — Virtual accessibility node IDs are deterministic across refreshes for the same semantic entity. Focused element identity survives non-semantic refreshes.
2. **No orphan subtrees** — No accessibility subtree may be injected without a valid registered parent/anchor in the current frame.
3. **No duplicate active roots** — A surface/viewer contributes at most one active root subtree per frame per anchor.
4. **Parent/child consistency** — Every child reference in emitted/injected nodes refers to a node present in the same update set or a stable pre-existing parent/anchor.
5. **Stale update safety** — Updates for closed/removed webviews or panes are dropped deterministically and logged, never causing panics or memory growth.

### 3.2 Focus & Navigation Invariants

1. **Focus preservation on refresh** — If a focused semantic target still exists after refresh, focus remains on that target.
2. **Predictable fallback focus** — If the focused target disappears, focus falls back to a documented parent/sibling/root policy (never to arbitrary elements).
3. **Mode transitions preserve return path** — Room ↔ Map transitions retain enough state to restore focus to the prior semantic location.
4. **F6 region cycle completeness** — Top-level focus cycle is deterministic and visits all required regions in a stable order.

### 3.3 Action Routing Invariants

1. **Action delivery correctness** — AccessKit action requests are routed to the owning subsystem (egui widget, Graph Reader, or WebView bridge target).
2. **No cross-surface misrouting** — Actions intended for one webview/pane must not mutate another.
3. **Unsupported action behavior is explicit** — Unsupported actions return/log a clear outcome; they are not silently ignored in a way that appears successful.

### 3.4 Degradation Invariants

1. **Graceful degradation is declared** — If a capability is unavailable (e.g., WebView bridge disabled), the system emits diagnostics and exposes user-visible status.
2. **Degradation is non-silent** — Repeated fallback/drop paths are observable (counters/channels), not one-time logs only.
3. **Fallback remains usable** — Core app navigation remains accessible to the maximum supported extent even when one subsystem degrades.

---

## 4. Surface Capability Declarations (Folded Approach)

Each viewer/surface declares accessibility capabilities in its owning registry entry via an `AccessibilityCapabilities` sub-structure:

### 4.1 Descriptor Shape

```
surface_id: String
owner_source: core | viewer | mod
capabilities:
  native_tree_bridge: full | partial | none
  virtual_tree: full | partial | none
  focus_sync: full | partial | none
  action_routing: full | partial | none
  live_regions: full | partial | none
  keyboard_navigation: full | partial | none
degradation_mode: full | partial | none
notes: String  // reason for unsupported capabilities
```

### 4.2 Registry Integration

- `ViewerRegistry` entries (Servo, Wry, plaintext, future mod viewers) carry `AccessibilityCapabilities`.
- `CanvasRegistry` carries capabilities for the graph canvas.
- `WorkbenchSurfaceRegistry` carries capabilities for the tile-tree surface (tab bars, split handles, container chrome).

### 4.3 Why This Matters

- Prevents silent regressions when new viewers are added.
- Enables diagnostics pane health summaries by surface.
- Lets CI assert minimum accessibility support for core surfaces.
- Provides a contract point for mod-contributed panes/viewers.

---

## 5. Diagnostics Integration

### 5.1 Required Diagnostic Channels

| Channel | Severity | Description |
|---|---|---|
| `accessibility.bridge.webview_update_received` | Info | WebView tree update received from embedder |
| `accessibility.bridge.webview_update_queued` | Info | Update queued for bridge processing |
| `accessibility.bridge.webview_update_injected` | Info | Tree nodes injected into egui AccessKit |
| `accessibility.bridge.webview_update_dropped` | Warn | Update dropped (stale/removed webview) |
| `accessibility.bridge.webview_update_conversion_failed` | Error | AccessKit version conversion failed |
| `accessibility.bridge.webview_anchor_missing` | Warn | WebView lacks anchor registration |
| `accessibility.bridge.webview_stale_update` | Warn | Update for closed/removed webview |
| `accessibility.focus.sync_succeeded` | Info | Focus transferred to target |
| `accessibility.focus.sync_failed` | Error | Focus transfer failed |
| `accessibility.action.routed` | Info | Action routed to correct subsystem |
| `accessibility.action.route_failed` | Error | Action routing failed |
| `accessibility.graph.virtual_tree_rebuilt` | Info | Graph Reader tree rebuilt |
| `accessibility.graph.virtual_tree_throttled` | Warn | Tree rebuild throttled |
| `accessibility.graph.virtual_tree_invariant_failed` | Error | Tree integrity invariant violated |
| `accessibility.announcer.message_emitted` | Info | Live region announcement emitted |

### 5.2 Health Summary (Diagnostic Inspector)

The Diagnostic Inspector accessibility section exposes:
- WebView bridge status (`active` / `degraded` / `disabled`)
- Last update latency / queue depth
- Recent drop/conversion-failure counters
- Focus sync success/failure counts
- Active Graph Reader mode (`Off` / `Room` / `Map`)
- Capability coverage summary by surface/viewer

### 5.3 Invariant Violations as First-Class Events

Accessibility invariant failures follow the diagnostics pattern: explicit invariant IDs, structured context (surface/viewer, target IDs, reason), session counts, last occurrence timestamp.

---

## 6. Validation Strategy

### 6.1 Test Categories

**Unit tests (deterministic)**:
- Node ID derivation stability
- Semantic hierarchy ordering (Cluster → Hub → Leaf)
- Room/Map tree builders
- WebView tree conversion compatibility (0.24 → egui-compatible types)
- Fallback/degradation policy decisions

**Integration tests (headless/local state)**:
- Focus preservation across tree refreshes
- Action routing to correct target
- F6 region cycle order
- Room ↔ Map return-path focus restoration

**Scenario tests (harness/diagnostics-backed)**:
- WebView bridge receives updates and injects (or degrades with explicit diagnostics)
- Graph Reader updates emit tree rebuild/throttle channels
- Accessibility health remains green under typical workflows

**Manual smoke checks (screen reader)**:
- Platform-specific smoke scripts (NVDA/Windows, Orca/Linux, VoiceOver/macOS when applicable)
- Required for milestone gates, not every PR

### 6.2 CI Gates

Dedicated accessibility test lane with required checks for PRs touching:
- `shell/desktop/ui/**`, `shell/desktop/workbench/**`, `render/**`, `app.rs`
- Viewer integration / webview lifecycle code
- Accessibility/diagnostics registries and adapters

**Phase A (immediate)**: Unit tests for ID stability + degradation policy; compile-time guard that WebView bridge fallback is observable.
**Phase B (bridge functional)**: Integration test proving `received → injected` path works; no unexpected drop/conversion-failure diagnostics in happy path.
**Phase C (Graph Reader landed)**: Deterministic linearization tests; focus preservation tests; F6 and mode-switch navigation tests.

### 6.3 Golden Snapshot Policy

For virtual trees (Graph Reader, key tool panes): tree shape, labels/descriptions, node IDs. Small, deterministic, reviewed when changed.

---

## 7. Degradation Policy

### 7.1 Required States

Per surface/viewer: `Full`, `Partial` (enumerate which capabilities unavailable), `Unavailable` (no active bridge/tree).

### 7.2 Required Signals

When degraded/unavailable: diagnostics channels emitted, Diagnostic Inspector status reflects degradation, log message rate-limited, optional UI indicator in debug mode.

### 7.3 Known Degradation: WebView Bridge Version Mismatch

Current condition: Servo emits `accesskit 0.24`, egui 0.33 consumes `accesskit 0.21`.

Required behavior until fixed: queue updates, convert or fail deterministically, emit `accessibility.bridge.webview_update_conversion_failed`, report degraded bridge status, avoid unbounded queue growth and panics.

---

## 8. Implementation Roadmap (Subsystem-Local)

This is the canonical subsystem-local roadmap. Phase mechanics and historical drafting detail are retained in linked docs; update this roadmap first when priorities change.

### Phase 1: WebView Bridge (Critical Fix)

Screen readers can read web content inside Graphshell.

1. Update `EmbedderWindow`: forward `notify_accessibility_tree_update` events to `Gui`.
2. Update `Gui` bridge state: track `WebViewId → egui::Id` accessibility anchors; queue pending updates per webview.
3. Compatibility layer: convert Servo `accesskit 0.24` tree updates to egui-compatible types.
4. Bridge injection: inject converted nodes via egui's AccessKit node-builder hooks under registered anchor.
5. Diagnostics + degradation: emit bridge channels, surface degraded status.

### Phase 2: Graph Reader (Virtual Tree / Graph Linearization)

The graph canvas becomes a navigable structure for screen readers and keyboard-only users.

**Mode A — Room Mode (default when a node is focused)**: Focused node as "Room" — node summary, connected edges grouped by direction (Outgoing/Incoming/Bidirectional), cluster context. Depth 1 only.

**Mode B — Map Mode (global linearization)**: Full graph flattened via Semantic Hierarchy algorithm (Cluster → Hub → Leaf). Fallback: Spatial Sweep (Y then X). Edges not materialized at map level.

**Navigation Entry Points**:

| Trigger | Action |
|---|---|
| `Tab`/`Shift+Tab` (graph canvas focused) | Next/previous node in active linearization |
| `Ctrl+L` | Toggle Canvas Mode ↔ List Mode (Map Mode linearization) |
| `Enter` on focused node (Map Mode) | Drill into Room Mode |
| `Escape` (Room Mode) | Return to Map Mode, restore prior focus |
| `Ctrl+Arrow` | Jump between cluster groups (Level 1) in Map Mode |
| `F6` | Cycle focus: Toolbar → Graph Canvas → Active Pane |
| `Alt+Shift+R` | Explicitly enter Map Mode from anywhere |

**Architecture**: `GraphAccessKitAdapter` takes `Graph` + `SelectionState` + `MetadataFrame` → produces `accesskit::TreeUpdate`. Stable `accesskit::NodeId`s derived from `Node.id` UUIDs. Updates throttled to 10 Hz.

### Phase 3: Navigation & Focus

`F6` skip-link handler, programmatic focus on `FocusNode`, spatial D-Pad (arrow keys → nearest node by direction).

### Phase 4: Sonification (Audio Display)

`Sonifier` via `rodio` + `fundsp`. Spatial cues (panning, pitch) based on graph state.

### Phase 5: Inspector & Automation

"Accessibility" tab in Diagnostic Inspector showing current AccessKit tree structure and announcer events. `test_linearization_order` harness scenario.

---

## 9. Ownership Boundaries

| Owner | Responsibility |
|---|---|
| `EmbedderWindow` / Host Layer | Forward native accessibility updates/events; preserve source identity (`WebViewId`); no silent drops without diagnostics |
| `Gui` / Bridge Layer | Queue, anchor, convert, inject, diagnose bridge updates; enforce stale-update and anchor-missing policies; track bridge health |
| `GraphAccessKitAdapter` (future) | Produce deterministic virtual tree + stable IDs; enforce Room/Map integrity invariants; emit rebuild/throttle diagnostics |
| Input/Navigation Layer | Own F6 cycles and Graph Reader command routing; preserve focus return-path semantics |
| Diagnostics Layer | Record accessibility channels and invariant failures; expose health summaries |

---

## 10. Current Status & Gaps

**What exists**:
- Accessibility subsystem contracts, diagnostics integration requirements, validation strategy, and degradation policy are now centralized in this guide.
- WebView bridge event forwarding and GUI queueing path landed, but the injection step is currently a compile-safe fallback (updates drained with diagnostics) due to `accesskit` version mismatch.
- Phase roadmap for WebView bridge, Graph Reader, and focus/navigation is defined and sequenced.

**What's missing / open**:
- Functional WebView tree injection into egui via an `accesskit` compatibility layer or dependency alignment.
- Stable `WebViewId -> egui::Id` anchor registration path wired from pane/tile render lifecycle.
- Bridge invariants and CI checks for happy-path injection vs degraded fallback.
- Graph Reader (`GraphAccessKitAdapter`) implementation and focus/action routing coverage.

## 11. Dependencies / Blockers

- `egui`/`accesskit` version compatibility mismatch currently blocks direct WebView accessibility subtree injection.
- Pane/tile render lifecycle needs stable anchor registration semantics for webview-hosting panes.
- Focus routing work depends on pane-hosted multi-view architecture clarity (`Tile`/`Pane`/viewer ownership boundaries).
- Graph Reader implementation depends on graph view model stabilization and diagnostics hooks for invariant reporting.

## 12. Linked Docs

- `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-24_spatial_accessibility_plan.md` (detailed feature-plan phases and earlier implementation notes; archived and superseded by this guide)
- `design_docs/archive_docs/checkpoint_2026-02-25/2026-02-25_accessibility_contracts_diagnostics_and_validation_strategy.md` (companion guarantee framework; archived and superseded into this guide, retained for design history/detail)
- `../research/2026-02-24_spatial_accessibility_research.md` (research basis)
- `2026-02-25_subsystem_diagnostics.md` (diagnostics channels, severity, health summaries used by accessibility)
- `2026-02-25_planning_register_backlog_and_copilot_guides.md` (cross-subsystem sequencing and backlog)

## 13. Execution Order (Near-Term)

1. **Accessibility observability baseline** — channels + runtime counters + diagnostics pane status placeholders
2. **WebView bridge plumbing** — anchor mapping, stale-update policy, queue metrics
3. **Type compatibility layer** — resolve `accesskit` version split (conversion or dependency alignment)
4. **Bridge invariants + tests** — explicit checks and CI tests for bridge happy and degradation paths
5. **Graph Reader** — `GraphAccessKitAdapter` built on diagnostics/validation scaffold
6. **Focus/navigation guarantees** — F6/mode/focus preservation tests and diagnostics
7. **Announcer + sonification** — observability and validation contracts as these land

---

## 14. Done Definition

Accessibility is a guaranteed system property when all of the following are true:

- Accessibility contracts are documented and encoded as tests/invariants
- Accessibility diagnostics channels are part of the core diagnostics schema
- CI has required accessibility checks for UI/viewer changes
- Degradation modes are explicit, observable, and tested
- New viewers/surfaces must declare accessibility capability coverage via Surface Capability Declarations

Until then, accessibility is an implementation effort. After that, it is a maintained system property.
