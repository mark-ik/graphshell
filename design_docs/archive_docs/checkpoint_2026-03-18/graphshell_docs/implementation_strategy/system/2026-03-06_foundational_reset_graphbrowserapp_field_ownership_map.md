# Foundational Reset GraphBrowserApp Field Ownership Map

**Date**: 2026-03-06
**Status**: Phase A receipt (updated after CLAT-1 landing)
**Purpose**: Classify current `GraphBrowserApp`-owned mutable state as `domain`, `workbench`, `runtime`, or `unknown` so the foundational reset can move state by declared ownership instead of intuition.

**Related**:
- `system_architecture_spec.md`
- `2026-03-06_foundational_reset_implementation_plan.md`
- `2026-03-06_reducer_only_mutation_enforcement_plan.md`
- `2026-02-22_registry_layer_plan.md`

---

## 1. Scope And Rules

This receipt classifies mutable state reachable from `GraphBrowserApp` in `graph_app.rs`.

Classification vocabulary:

- `domain`: durable graph truth, replay-worthy data, or state that should survive runtime reconstruction
- `workbench`: pane/frame/view/layout/selection/tool-surface state owned by the app shell
- `runtime`: disposable live bindings, caches, effect staging, render state, or resource-management state
- `unknown`: semantically mixed, contradictory, or still unclear enough that Phase B should not move it blindly

Deliberate scope limit:

- opaque value types such as `Camera`, `LensConfig`, and physics parameter structs are classified at the containing-field level unless they are themselves primary state carriers in `graph_app.rs`
- nested state carriers defined in `graph_app.rs` are expanded below when they materially affect ownership decisions

Important reconciliation rule:

- this receipt classifies **state storage ownership**
- it does not replace existing **subsystem policy ownership**

Examples:

- Graph subsystem may own graph-camera interaction policy while concrete view-local camera state is stored in workbench-owned containers.
- Focus subsystem may own focus rules while focus/selection state is stored in workbench-owned containers.
- Register/runtime may own async composition while service handles remain runtime-owned state.

---

## 2. Decision Summary

The current `GraphBrowserApp` is not uniformly mixed chaos.

Most state already clusters into three buckets:

- `domain`: small but clear core around `graph`, `notes`, and durable identity-support helpers
- `workbench`: large shell-owned surface for view, frame, selection, tool, snapshot, and preference state
- `runtime`: large disposable surface for webview bindings, render caches, physics churn, memory pressure, and history preview execution state

The real unresolved seams are concentrated in a short list:

1. `views`: one container holding both workbench view state and runtime-only view caches
2. `camera`: legacy global camera state that competes with per-view camera ownership
3. `undo_stack` / `redo_stack`: mixed snapshots containing both domain and workbench truth with no first-class transaction model
4. `semantic_tags`: runtime-keyed semantic storage that conflicts with the reset's domain-owned semantic truth model

Those should be treated as the first explicit `unknown` items for Phase B and later transaction work.

Current execution note:

- the first concrete Phase B CLAT is now landed in code as `DomainState { graph, notes, next_placeholder_id }`
- `GraphWorkspace` now stores that durable core under `domain: DomainState`
- the first bounded bridge-reduction follow-on slice is complete for workbench-family graph reads
- that completed slice does not close the remaining unknowns above
- those unknowns remain separate future CLATs

---

## 3. Top-Level GraphBrowserApp

| Field | Classification | Why |
| --- | --- | --- |
| `workspace` | `unknown` | Bridge container over mixed domain, workbench, and runtime state. It should be decomposed rather than treated as canonical truth. |
| `services` | `runtime` | Effect boundary for persistence handles and sync command channels. |

### AppServices

| Field | Classification | Why |
| --- | --- | --- |
| `persistence` | `runtime` | Live service handle to persistence infrastructure, not durable state itself. |
| `sync_command_tx` | `runtime` | Live async effect channel. |

---

## 4. GraphWorkspace Field Ledger

### 4.1 Domain-owned

| Field | Why |
| --- | --- |
| `domain` | Bridge container for the landed durable core CLAT. `GraphWorkspace` no longer stores `graph`, `notes`, or `next_placeholder_id` directly. |

### 4.2 Workbench-owned

| Field | Why |
| --- | --- |
| `selected_nodes` | Current workbench selection mirror for the focused graph view. |
| `selected_nodes_by_view` | Per-view selection state. |
| `history_manager_tab` | Tool-surface page selection. |
| `settings_tool_page` | Tool-surface page selection. |
| `show_help_panel` | Workbench UI visibility state. |
| `show_command_palette` | Workbench UI visibility state. |
| `show_radial_menu` | Workbench UI visibility state. |
| `toast_anchor_preference` | Persisted shell preference. |
| `command_palette_shortcut` | Persisted shell preference. |
| `help_panel_shortcut` | Persisted shell preference. |
| `radial_menu_shortcut` | Persisted shell preference. |
| `keyboard_pan_step` | View/workbench-owned camera-control preference storage. Graph subsystem still owns graph-space camera interaction semantics. |
| `keyboard_pan_input_mode` | View/workbench-owned camera-control preference storage. Graph subsystem still owns graph-space camera interaction semantics. |
| `camera_pan_inertia_enabled` | View/workbench-owned camera-control preference storage. Graph subsystem still owns graph-space camera interaction semantics. |
| `camera_pan_inertia_damping` | View/workbench-owned camera-control preference storage. Graph subsystem still owns graph-space camera interaction semantics. |
| `lasso_binding_preference` | Canvas interaction preference. |
| `omnibar_preferred_scope` | Omnibar preference state. |
| `omnibar_non_at_order` | Omnibar preference state. |
| `selected_tab_nodes` | Tab-strip workbench selection. |
| `tab_selection_anchor` | Tab-strip workbench selection anchor. |
| `search_display_mode` | Graph-view/workbench presentation mode. |
| `file_tree_projection_state` | Workbench projection state for file-tree navigation; current inline comment should be reconciled with the reset because this is not durable domain truth. |
| `pending_node_context_target` | Staged workbench context target. |
| `highlighted_graph_edge` | Workbench/UI targeting state. |
| `pending_tool_surface_return_target` | Workbench navigation staging. |
| `pending_workbench_intents` | Workbench-authority intent staging. |
| `pending_open_connected_from` | Pending workbench open action. |
| `pending_open_node_request` | Pending workbench open action. |
| `pending_save_workspace_snapshot` | Pending workbench snapshot action. |
| `pending_save_workspace_snapshot_named` | Pending workbench snapshot action. |
| `pending_restore_workspace_snapshot_named` | Pending workbench snapshot action. |
| `pending_workspace_restore_open_request` | Pending workbench restore follow-up. |
| `pending_unsaved_workspace_prompt` | Pending workbench modal state. |
| `pending_unsaved_workspace_prompt_action` | Pending workbench modal result. |
| `pending_choose_workspace_picker_request` | Pending workbench picker state. |
| `pending_add_node_to_workspace` | Pending workbench snapshot mutation request. |
| `pending_add_connected_to_workspace` | Pending workbench snapshot mutation request. |
| `pending_choose_workspace_picker_exact_nodes` | Pending workbench picker selection payload. |
| `pending_add_exact_to_workspace` | Pending workbench snapshot mutation request. |
| `pending_save_graph_snapshot_named` | Workbench-issued full-graph snapshot request staging. |
| `pending_restore_graph_snapshot_named` | Workbench-issued full-graph snapshot request staging. |
| `pending_restore_graph_snapshot_latest` | Workbench-issued full-graph restore request staging. |
| `pending_delete_graph_snapshot_named` | Workbench-issued full-graph snapshot deletion staging. |
| `pending_detach_node_to_split` | Workbench pane-structure operation staging. |
| `pending_prune_empty_workspaces` | Workbench maintenance action staging. |
| `pending_keep_latest_named_workspaces` | Workbench maintenance action staging. |
| `pending_clipboard_copy` | Workbench copy action staging. |
| `pending_open_note_request` | Workbench navigation request. |
| `pending_open_clip_request` | Workbench navigation request. |
| `pending_keyboard_zoom_request` | Workbench camera action staging. |
| `pending_camera_command` | Workbench camera action staging. |
| `graph_view_layout_manager` | Workbench slot-grid and view arrangement state. |
| `focused_view` | Workbench focus ownership. |
| `pending_history_workspace_layout_json` | Workbench restore payload from undo/redo. |
| `last_session_workspace_layout_hash` | Workbench autosave/change tracking. |
| `last_session_workspace_layout_json` | Workbench autosave/change tracking. |
| `workspace_autosave_interval` | Workbench persistence policy for layout autosave. |
| `workspace_autosave_retention` | Workbench persistence policy for layout autosave. |
| `workspace_activation_seq` | Workbench activation recency tracking. |
| `node_last_active_workspace` | Workbench recency index keyed by node UUID. |
| `node_workspace_membership` | Workbench membership index derived from workspace layouts. |
| `current_workspace_is_synthesized` | Workbench routing/session bookkeeping. |
| `workspace_has_unsaved_changes` | Workbench dirty-state tracking. |
| `unsaved_workspace_prompt_warned` | Workbench dirty-state prompt bookkeeping. |
| `default_registry_lens_id` | Workbench preference/default. |
| `default_registry_physics_id` | Workbench preference/default. |
| `default_registry_layout_id` | Workbench preference/default. |
| `default_registry_theme_id` | Workbench preference/default. |

### 4.3 Runtime-owned

| Field | Why |
| --- | --- |
| `physics` | Live force-layout execution state and tuning, not durable graph truth. |
| `physics_running_before_interaction` | Runtime drag/interaction bookkeeping. |
| `webview_to_node` | Live renderer binding map. |
| `node_to_webview` | Live renderer binding map. |
| `runtime_block_state` | Live runtime backoff/block metadata. |
| `active_webview_nodes` | Runtime mapping/restoration list for active webviews. |
| `active_lru` | Runtime webview resource cache policy. |
| `active_webview_limit` | Runtime resource policy. |
| `warm_cache_lru` | Runtime webview cache policy. |
| `warm_cache_limit` | Runtime resource policy. |
| `is_interacting` | Frame-local interaction state. |
| `drag_release_frames_remaining` | Frame-local post-drag decay state. |
| `wry_enabled` | Runtime backend toggle. |
| `hovered_graph_node` | Frame-local hover state. |
| `pending_switch_data_dir` | Runtime service reconfiguration staging. |
| `pending_wheel_zoom_delta` | Pre-render input buffer. |
| `pending_wheel_zoom_target_view` | Pre-render input buffer target. |
| `pending_wheel_zoom_anchor_screen` | Pre-render input buffer anchor. |
| `graph_view_frames` | Render-pass output cache. |
| `last_workspace_autosave_at` | Runtime timer/bookkeeping state. |
| `egui_state` | Runtime render cache. |
| `egui_state_dirty` | Runtime rebuild flag. |
| `last_culled_node_keys` | Runtime render-culling cache. |
| `memory_pressure_level` | Runtime telemetry sample. |
| `memory_available_mib` | Runtime telemetry sample. |
| `memory_total_mib` | Runtime telemetry sample. |
| `history_recent_traversal_append_failures` | Runtime error counter. |
| `history_preview_mode_active` | Runtime mode flag. |
| `history_last_preview_isolation_violation` | Runtime isolation bookkeeping. |
| `history_replay_in_progress` | Runtime replay bookkeeping. |
| `history_replay_cursor` | Runtime replay bookkeeping. |
| `history_replay_total_steps` | Runtime replay bookkeeping. |
| `history_preview_live_graph_snapshot` | Runtime detached graph copy for preview isolation. |
| `history_preview_graph` | Runtime detached replay graph. |
| `history_last_event_unix_ms` | Runtime subsystem telemetry. |
| `history_last_error` | Runtime operator-facing error state. |
| `history_recent_failure_reason_bucket` | Runtime failure classification. |
| `history_last_return_to_present_result` | Runtime replay summary. |
| `form_draft_capture_enabled` | Runtime/config gate currently sourced from environment. |
| `semantic_index` | Runtime/cache projection of semantic data for physics and lookup. |
| `semantic_index_dirty` | Runtime cache invalidation flag. |

### 4.4 Unknown / unclassified

| Field | Why it is still unresolved |
| --- | --- |
| `views` | Container mixes workbench view state (`id`, `name`, `camera`, `lens`, locks, `dimension`) with runtime-only caches (`local_simulation`, `egui_state`). This should be split into workbench view state plus per-view runtime state. |
| `camera` | Legacy global camera state competes with per-view camera ownership. Storage owner is unclear, and it currently muddies the distinction between Graph-subsystem camera policy and workbench-owned persisted view camera state. |
| `undo_stack` | Snapshot payload mixes domain graph, workbench selection, and workspace layout in one legacy transaction model. |
| `redo_stack` | Same mixed-transaction problem as `undo_stack`. |
| `semantic_tags` | Comment describes runtime tags keyed by `NodeKey`, but reset target expects semantic truth to be durable/domain-owned and identity-stable. |

---

## 5. Nested State Carriers

### 5.1 DomainState

Current source: `graph_app.rs`

| Field | Classification | Why |
| --- | --- | --- |
| `graph` | `domain` | Canonical durable graph truth. |
| `next_placeholder_id` | `domain` | Stable durable-node identity support for placeholder URL creation. |
| `notes` | `domain` | Durable note documents keyed by note identity. |

### 5.2 GraphViewState

Current source: `graph_app.rs`

| Field | Classification | Why |
| --- | --- | --- |
| `id` | `workbench` | Stable view identity in workbench state. |
| `name` | `workbench` | Operator-facing view metadata. |
| `camera` | `workbench` | View-local camera state storage and persistence. This does not contradict Graph-subsystem ownership of graph-camera interaction policy or runtime hydration of live view state. |
| `position_fit_locked` | `workbench` | View-local camera preference/policy storage. |
| `zoom_fit_locked` | `workbench` | View-local camera preference/policy storage. |
| `lens` | `workbench` | View-local presentation/policy choice. |
| `local_simulation` | `runtime` | Local simulation/projection cache, not canonical authored truth. |
| `dimension` | `workbench` | Persisted per-view presentation choice. |
| `egui_state` | `runtime` | Runtime render cache. |

### 5.3 GraphViewSlot

| Field | Classification | Why |
| --- | --- | --- |
| `view_id` | `workbench` | Workbench slot identity link. |
| `name` | `workbench` | Workbench slot label. |
| `row` | `workbench` | Workbench arrangement coordinate. |
| `col` | `workbench` | Workbench arrangement coordinate. |
| `archived` | `workbench` | Workbench visibility/lifecycle state. |

### 5.4 GraphViewLayoutManagerState

| Field | Classification | Why |
| --- | --- | --- |
| `active` | `workbench` | Workbench manager toggle. |
| `slots` | `workbench` | Workbench arrangement authority. |

### 5.5 FileTreeProjectionState

| Field | Classification | Why |
| --- | --- | --- |
| `containment_relation_source` | `workbench` | Projection source selection for a workbench tool surface. |
| `expanded_rows` | `workbench` | Workbench disclosure state. |
| `collapsed_rows` | `workbench` | Workbench disclosure state. |
| `selected_rows` | `workbench` | Workbench selection state. |
| `sort_mode` | `workbench` | Workbench presentation preference. |
| `root_filter` | `workbench` | Workbench filter state. |
| `row_targets` | `workbench` | Derived projection index for a workbench tool surface. |

### 5.6 SelectionState

| Field | Classification | Why |
| --- | --- | --- |
| `nodes` | `workbench` | Workbench selection membership. |
| `order` | `workbench` | Workbench selection ordering. |
| `primary` | `workbench` | Workbench primary selection. |
| `revision` | `workbench` | Workbench selection change tracking. |

### 5.6 RuntimeBlockState

| Field | Classification | Why |
| --- | --- | --- |
| `reason` | `runtime` | Runtime failure classification. |
| `retry_at` | `runtime` | Runtime retry scheduling. |
| `message` | `runtime` | Runtime operator/debug message. |
| `has_backtrace` | `runtime` | Runtime crash/debug metadata. |
| `blocked_at` | `runtime` | Runtime event timestamp. |

### 5.7 UndoRedoSnapshot

| Field | Classification | Why |
| --- | --- | --- |
| `graph` | `unknown` | Domain payload packed into a mixed snapshot format. |
| `selected_nodes` | `unknown` | Workbench payload packed into a mixed snapshot format. |
| `selected_nodes_by_view` | `unknown` | Workbench payload packed into a mixed snapshot format. |
| `highlighted_graph_edge` | `unknown` | Workbench payload packed into a mixed snapshot format. |
| `workspace_layout_json` | `unknown` | Workbench payload packed into a mixed snapshot format. |

---

## 6. Immediate Phase B Targets

Based on this map, the least-ambiguous next structural moves are:

1. Extract `DomainState` around `graph`, `notes`, and related durable graph helpers.
2. Extract `WorkbenchState` around selection, views, slot layout, tool-surface state, snapshot/restore staging, and workbench preferences.
3. Extract `RuntimeState` around renderer bindings, render caches, physics execution state, memory pressure, and history preview execution state.
4. Split `views` into workbench view state plus per-view runtime caches.
5. Replace `undo_stack` / `redo_stack` with a transaction-shaped model or an explicitly named mixed bridge.
6. Reclassify `semantic_tags` onto stable domain-owned semantic truth or name it as a temporary bridge with deletion criteria.
7. Reconcile the global `camera` field with the per-view camera model and delete one authority.

---

## 7. Unknown-Surface Receipt

Phase A unknowns discovered and explicitly classified:

- `views`
- `camera`
- `undo_stack`
- `redo_stack`
- `semantic_tags`

These are now in scope. None should be treated as background cleanup.
