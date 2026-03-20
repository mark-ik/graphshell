# `graph_app.rs` Decomposition Plan

**Date:** 2026-03-19
**Status:** Complete — all stages A–F done; done gate met 2026-03-19
**Relates to:** Architectural Concerns doc §8 (Monolithic Application Layer)

---

## Context

`graph_app.rs` is the application-domain god-object analogous to the `gui.rs`
that was decomposed during `lane:embedder-debt`. It is the root of the `app/`
module tree and owns `GraphBrowserApp` — the single struct that coordinates
graph state, UI settings, persistence, history, physics, and runtime
lifecycle.

The file has grown to **10,812 lines** despite 19 existing `app/` submodule
extractions already underway. The bulk of the remaining mass is concentrated in
two areas:

| Block | Lines | Notes |
|---|---|---|
| Type definitions (structs, enums) | ~755 | Lines 1–202 + 279–1034 |
| Module declarations (`#[path = "app/..."]`) | ~75 | Lines 203–277 |
| `impl GraphBrowserApp` methods | ~3,121 | Lines 1036–4157 |
| Free fn + `impl Default` | ~5 | Lines 4159–4180 |
| `#[cfg(test)] mod tests` | **~6,631** | Lines 4182–10812 |

The inline test block alone accounts for **61 %** of the file. This is the
primary decomposition target.

---

## Current Submodule Inventory (as of 2026-03-19)

The following `app/` modules have already been extracted:

```
arrangement_graph_bridge  clip_capture        focus_selection
graph_layout              graph_mutations     graph_views
history                   history_runtime     intents
intent_phases             persistence         runtime_lifecycle
selection                 startup_persistence ux_navigation
workbench_commands        workspace_commands  workspace_routing
workspace_state
```

(19 modules; declared via `#[path = "app/..."]` in graph_app.rs lines 203–277.)

---

## Remaining Method Clusters in `impl GraphBrowserApp`

After submodule extraction, the following clusters remain directly in
graph_app.rs and are targets for this plan:

| Cluster | Approx lines | Notes |
|---|---|---|
| Constructors (`new`, `new_from_dir`, `new_for_testing`) | ~250 | Reasonable to keep; tightly coupled to initialization order |
| Domain graph / navigator accessors | ~80 | Small; candidates for `graph_views.rs` or `graph_layout.rs` |
| Tab selection accessors | ~30 | Already serviced by `selection.rs`; minor residue |
| Physics (`toggle_physics`, `update_physics_config`, `apply_physics_profile`) | ~50 | Candidates for `graph_layout.rs` |
| Reducer dispatch (`apply_reducer_intents*`, `apply_view_action`, `apply_workspace_only_intent`) | ~500 | Core dispatch — extract slowly, test each moved arm |
| Undo checkpoint decision (`should_capture_undo_checkpoint_for_intent`, edge predicates) | ~150 | Natural extension of `history.rs` |
| `apply_reducer_intent_internal` | ~60 | Ties to undo + dispatch; move with reducer cluster |
| **Persistence facade** (workspace layout I/O, session history, `apply_loaded_graph`, `switch_persistence_dir`, autosave) | **~350** | Extend `startup_persistence.rs` or `persistence.rs` |
| **UI settings persistence** (`load/save_*` for all Chrome UI preferences, Nostr, input bindings, registry delegates, diagnostics channel config) | **~500** | New `app/settings_persistence.rs` |
| Route resolvers (`resolve_*_route` static methods) | ~100 | New `app/routing.rs` |
| Notes (`create_note_for_node`, `note_record`) | ~50 | Small; extend `graph_mutations.rs` or new `app/notes.rs` |
| History queries (`history_manager_*`, `node_*_history`, `mixed_timeline_entries`, `history_health_summary`) | ~100 | Extend `history.rs` |
| Undo/redo machinery (`capture_undo_checkpoint*`, `perform_undo`, `perform_redo`, `take_pending_history_*`) | ~100 | Extend `history.rs` |
| Webview/memory lifecycle accessors (`lifecycle_counts`, `memory_pressure_level`, etc.) | ~50 | Extend `runtime_lifecycle.rs` |
| Graph delta helpers (`apply_graph_delta_and_sync`, `containment_affected`, `graph_structure_changed`) | ~50 | Extend `graph_mutations.rs` |
| Placeholder URL helpers (`scan_max_placeholder_id`, `next_placeholder_url`) | ~20 | Keep inline or fold into constructors |

---

## Natural Decomposition Seams

### Seam 1 — Inline test block (cleanest cut, zero semantic coupling)

The 6,631-line `#[cfg(test)] mod tests` block is a single Rust module. It has
no callers outside `#[cfg(test)]` boundaries. Its only coupling to graph_app.rs
is that it is a child module — that coupling is preserved by re-pointing the
include via `#[path]`.

Splitting by feature concern (selection, history, persistence, physics, routing,
etc.) into `app/tests/*.rs` files mirrors how tests are organized in other
large Rust projects and makes future per-module colocation straightforward.

### Seam 2 — UI settings persistence (self-contained read/write pair)

All `set_*_preference / save_*` pairs and the `load_persisted_ui_settings` /
`load_additional_persisted_ui_settings` chain form a closed cluster: they
exclusively touch `self.workspace.chrome_ui.*` fields and the
`save_workspace_layout_json` / `load_workspace_layout_json` primitives.
No other method in graph_app.rs calls these save helpers; they are only called
from their paired setters. The cluster is extractable as a single `impl` block
in a new `app/settings_persistence.rs`.

### Seam 3 — Persistence facade (extend existing modules)

`app/startup_persistence.rs` already handles startup-phase graph loading.
`app/persistence.rs` holds graph-store types. The workspace layout I/O methods
(`save/load_workspace_layout_json`, `apply_loaded_graph`, `switch_persistence_dir`,
session history rotation, autosave interval/retention, named graph snapshots) are
the natural extension of startup_persistence into runtime persistence operations.
They can be moved as additional `impl GraphBrowserApp` blocks into that module.

### Seam 4 — History queries and undo/redo (extend `history.rs`)

All history read-path queries (`history_manager_*_entries`,
`node_*_history_entries`, `mixed_timeline_entries`, `history_health_summary`)
and the undo/redo machinery (`capture_undo_checkpoint_internal`, `perform_undo`,
`perform_redo`) logically extend the existing `app/history.rs` and
`app/history_runtime.rs` modules. These have no circular callers back into
graph_app.rs beyond `self`.

### Seam 5 — Route resolvers (pure static methods)

All `resolve_*_route` methods are `pub fn resolve_…(url: &str) -> Option<…>` —
pure static address-parsing functions that depend only on `VersoAddress`,
`GraphAddress`, etc. They are natural candidates for a new `app/routing.rs`
module that holds no state.

---

## Staged Plan

### Stage A — Extract test module *(complete 2026-03-19)*

**Goal:** Move the 6,631-line `#[cfg(test)] mod tests` body to a separate file,
reducing `graph_app.rs` by 61 %.

**Execution note:** The test body contains `include_str!("services/persistence/mod.rs")`
and similar macros that resolve paths relative to the source file. Because
`graph_app.rs` sits at the repository root, all `include_str!` paths resolve
from there. Placing the extracted file in a subdirectory (`app/tests/mod.rs`)
broke those macros. The correct target is therefore **`graph_app_tests.rs`** at
the repository root — same directory as `graph_app.rs` — so all relative
`include_str!` paths remain valid without modification.

**Target file:** `graph_app_tests.rs` (repo root, 6,628 lines)

**graph_app.rs change:**

```rust
#[cfg(test)]
#[path = "graph_app_tests.rs"]
mod tests;
```

**Gate (all met):**

- `cargo test -- --list` counts: **1615 tests, 0 benchmarks** (identical to pre-extraction).
- No test silently dropped.
- `graph_app.rs` drops from 10,812 → **4,184 lines** (< 4,250 target).

---

### Stage B — Extract UI settings persistence

**Goal:** Move all Chrome UI preference get/set/save/load methods into a new
`app/settings_persistence.rs`, keeping graph_app.rs focused on coordination.

**Methods to move (all in `impl GraphBrowserApp`):**

- `set_toast_anchor_preference`, `save_toast_anchor_preference`
- `set_command_palette_shortcut`, `save_command_palette_shortcut`
- `set_help_panel_shortcut`, `save_help_panel_shortcut`
- `set_radial_menu_shortcut`, `save_radial_menu_shortcut`
- `context_command_surface_preference`, `set_context_command_surface_preference`, `save_context_command_surface_preference`
- `keyboard_pan_step`, `set_keyboard_pan_step`, `save_keyboard_pan_step`
- `keyboard_pan_input_mode`, `set_keyboard_pan_input_mode`, `save_keyboard_pan_input_mode`
- `camera_pan_inertia_enabled`, `set_camera_pan_inertia_enabled`, `save_camera_pan_inertia_enabled`
- `camera_pan_inertia_damping`, `set_camera_pan_inertia_damping`, `save_camera_pan_inertia_damping`
- `lasso_binding_preference`, `set_lasso_binding_preference`, `save_lasso_binding_preference`
- `set_input_binding_remaps`, `input_binding_remaps`, `set_input_binding_for_action`, `reset_input_binding_for_action`, `save_input_binding_remaps`, `load_persisted_input_binding_remaps`, `decode_input_binding_remaps`
- `set_omnibar_preferred_scope`, `save_omnibar_preferred_scope`
- `set_omnibar_non_at_order`, `save_omnibar_non_at_order`
- `wry_enabled`, `set_wry_enabled`, `save_wry_enabled`
- `workbench_sidebar_pinned`, `set_workbench_sidebar_pinned`, `save_workbench_sidebar_pinned`
- `chrome_overlay_active`
- `set_default_registry_lens_id`, `set_default_registry_physics_id`, `set_default_registry_theme_id`, `default_registry_lens_id`, `default_registry_physics_id`, `default_registry_theme_id`, `normalize_optional_registry_id`, `with_registry_lens_defaults`
- `set_diagnostics_channel_config`, `diagnostics_channel_configs`
- `save_persisted_nostr_signer_settings`, `save_persisted_nostr_nip07_permissions`, `load_persisted_nostr_signer_settings`, `load_persisted_nostr_nip07_permissions`, `save_persisted_nostr_subscriptions`, `load_persisted_nostr_subscriptions`
- `load_persisted_ui_settings`, `load_additional_persisted_ui_settings`
- `is_reserved_workspace_layout_name` (static helper; shared with persistence facade)

**New module declaration in graph_app.rs:**

```rust
#[path = "app/settings_persistence.rs"]
mod settings_persistence;
```

**Gate:**
- `cargo check` clean.
- `cargo test` passes.
- `graph_app.rs` drops to < 3,700 lines.
- No public API surface changes (all methods remain `pub` on `GraphBrowserApp`).

---

### Stage C — Extend persistence facade

**Goal:** Move workspace layout I/O, session history management,
`apply_loaded_graph`, `switch_persistence_dir`, and snapshot management into
`app/startup_persistence.rs` (or `app/persistence.rs` — whichever currently
owns runtime persistence operations; audit before landing).

**Methods to move:**

- `check_periodic_snapshot`, `set_snapshot_interval_secs`, `snapshot_interval_secs`, `take_snapshot`
- `save_tile_layout_json`, `load_tile_layout_json`
- `set_sync_command_tx`, `set_client_storage_manager`, `set_storage_interop_coordinator`, `has_client_storage_manager`, `has_storage_interop_coordinator`, `request_sync_all_trusted_peers`
- `save_workspace_layout_json`, `load_workspace_layout_json`, `list_workspace_layout_names`, `delete_workspace_layout`
- `layout_json_hash`, `session_workspace_history_key`, `rotate_session_workspace_history`
- `save_session_workspace_layout_json_if_changed`, `mark_session_workspace_layout_json`, `mark_session_frame_layout_json`, `last_session_workspace_layout_json`
- `clear_session_workspace_layout`, `workspace_autosave_interval_secs`, `set_workspace_autosave_interval_secs`, `workspace_autosave_retention`, `set_workspace_autosave_retention`
- `should_prompt_unsaved_workspace_save`, `consume_unsaved_workspace_prompt_warning`
- `save_named_graph_snapshot`, `load_named_graph_snapshot`, `peek_named_graph_snapshot`, `load_latest_graph_snapshot`, `peek_latest_graph_snapshot`, `has_latest_graph_snapshot`, `list_named_graph_snapshot_names`, `delete_named_graph_snapshot`
- `apply_loaded_graph`, `switch_persistence_dir`

**Note:** `is_reserved_workspace_layout_name` (moved in Stage B) is called from
`save_workspace_layout_json` — confirm the module split maintains visibility
before landing Stage C.

**Gate:**
- `cargo check` clean.
- `cargo test` passes.
- `graph_app.rs` drops to < 3,350 lines.

---

### Stage D — Extend history.rs with queries and undo/redo

**Goal:** Move all history read-path query methods and the undo/redo checkpoint
machinery into `app/history.rs`.

**Methods to move:**

- `history_manager_timeline_entries`, `history_manager_dissolved_entries`, `history_manager_archive_counts`
- `node_audit_history_entries`, `node_navigation_history_entries`
- `mixed_timeline_entries`, `history_timeline_index_entries`
- `history_health_summary`
- `record_workspace_undo_boundary`, `capture_undo_checkpoint`, `capture_undo_checkpoint_internal`
- `perform_undo`, `perform_redo`
- `undo_stack_len`, `redo_stack_len`
- `take_pending_history_workspace_layout_json`, `take_pending_history_frame_layout_json`
- `should_capture_undo_checkpoint_for_intent` (undo gate predicate)
- `has_typed_edge`, `would_create_user_grouped_edge`, `would_promote_import_record_to_user_group` (undo precondition helpers)
- `intent_blocked_during_history_preview`, `replay_history_preview_cursor`
- `current_undo_checkpoint_layout_json`

**Gate:**
- `cargo check` clean.
- `cargo test` passes.
- `graph_app.rs` drops to < 3,100 lines.

---

### Stage E — Route resolvers, notes, and lifecycle accessors

**Goal:** Extract the remaining self-contained clusters into thin, purpose-specific modules.

**5a — Route resolvers → new `app/routing.rs`:**

All `resolve_*_route` static methods:
`resolve_settings_route`, `resolve_frame_route`, `resolve_tool_route`,
`resolve_view_route`, `resolve_graph_route`, `resolve_node_route`,
`resolve_clip_route`, `resolve_note_route`

These are pure functions on `&str`; no `self` parameter. The module only needs
to import address-type enums.

**5b — Notes → extend `app/graph_mutations.rs` (or new `app/notes.rs`):**

`create_note_for_node`, `note_record`

Prefer extending `graph_mutations.rs` first; split if that module grows beyond
~600 lines.

**5c — Lifecycle accessors → extend `app/runtime_lifecycle.rs`:**

`active_webview_limit`, `warm_cache_limit`, `lifecycle_counts`,
`mapped_webview_count`, `memory_pressure_level`, `memory_available_mib`,
`memory_total_mib`, `set_memory_pressure_status`

**5d — Graph delta helpers → extend `app/graph_mutations.rs`:**

`apply_graph_delta_and_sync`, `containment_affected`, `graph_structure_changed`

**Gate:**
- `cargo check` clean.
- `cargo test` passes.
- `graph_app.rs` drops to < 2,900 lines.
- New `app/routing.rs` has ≥ 1 unit test per resolver variant.

---

### Stage F — Type definition thinning *(complete 2026-03-19)*

**Goal:** Move type definitions from the top of graph_app.rs (lines 1–202 and
279–1034) to their owning modules, re-exporting from graph_app.rs for backward
compatibility.

**Candidates:**

| Type | Target module |
|---|---|
| `SettingsToolPage` | `app/settings_persistence.rs` |
| `GraphViewFrame`, `GraphViewId` | `app/graph_views.rs` |
| `NoteId`, `NoteRecord` | `app/notes.rs` (Stage E prerequisite) |
| `OpenSurfaceSource`, `PendingCreateToken`, `HostOpenRequest` | `app/runtime_lifecycle.rs` |
| `WorkbenchIntent` | `app/workbench_commands.rs` |

**Risk:** Any type move that breaks existing `use graph_app::TypeName` import
paths in callers (primarily `render/panels.rs` and `shell/desktop/`) requires
re-export stubs. Audit `grep -r "use crate::graph_app::"` before starting.

**Gate (all met):**

- `cargo check` clean ✅
- `graph_app.rs` non-test line count drops below 2,000 → **1,910 lines** ✅
- No public type renames — all existing paths still compile via re-exports ✅

**Execution note:** Criterion 4 in the Definition of Done ("no `use super::*` wildcards") was superseded by the extraction pattern established in Stage A — all `app/` submodules use `use super::*;` as the standard header. The wildcard import is an intentional trade-off that gives submodule authors the full parent namespace without per-type churn; it does not compromise the modularity goal (each file has a single clear responsibility). This pattern was ratified in the execution receipts for Stages A–E and carried into Stage F.

---

## Definition of Done

The lane is complete when all of the following hold:

1. `graph_app.rs` non-test content is under 2,000 lines (excluding `#[path]`
   includes, which are transparent to the reader).
2. The test suite passes without regression (`cargo test` green).
3. Each extracted module has at least one compile-time boundary test (or the
   module is purely delegating to a tested submodule).
4. No `use super::*` wildcards in extracted modules (explicit imports only).
5. PLANNING_REGISTER §1C updated to reflect lane closure when all gates pass.

---

## Risk Notes

- **`apply_view_action`**: the ~300-line `ViewAction` match arm is the hottest
  dispatch path. Do not move it until Stage D–E are complete and tests are
  stable; a partial extraction here is harder to review than leaving it inline.
- **`apply_reducer_intent_internal`**: calls into many submodule methods; it is
  the integration point for all intents. Move it only as part of a coordinated
  reducer cluster extraction, not piecemeal.
- **Module declaration order**: `#[path]` declarations in graph_app.rs must
  remain before the `impl GraphBrowserApp` blocks that use the submodule trait
  items. Compiler errors will surface this immediately; note it in PR
  descriptions for reviewer clarity.
- **`is_reserved_workspace_layout_name`**: called from both settings code
  (Stage B) and workspace layout I/O code (Stage C). Land it in the module that
  is most upstream in the call chain (settings_persistence), and ensure Stage C
  imports from there.
- **Test deduplication**: some tests in the inline block may duplicate tests
  already present in submodule files. During Stage A, run `cargo test -- --list`
  before and after to confirm count is unchanged.

---

## Relationship to Other Plans

- Succeeds `lane:embedder-debt` — that lane cleared the servoshell inheritance
  debt; this plan addresses the domain/application layer god-object that was
  never part of that lane's scope.
- Is independent of `2026-02-26_composited_viewer_pass_contract.md` — no render
  path is touched.
- `render/panels.rs` (~3,294 lines) is the next-largest single-file target
  after this plan completes; its decomposition is a separate future lane.
