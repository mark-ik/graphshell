# Graphshell Architectural Overview

**Last Updated**: 2026-03-12
**Status**: Canonical orientation doc — updated with subsystem status map and dependency topology
**Purpose**: High-level orientation, subsystem status at a glance, and dependency map.
Canonical model details live in subsystem and implementation-strategy specs.

---

## 0. Standards Policy

Graphshell should prefer **external normative standards** whenever they specify the behavior we want, and should define **internal specifications** only when:

1. no relevant external standard exists,
2. Graphshell must project multiple standards into one product-specific model,
3. Graphshell intentionally constrains or deviates from an external standard,
4. Graphshell is the only meaningful source of truth for the behavior.

Practical use of external standards in this codebase:

- as the normative source for browser/content behavior,
- as terminology discipline,
- as a guardrail for host/embedder behavior and interoperability.

Use [`2026-03-12_specification_coverage_register.md`](./2026-03-12_specification_coverage_register.md) as the canonical map of:

- which external standards families matter to current/planned features,
- which internal specs already exist,
- which parts of the codebase are still unspecified or underspecified,
- which feature families are inherently Graphshell-defined.

---

## 1. What Graphshell Is

Graphshell is a spatial browser/workbench with three authority domains:

- **Graph Tree**: semantic identity, node/edge truth, traversal/history truth, lifecycle truth.
- **Workbench Tree**: panes, splits, tabs, focus regions, frame/workbench arrangement.
- **Viewer Runtime**: live rendering attachments reconciled from graph/workbench intent.

These domains are strictly separated. Mutation authority, routing, and registry boundaries are
defined in `implementation_strategy/system/system_architecture_spec.md`.

---

## 2. Canonical Authority Map

| Concern | Canonical doc |
| --- | --- |
| System authority boundaries and registries | `implementation_strategy/system/system_architecture_spec.md` |
| Graph/canvas interaction semantics | `implementation_strategy/graph/graph_node_edge_interaction_spec.md` |
| Traversal model, edge payloads, history manager behavior | `implementation_strategy/subsystem_history/edge_traversal_spec.md` |
| History subsystem policy and diagnostics expectations | `implementation_strategy/subsystem_history/SUBSYSTEM_HISTORY.md` |
| Node lifecycle and runtime reconcile | `implementation_strategy/viewer/node_lifecycle_and_runtime_reconcile_spec.md` |
| Viewer selection, presentation, fallback | `implementation_strategy/viewer/viewer_presentation_and_fallback_spec.md` |
| Workbench/frame/tile semantics | `implementation_strategy/workbench/workbench_frame_tile_interaction_spec.md` |
| Graph-first frame semantics | `implementation_strategy/workbench/graph_first_frame_semantics_spec.md` |
| Input routing and modal ownership | `implementation_strategy/aspect_input/input_interaction_spec.md` |
| Command surfaces and omnibar/radial/context parity | `implementation_strategy/aspect_command/command_surface_interaction_spec.md` |
| UX semantic projection, probes, scenarios | `implementation_strategy/subsystem_ux_semantics/SUBSYSTEM_UX_SEMANTICS.md` |
| Diagnostics contracts and health summaries | `implementation_strategy/subsystem_diagnostics/SUBSYSTEM_DIAGNOSTICS.md` |
| Render/compositor pass ownership | `implementation_strategy/aspect_render/frame_assembly_and_compositor_spec.md` |
| Focus authority and region navigation | `implementation_strategy/subsystem_focus/SUBSYSTEM_FOCUS.md` |
| Storage, persistence, WAL integrity | `implementation_strategy/subsystem_storage/SUBSYSTEM_STORAGE.md` |
| Accessibility, AccessKit bridge, Graph Reader | `implementation_strategy/subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md` |

---

## 3. Subsystem Status Map

Implementation status at the subsystem level. For per-feature breakdown see
`implementation_strategy/2026-03-01_complete_feature_inventory.md`.

Status legend: ✅ Done / 🔨 Active (current milestone) / 📋 Planned (spec exists) / 🔭 Speculative

### Core Subsystems

| Subsystem | Status | Summary | Canonical doc |
| --- | --- | --- | --- |
| **Graph — Data Model** | ✅ Done | Force-directed canvas; node/edge CRUD; WAL persistence (AES-256-GCM); zoom/pan/select/lasso; traversal-derived + user-grouped edges | `canvas/graph_node_edge_interaction_spec.md` |
| **Graph — Node Lifecycle** | ✅ Done | Four-state lifecycle (Active → Warm → Cold → Tombstone); LRU eviction; RuntimeBlocked + recovery affordance | `viewer/node_lifecycle_and_runtime_reconcile_spec.md` |
| **Graph — Physics & Layout** | ✅/🔨 | Core force-directed engine done; physics presets wiring active; grid/hierarchical/radial layouts planned | `canvas/2026-02-24_physics_engine_extensibility_plan.md` |
| **Workbench — Tile Tree** | ✅ Done | egui_tiles; Tab Group / Split / Grid; Graph/Node/Tool panes; PaneLock; drag-drop/reorder | `workbench/workbench_frame_tile_interaction_spec.md` |
| **Workbench — Frame Management** | ✅ Done | FrameSnapshot (Layout + Manifest + Metadata); frame switching/workbar; pane-close successor focus handoff | `workbench/graph_first_frame_semantics_spec.md` |
| **Workbench — Multi-View** | ✅ Done | Multiple `GraphViewId` panes; Canonical vs Divergent layout modes; per-view selection state | `canvas/view_dimension_spec.md` |
| **Viewer — Servo/Web** | ✅ Done | Verso mod; Servo WebRender GL output; compositor callback bridge; webview lifecycle; http/https/file; JS/CSS/cookies | `viewer/VIEWER.md` |
| **Viewer — Routing & Fallback** | ✅ Done | ViewerRegistry (MIME → viewer); placeholder fallback; capability declarations; attachment lifecycle | `viewer/viewer_presentation_and_fallback_spec.md` |
| **Viewer — Non-Web Types** | 📋 Planned | Wry native overlay (scaffold); PDF; CSV; Markdown; Settings pane; DOM inspector | `viewer/universal_content_model_spec.md` |
| **Viewer — TileRenderMode** | 🔨 Active | CompositedTexture / NativeOverlay / EmbeddedEgui / Placeholder enum active; pane-targeted mode dispatch live | `aspect_render/frame_assembly_and_compositor_spec.md` |
| **Command Surfaces** | 🔨 Active | ActionRegistry routing done; Command Palette + Omnibar + Radial + Context surfaces active; palette/radial contract in closure | `aspect_command/command_surface_interaction_spec.md` |
| **Input Architecture** | ✅ Done | Input context stack; chord/sequence keybindings; Gamepad support; modal capture | `aspect_input/input_interaction_spec.md` |
| **Render / Compositor** | 🔨 Active | Three-pass composition (UI Chrome → Content → Overlay Affordance) done; CompositorAdapter GL isolation active; differential composition planned | `aspect_render/frame_assembly_and_compositor_spec.md` |

### Cross-Cutting Subsystems

| Subsystem | Status | Summary | Canonical doc |
| --- | --- | --- | --- |
| **Focus** | 🔨 Active | Six-track taxonomy (SemanticRegion / PaneActivation / GraphView / LocalWidget / EmbeddedContent / ReturnCapture); F6 region cycle done; split-authority resolution in progress | `subsystem_focus/2026-03-08_unified_focus_architecture_plan.md` |
| **History** | ✅/📋 | Traversal capture, WAL logging, History Manager timeline done; temporal navigation / replay / time-travel preview spec-landed but runtime-pending | `subsystem_history/SUBSYSTEM_HISTORY.md` |
| **Diagnostics** | 🔨 Active | ChannelRegistry; DiagnosticEvent ring; Diagnostics Inspector pane; channel severity (Error/Warn/Info); recovery affordance S5 active; AnalyzerRegistry + health summaries planned | `subsystem_diagnostics/SUBSYSTEM_DIAGNOSTICS.md` |
| **Storage** | ✅/📋 | WAL append-only log; single-write-path invariant; frame save/load; workspace manifest done; AES-256-GCM at-rest encryption planned | `subsystem_storage/SUBSYSTEM_STORAGE.md` |
| **UX Semantics** | 📋 Planned | UxTree runtime snapshot; UxNodeId (stable path-based); UxProbeSet; UxScenario runner; WebDriver bridge — partial active, spec closure in progress | `subsystem_ux_semantics/SUBSYSTEM_UX_SEMANTICS.md` |
| **Accessibility** | 🔨 Active | AccessKit + egui bridge (version mismatch degraded); WebView accessibility tree forwarding active; Graph Reader (virtual a11y tree) planned; WCAG 2.2 AA normative target | `subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md` |

### Registry Infrastructure (System Layer)

All registries use `namespace:name` key policy. The canonical registry hub is
`implementation_strategy/system/register/SYSTEM_REGISTER.md`.

| Registry | Status | Purpose |
| --- | --- | --- |
| `ActionRegistry` | ✅ Done | Action invocation routing — no hardcoded command enums |
| `ViewerRegistry` | ✅ Done | MIME → viewer resolution; capability declarations |
| `ChannelRegistry` | 🔨 Active | Diagnostic channel schema and severity (formerly DiagnosticsRegistry) |
| `InputRegistry` | ✅ Done | Keybinding resolution |
| `PhysicsProfileRegistry` | ✅ Done | Physics presets (Liquid/Gas/Solid/Frozen) |
| `LensCompositor` | ✅ Done | Resolves Lens (topology + layout + physics + theme) |
| `KnowledgeRegistry` | 📋 Planned | UDC semantic tagging |
| `ModRegistry` | ✅ Done | Native mod loading via `inventory::submit!` |
| `AgentRegistry` | 📋 Planned | Autonomous background agents |
| `ProtocolRegistry` | ✅ Done | Protocol handlers (http, https, file) |

---

## 4. Dependency Topology

Data flow and authority dependencies between subsystems. Arrows indicate "depends on / receives authority from."

```text
                        ┌─────────────────────┐
                        │   Storage (WAL)      │
                        │  ✅ persistence layer │
                        └──────────┬──────────┘
                                   │ durable state
                    ┌──────────────▼──────────────┐
                    │        Graph Subsystem        │
                    │  ✅ node/edge/lifecycle truth  │
                    └───┬───────────────────────┬──┘
                        │ node identity          │ traversal edges
              ┌─────────▼──────────┐   ┌────────▼────────────┐
              │  Workbench Subsys  │   │  History Subsystem   │
              │  ✅ tile tree/panes │   │  ✅ done / 📋 replay  │
              └─────────┬──────────┘   └────────┬────────────┘
                        │ pane host               │ timeline data
                        ▼                         ▼
              ┌─────────────────────────────────────────────┐
              │              Viewer Subsystem                │
              │  ✅ Servo/web  🔨 TileRenderMode  📋 non-web  │
              └─────────────────────────┬───────────────────┘
                                        │ render mode
                                        ▼
                              ┌─────────────────────┐
                              │  Render/Compositor   │
                              │  🔨 three-pass active │
                              └─────────────────────┘

         ┌──────────────────────────────────────────────────────────┐
         │                  Cross-Cutting Authorities                │
         │                                                          │
         │  Focus ──────── arbitrates input routing for all above   │
         │  🔨 active       F6 cycle done; unified model in progress │
         │                                                          │
         │  Command Surfaces ── routes actions into all subsystems  │
         │  🔨 active          ActionRegistry; palette/radial active │
         │                                                          │
         │  Diagnostics ─────── observability outlet for all above  │
         │  🔨 active           channel ring; health summaries TBD   │
         │                                                          │
         │  UX Semantics ── runtime semantic tree; test/probe layer │
         │  📋 planned       UxTree partial; scenario runner planned │
         │                                                          │
         │  Accessibility ── AccessKit bridge; Graph Reader planned │
         │  🔨 active         bridge active (degraded); WCAG AA goal │
         └──────────────────────────────────────────────────────────┘
```

**Key dependency rules** (from `system_architecture_spec.md`):

- Graph semantics ≠ tile-tree layout. Graph Reducer owns model mutations; Workbench Authority owns pane arrangement.
- Workbench layout ≠ graph truth. A pane closing does not imply a node deletion.
- Viewers ≠ graph semantics. ViewerRegistry resolves rendering; it does not own node identity.
- Commands ≠ action meaning. ActionRegistry routes; subsystems define semantics.
- Focus is the arbiter of input routing. No subsystem should capture input without going through Focus authority.
- Diagnostics is observability infrastructure only. It does not own state or make routing decisions.

---

## 5. Implementation Closure Register

Spec-landed features not yet implemented in the runtime — most likely to be misread as done.

| Feature | Spec | Runtime | Blocker |
| --- | --- | --- | --- |
| Faceted filter surface | ✅ `canvas/faceted_filter_surface_spec.md` | ❌ Not started | Reducer filter execution; UxTree surface; diagnostics |
| Facet pane routing | ✅ `canvas/facet_pane_routing_spec.md` | ❌ Not started | Facet-rail context binding; pane target resolution |
| Temporal navigation / replay | ✅ `subsystem_history/SUBSYSTEM_HISTORY.md` | ❌ Not started | Detached preview controller; no-side-effect gates; return-to-present |
| History diagnostics + health summary | ✅ `subsystem_diagnostics/SUBSYSTEM_DIAGNOSTICS.md` | ❌ Not started | `history.*` channel wiring; health surface; CI watchdog |
| Wry native overlay lifecycle | ✅ `viewer/wry_integration_spec.md` | 🏗️ Scaffold only | E2E attach/hide/destroy; overlay sync; platform validation |

---

## 6. Active Architectural Concerns

Open concerns as of 2026-03-08. See `ARCHITECTURAL_CONCERNS.md` for full history and resolved items.

| Concern | Severity | Notes |
| --- | --- | --- |
| `AddressKind` migration to typed `Address` enum | 🔴 Open | Typed enum designed; migration from `AddressKind` hint not scheduled. Blocks IPFS/Gemini/Tor resolvers. |
| `GraphshellClip` address family | 🔴 Open | `graphshell://clip/<uuid>` not canonicalized into address resolution. Clipping plan exists. |
| `RendererKind` vs `viewer_override` relationship | 🟡 Unresolved | Core extraction plan introduces `RendererKind`; UCM uses `viewer_override: Option<ViewerId>`. Relationship unclear. |
| `graph_app.rs` size (11,269 lines) | 🔴 Active decomposition | Stage 1–3 extractions done; still dominant hotspot. Plan: `system/2026-03-08_graph_app_decomposition_plan.md` |
| `render/mod.rs` size (5,146 lines) | 🟡 Active decomposition | Stage 1 done. Follow-on staging needed. Plan: `aspect_render/2026-03-08_render_mod_decomposition_plan.md` |

---

## 7. Current Product Summary

- Core browsing graph is functional (force-directed canvas, node lifecycle, WAL persistence).
- Workbench tile tree is functional (tabs, splits, multi-pane, frame snapshots).
- Traversal-aware edge/history model is the canonical runtime model.
- Four-state lifecycle (Active → Warm → Cold → Tombstone) is canonical.
- History Manager timeline/dissolved-tabs surface is active.
- Temporal preview/replay is spec-complete but runtime-pending.
- Faceted filtering is spec-complete but runtime-pending.
- TileRenderMode is active; non-web viewers (Wry/PDF/Markdown) are planned.
- wgpu/WebRender migration is **deferred indefinitely** (2026-03-12). Graphshell ships on egui_glow / Servo GL compositor.
- v0.0.2 release gate: AG0–AG8 all closed with evidence (see `2026-03-03_pre_wgpu_plot.md`).

For status-by-feature, use `implementation_strategy/2026-03-01_complete_feature_inventory.md`.

---

## 8. Read This Next

- If you are changing reducer/model behavior → start in the relevant subsystem spec.
- If you are changing pane/open/focus behavior → start in workbench and focus specs.
- If you are changing user-visible interaction → start in the UX coverage matrix and canonical interaction spec.
- If you are changing observability or test gates → start in diagnostics and UxScenario specs.
- If you are adding a registry key → follow `namespace:name` policy; see `SYSTEM_REGISTER.md`.
- If you are adding a `GraphIntent` variant → it must be handled in `apply_intents()`.
- If you are adding a `DiagnosticChannelDescriptor` → it must have a `severity` field.

---

## 9. Anti-Pattern

Do not treat this document as authority for:

- concrete Rust type shapes,
- lifecycle transition tables,
- traversal append rules,
- route naming policy,
- diagnostics channel lists,
- acceptance criteria.

Those belong in canonical subsystem/spec docs and must be changed there first.
