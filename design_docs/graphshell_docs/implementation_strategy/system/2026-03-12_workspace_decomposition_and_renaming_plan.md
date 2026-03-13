<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Workspace Decomposition and Renaming Plan (2026-03-12)

**Status**: Draft (Execution-ready)

**Purpose**: Decompose the current `GraphWorkspace` monolith into explicit state containers with names that reflect actual ownership. This plan extends, and does not replace, the March 6 foundational reset work.

**Companion docs**:

- `2026-03-06_foundational_reset_graphbrowserapp_field_ownership_map.md`
- `2026-03-06_foundational_reset_implementation_plan.md`
- `2026-03-08_graph_app_decomposition_plan.md`
- `2026-03-08_unified_focus_architecture_plan.md`

---

## 1. Problem Statement

`GraphWorkspace` is no longer a coherent concept.

Today it holds a mixture of:

1. durable domain truth,
2. workbench/session state,
3. runtime UI state,
4. derived caches and indexes,
5. frame-loop authority queues and staged commands.

This is causing two kinds of drift:

- **ownership drift**: fields with different truth models are stored in one container and named as if they shared a common owner,
- **terminology drift**: the word `workspace` increasingly implies semantic ownership over data that is actually node-owned, workbench-owned, or runtime-only.

`workspace.semantic_tags` is the clearest current example: the name suggests workspace-scoped semantic truth, while the intended meaning is canonical node-associated tagging.

---

## 2. Architectural Goal

Move from:

```rust
GraphBrowserApp {
    workspace: GraphWorkspace,
    services: AppServices,
}
```

where `GraphWorkspace` is a mixed state bucket,

to a model where the major state families are explicit:

```rust
GraphBrowserApp {
    domain: DomainState,
    session: WorkbenchSessionState,
    ui_runtime: UiRuntimeState,
    runtime_cache: RuntimeDerivedState,
    authority: RuntimeAuthorityState,
    services: AppServices,
}
```

This is a conceptual target, not a one-shot rewrite requirement. The extraction may proceed incrementally while keeping `GraphBrowserApp` as the façade.

---

## 3. Naming Recommendation

### 3.1 Final naming target

Do **not** replace `workspace` with one new giant synonym.

The correct end state is multiple named containers:

- `DomainState`
- `WorkbenchSessionState`
- `UiRuntimeState`
- `RuntimeDerivedState`
- `RuntimeAuthorityState`

### 3.2 Interim naming recommendation

If an interim single-container rename is needed before full decomposition, the least-wrong replacement for `GraphWorkspace` is:

- `GraphSessionState`

Why:

- it better describes “current live operator/session state”,
- it does not falsely imply semantic ownership of node truth,
- it aligns with the already-emerging distinction between durable domain state and live workbench/session behavior.

Why not use it as the final state model:

- it is still too broad for the actual ownership families,
- it would only rename the monolith, not fix it.

**Recommendation**: postpone the actual type rename until after the first extraction pass lands. Otherwise the codebase pays the rename cost before gaining the ownership clarity.

---

## 4. Target State Containers

### 4.1 `DomainState`

Owns durable semantic truth:

- graph nodes and edges,
- node metadata,
- note documents,
- canonical node tags,
- future durable frame/tile-group graph entities.

Rule:

- if the value should survive independently of a particular workbench/session arrangement, it belongs here.

### 4.2 `WorkbenchSessionState`

Owns live session and arrangement state:

- active views,
- focused view,
- graph-view layout manager,
- workbench selection,
- tab-strip selection,
- named frame recency and frame membership indexes,
- autosave/change tracking for workbench layout.

Rule:

- if the value describes how the operator currently has the environment arranged or selected, it belongs here.

### 4.3 `UiRuntimeState`

Owns transient UI/editor state:

- open command/help/radial surfaces,
- tag panel state,
- graph search draft/restore UI state,
- highlighted edge targeting,
- hovered graph node,
- modal and prompt staging that is purely UI-facing.

Rule:

- if the value exists only because an interactive surface is open, focused, hovered, or staged, it belongs here.

### 4.4 `RuntimeDerivedState`

Owns derived caches and indexes:

- semantic index,
- dirty flags for derived indexes,
- hop-distance cache,
- render cache state (`egui_state`, culling cache),
- memory-pressure telemetry,
- graph-view frame caches,
- search/index-derived projections.

Rule:

- if it can be rebuilt from canonical state plus runtime observations, it belongs here.

### 4.5 `RuntimeAuthorityState`

Owns frame-loop and orchestration authority:

- pending workbench intents,
- pending app commands,
- pending host-create tokens,
- focus authority,
- command/restore queues,
- other staged control-plane carriers.

Rule:

- if it mediates live runtime orchestration rather than representing durable or UI state, it belongs here.

---

## 5. Current `GraphWorkspace` Classification (Updated 2026-03-12)

This section refines the March 6 ownership map based on the current field set in `graph_app.rs`.

### 5.1 Should stay outside the decomposition discussion

Already correctly separated:

- `services: AppServices`
- `workbench_tile_selection: WorkbenchTileSelectionState`

These should remain independent of the former `workspace` monolith.

### 5.2 Move to `DomainState`

These are canonical or should become canonical:

- `domain`
- `semantic_tags` → migrate to node-owned tags inside `DomainState` rather than a parallel map

Notes:

- `semantic_tags` is the highest-priority ownership correction.
- `semantic_index` stays derived; it does **not** move with `semantic_tags`.

### 5.3 Move to `WorkbenchSessionState`

Current fields:

- `views`
- `graph_view_layout_manager`
- `focused_view`
- `camera` (until fully eliminated or reduced to per-view storage)
- `selected_tab_nodes`
- `tab_selection_anchor`
- `search_display_mode`
- `file_tree_projection_state`
- `last_session_workspace_layout_hash`
- `last_session_workspace_layout_json`
- `workspace_autosave_interval`
- `workspace_autosave_retention`
- `workspace_activation_seq`
- `node_last_active_workspace`
- `node_workspace_membership`
- `current_workspace_is_synthesized`
- `workspace_has_unsaved_changes`
- `unsaved_workspace_prompt_warned`
- persisted defaults/preferences that are really workbench/session preferences:
  - `toast_anchor_preference`
  - `command_palette_shortcut`
  - `help_panel_shortcut`
  - `radial_menu_shortcut`
  - `context_command_surface_preference`
  - `keyboard_pan_step`
  - `keyboard_pan_input_mode`
  - `camera_pan_inertia_enabled`
  - `camera_pan_inertia_damping`
  - `lasso_binding_preference`
  - `omnibar_preferred_scope`
  - `omnibar_non_at_order`
  - `default_registry_lens_id`
  - `default_registry_physics_id`
  - `default_registry_theme_id`

### 5.4 Move to `UiRuntimeState`

Current fields:

- `show_command_palette`
- `command_palette_contextual_mode`
- `show_radial_menu`
- `hovered_graph_node`
- `active_graph_search_query`
- `active_graph_search_match_count`
- `active_graph_search_origin`
- `active_graph_search_neighborhood_anchor`
- `active_graph_search_neighborhood_depth`
- `graph_search_history`
- `pinned_graph_search`
- `tag_panel_state`
- `highlighted_graph_edge`

Possible later split:

- graph-search state could become its own `GraphSearchUiState`.

### 5.5 Move to `RuntimeDerivedState`

Current fields:

- `graph_view_frames`
- `hop_distance_cache`
- `egui_state`
- `egui_state_dirty`
- `last_culled_node_keys`
- `memory_pressure_level`
- `memory_available_mib`
- `memory_total_mib`
- `semantic_index`
- `semantic_index_dirty`
- `suggested_semantic_tags`

Important note:

- `suggested_semantic_tags` is not canonical truth, but it is not just raw UI state either. Treat it as a derived/background-surfaced semantic hint cache.

### 5.6 Move to `RuntimeAuthorityState`

Current fields:

- `pending_workbench_intents`
- `pending_app_commands`
- `pending_host_create_tokens`

And any focus-authority carriers that are still nested elsewhere should converge here when practical.

### 5.7 Remain explicit runtime subsystem state for now

These are valid runtime-only families but may deserve their own nested structs instead of one top-level bucket:

- history preview/replay fields
- form draft capture flag
- any remaining runtime block / webview policy / physics live state fields not shown in the current slice

Recommended nested families:

- `HistoryRuntimeState`
- `RenderRuntimeState`
- `ViewerRuntimeState`

---

## 6. Highest-Priority Ownership Corrections

### 6.1 Move canonical tags onto nodes

Current problem:

- `semantic_tags` is stored as `HashMap<NodeKey, HashSet<String>>`
- naming implies workspace/session scope
- semantics imply node-owned truth

Target:

- `Node.tags`
- `RuntimeDerivedState.semantic_index` remains derived
- `UiRuntimeState.tag_panel_state` remains transient

This is the first recommended migration because it fixes both terminology and ownership drift.

### 6.2 Split workbench/session state from runtime caches

Current problem:

- `views`, layout state, autosave tracking, and render caches live beside each other

Target:

- `WorkbenchSessionState` for view/layout/session truth
- `RuntimeDerivedState` for render/cache/index projections

### 6.3 Separate UI surface state from orchestration queues

Current problem:

- open panel booleans and queued authority commands are mixed in one container

Target:

- `UiRuntimeState` for what the operator sees/interacts with
- `RuntimeAuthorityState` for how the runtime stages and applies control flow

---

## 7. Migration Plan

### Phase A - Classification lock

1. Add this plan and align it with the March 6 reset docs.
2. Treat new top-level `GraphWorkspace` fields as blocked unless they are classified into one of the target owners.

Done gate:

- no new unclassified state is added to `GraphWorkspace`.

### Phase B - Introduce explicit nested carriers

Inside `GraphWorkspace`, introduce nested state carriers without changing all callsites immediately:

- `session: WorkbenchSessionState`
- `ui_runtime: UiRuntimeState`
- `derived: RuntimeDerivedState`
- `authority: RuntimeAuthorityState`

During this phase, `GraphWorkspace` remains the outer shell.

Done gate:

- new fields land only inside a named nested carrier, not on the outer struct.

### Phase C - Migrate highest-risk families

Recommended order:

1. `semantic_tags` -> node-owned canonical tags
2. graph-search + tag-panel + command/help/radial open state -> `UiRuntimeState`
3. semantic index / hop-distance / egui caches -> `RuntimeDerivedState`
4. pending workbench/app command queues -> `RuntimeAuthorityState`
5. view/layout/autosave families -> `WorkbenchSessionState`

Done gate:

- top-level outer fields shrink materially and the worst ownership mismatches are gone.

### Phase D - Rename outer shell if still needed

If the outer shell still exists after the main extraction:

- rename `GraphWorkspace` -> `GraphSessionState`

Only do this after Phase C is mostly complete. Before then, the rename is churn without clarity.

### Phase E - Optional shell removal

If the explicit carriers are stable enough, `GraphBrowserApp` may hold them directly and the former shell type can disappear.

This is optional. The architectural win comes from explicit ownership, not from deleting one wrapper type.

---

## 8. Testing Strategy

### 8.1 Ownership guard tests

Add or extend contract tests so new code cannot reintroduce mixed ownership casually.

Useful guardrails:

- no direct writes to canonical node tag truth outside reducer paths
- no new top-level mixed state fields on the outer shell
- no UI-only fields inside `DomainState`
- no derived-cache recomputation logic writing canonical truth

### 8.2 Migration seam tests

For each moved family:

- keep behavior tests unchanged where possible
- add one “owner moved but behavior identical” test for the migration seam

Important examples:

- tag add/remove still updates pin sync and semantic index invalidation
- graph search UI still restores correctly after state move
- focus authority queues still reconcile correctly after authority-state extraction

---

## 9. Canonical Recommendation

The correct move is:

1. **decompose first**
2. **rename second**

Do not spend effort renaming `GraphWorkspace` while it still contains mixed ownership families.

The strongest immediate actions are:

- move canonical tags onto nodes,
- introduce explicit nested carriers for session/UI/cache/authority,
- block further top-level field accretion.

That gives Graphshell a state model that matches what the system is already becoming:

- durable domain truth,
- workbench session state,
- UI runtime state,
- derived caches,
- runtime authority/control-plane state.

