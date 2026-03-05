# Model Boundary Control Matrix

**Date**: 2026-03-04  
**Status**: Canonical cross-doc audit artifact  
**Authority root**: `2026-02-28_ux_contract_register.md` (§§2A, 3B, 3C)

---

## 1. Boundary legend

- `GraphId` = durable graph/content truth boundary.
- `GraphViewId` = scoped view-state boundary (camera/lens/selection/filter memory).
- file tree = graph-backed hierarchical projection (navigation surface only).
- workbench = arrangement boundary (pane/tile/frame hosting only).

---

## 2. Control matrix

| Subsystem/spec | Primary owner | Allowed state ownership | Prohibited ownership | Verification anchor |
| --- | --- | --- | --- | --- |
| UX register (`subsystem_ux_semantics/2026-02-28_ux_contract_register.md`) | UX semantics authority | Vocabulary, ownership model, contract template | Runtime layout-specific semantics | Contract-template compliance review + spec-link audit |
| Graph (`canvas/graph_node_edge_interaction_spec.md`) | Graph subsystem | Graph truth, graph interactions, graph-backed projection semantics | Workbench tile arrangement ownership | Graph interaction + routing scenario tests |
| Workbench (`workbench/workbench_frame_tile_interaction_spec.md`) | Workbench subsystem | Tile/frame/pane arrangement, focus handoff for arrangement transitions | Content truth / durable hierarchy ownership | Tile lifecycle and focus handoff scenario tests |
| Focus (`subsystem_focus/focus_and_region_navigation_spec.md`) | Focus router | Region focus ownership and deterministic return-path semantics | Reassignment of graph/workbench semantic ownership | Focus-cycle + no-trap diagnostics tests |
| Command (`aspect_command/command_surface_interaction_spec.md`) | Action/dispatch authority | Command semantics, target resolution, disabled-state policy | Independent semantic models per command UI surface | Action parity and dispatch diagnostics tests |
| Settings (`aspect_control/settings_and_control_surfaces_spec.md`) | Settings/control subsystem | Route/apply/persist/return-path control semantics | Becoming owner of graph/workbench identity models | Settings persistence + return-path scenario tests |
| Multi-view (`canvas/multi_view_pane_spec.md`) | Graph + workbench bridge | Per-`GraphViewId` isolation semantics and pane-host contract | Treating tile structure as graph truth | Multi-pane isolation and routing tests |
| Lens (`system/register/lens_compositor_spec.md`) | Lens compositor registry | Graph-view lens composition (view scope) | Workbench layout/session authority | Lens resolution + fallback contract tests |
| Workbench surface registry (`system/register/workbench_surface_registry_spec.md`) | Workbench surface registry | Arrangement policy and tile-tree interaction policy | Graph truth and projection semantics | Registry contract tests + workbench scenario coverage |

---

## 3. Terminology lock (audit checks)

1. No spec calls tile/frame arrangement a content hierarchy.
2. No spec calls file tree the content truth authority.
3. No spec calls physics presets camera modes.
4. No spec uses Canonical/Divergent as graph-view layout ownership labels; layout ownership is local-per-view.

Each canonical spec review should include this 3-item drift check before approval.

---

## 4. Tightening Pass Target (type guarantee + hardening + guard tests)

### 4.1 Boundary guarantee target

| Concern | Current risk | Target guarantee |
| --- | --- | --- |
| Workbench-authority intents in reducer path | Runtime-ordering contract; leakage currently detectable only at runtime | Compile-time separation: reducer APIs accept only reducer-domain intents |
| Workbench route/open URL semantics | Mixed handling paths can blur authority | Explicit route/open intent split between arrangement authority and reducer authority |
| Pane/session reopen behavior | Ambiguous restore vs fresh-open semantics | Explicit reopen policy by binding scope (same pane binding vs new pane binding) |

### 4.2 Required hardening steps

1. Introduce typed intent boundary:
   - reducer entrypoint consumes `GraphReducerIntent` only,
   - workbench frame loop consumes `WorkbenchIntent` only.
2. Keep any bridge operation explicit:
   - graph-to-workbench open/routing requests pass through a dedicated bridge type,
   - bridge includes semantic payload (`NodeKey`, `GraphViewId`, placement hint) and emits diagnostics receipts.
3. Convert runtime warnings for impossible routing to hard failure in debug/test builds:
   - leaked workbench intent in reducer path must `panic!` under `debug_assertions` and tests.
4. Lock persistence tiers:
   - graph durability,
   - workspace arrangement durability,
   - ephemeral pane-session cache.

### 4.3 Guard test minimum set

1. Reducer boundary test:
   - verify reducer API cannot accept workbench-only intents (type-level gate).
2. Leak guard test:
   - inject a leaked workbench intent path in test build and assert hard failure.
3. Reopen semantics scenario:
   - open node, navigate, close pane, reopen same pane binding -> session restored.
4. Fresh-open scenario:
   - open same node in a new pane binding -> fresh session by default.
5. Cross-boundary audit scenario:
   - route/open action from graph emits expected diagnostics and does not mutate tile tree from reducer code.
