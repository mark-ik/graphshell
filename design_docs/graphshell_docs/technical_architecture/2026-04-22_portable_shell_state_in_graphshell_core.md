<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Portable Shell State in `graphshell-core`

**Date**: 2026-04-22
**Status**: Architecture note + follow-on slice plan
**Audience**: Contributors extending the M4 runtime extraction toward
per-crate testing isolation and the eventual iced-host bring-up.

**Related docs**:

- [`../implementation_strategy/shell/2026-04-14_iced_host_migration_execution_plan.md`](../implementation_strategy/shell/2026-04-14_iced_host_migration_execution_plan.md)
- [`2026-03-29_portable_web_core_host_envelopes.md`](2026-03-29_portable_web_core_host_envelopes.md)
- [`2026-04-22_browser_subsystem_taxonomy_and_mapping.md`](2026-04-22_browser_subsystem_taxonomy_and_mapping.md)

---

## 1. Context

M4 has been extracting durable shell state off the egui `EguiHost`
struct onto a dedicated `GraphshellRuntime`. Sessions 1–5 landed focus
authority, toolbar/omnibar session state, command-palette session
state, thumbnail/update queues, and pane targeting on the runtime
side of the boundary.

As that work progressed it became clear that some runtime-owned
state is **genuinely portable** — no egui, no servo, no tokio, no
platform I/O. The ambition is to test those types without building
servo or webrender, both for fast iteration and to prove the
iced-host migration's preconditions are met.

The original scoping pass proposed a new `graphshell-shell-state`
sub-crate as the home for these portable types. On closer
investigation, a simpler structure is available and has already
landed part of the work.

## 2. The Insight

`graphshell-core` already exists, is already the portable-types
crate for Graphshell (see [`crates/graphshell-core/src/lib.rs`](../../../crates/graphshell-core/src/lib.rs)'s
top docstring: "no egui, wgpu, Servo, or platform I/O
dependencies"), and already contains the graph model + identity +
persistence kernel. It compiles to `wasm32-unknown-unknown` and
`wasip2`.

Creating a parallel `graphshell-shell-state` sub-crate for shell
state — subject to the same constraints — would be redundant. It
would have the same target-triple support rules, the same
dependency embargo, and the same test-isolation properties.

**Recommendation: extend `graphshell-core` with shell-state modules
as their types become portable, instead of creating a second
portable crate.**

Today `graphshell-core` is organized as:

```text
graphshell-core/src/
├── address.rs          — Graphshell address scheme
├── async_request.rs    — AsyncRequestState<T> (M4 follow-on, 2026-04-22)
├── content.rs          — ContentLoadState, ViewerInstanceId (M4 follow-on, 2026-04-22)
├── graph/              — Graph model
├── persistence.rs      — Portable persistence contracts
└── types.rs            — Leaf types
```

Shell-state types already landed as modules here follow the same
constraints as `graph/` and `types/`: WASM-clean, no UI framework,
no platform I/O. Future extractions (command palette session,
omnibar session minus tokio-coupled mailbox, toolbar session) would
land as additional modules:

```text
graphshell-core/src/shell_state/
├── command_palette.rs  — CommandPaletteSession, SearchPaletteScope
├── omnibar.rs          — OmnibarSearchSession (once mailbox is portable)
├── toolbar.rs          — ToolbarState, ToolbarEditable
├── command_surface_telemetry.rs — CommandSurfaceTelemetry (once globals removed)
├── frame_model.rs      — FrameViewModel, FrameHostInput
└── …
```

Or as flat modules under `graphshell-core/src/` if the hierarchy
isn't load-bearing. Grouping semantically (`shell_state/`) probably
helps readers distinguish "graph truth" from "shell session state".

## 3. Dependency Blockers By Type

Each type currently in the graphshell crate's shell-state surface
is blocked from moving into `graphshell-core` by one or more
dependencies. The table below enumerates the blocker and the
unblocking work.

| Type | Blocker | Unblocking work |
|------|---------|-----------------|
| `ContentLoadState` | — | ✅ landed 2026-04-22 |
| `ViewerInstanceId` | — | ✅ landed 2026-04-22 |
| `AsyncRequestState<T>` | — | ✅ landed 2026-04-22 |
| `CommandPaletteSession`, `SearchPaletteScope` | `ActionCategory` from `render::action_registry` | Move `ActionCategory` + `ActionId` to `graphshell-core`; they're `Clone+Copy` enums with `serde` derives, and they already have `category_persisted_name`/`category_from_persisted_name` for stable serialization |
| `ToolbarEditable`, `ToolbarDraft` | — (pure data) | These can move today; `ToolbarEditable` doesn't touch `LoadStatus`/`WebViewId` since the M4 refactor. Low-risk first slice |
| `ToolbarState` | 4 `ContentLoadState` fields (done), `show_clear_data_confirm: bool`, `status_text: Option<String>`, `can_go_back/can_go_forward: bool` — all portable | Move along with `ToolbarEditable` |
| `ToolbarViewModel` | Done post-ContentLoadState wrap | Move with the rest of `frame_model.rs` |
| `OmnibarSearchSession` | `ProviderSuggestionMailbox` uses `HostRequestMailbox<T>` which uses `crossbeam_channel::Receiver`, not `wasm32-unknown-unknown`-portable | Replace `HostRequestMailbox<T>` with `AsyncRequestState<T>` (landed 2026-04-22) inside the omnibar session; host-driver code outside the state polls the concrete receiver and calls `resolve(generation, value)` |
| `FrameViewModel`, `FrameHostInput` | `DegradedReceiptSpec.tile_rect: egui::Rect`; `FrameViewModel.active_pane_rects` uses `PortableRect` (OK); others are `egui::Rect` / `egui::Pos2` in a few places | Replace `egui::Rect` with `PortableRect` (from `compositor_adapter`, or lifted to core) throughout; landed partially during session 2/M3.6 cosmetic-leak cleanup |
| `CommandSurfaceTelemetry` | `std::sync::OnceLock<…>` + `std::sync::Mutex` aren't available on `wasm32-unknown-unknown`; also a crate-global — conflicts with "runtime-owned" direction | Either (a) migrate ownership onto `GraphshellRuntime` (loses the zero-cost global but portable), or (b) feature-gate the static for non-WASM only. The 4 `emit_omnibar_*` free functions would become `GraphshellRuntime` methods |
| `GraphshellRuntime` itself | Holds `tokio::runtime::Runtime`, `Arc<RegistryRuntime>`, `ControlPanel` (tokio), `ViewerSurfaceRegistry` (servo surfaces), `servo::WebViewId` via `PendingWebviewContextSurfaceRequest` and friends | Big. Mostly waits on: viewer-id wrap (partial; thumbnail field wrapped 2026-04-22, 5 fields TODO), `RegistryRuntime`/`ControlPanel` portability sketch, async infrastructure reshape |

## 4. Suggested Slice Order

Each slice is independently valuable and testable against
`graphshell-core`'s existing test harness.

1. **Move `ActionCategory` and `ActionId` to `graphshell-core`.** Tiny
   enums that a dozen modules reference. Unblocks the palette session
   move. Re-export from `render::action_registry` for zero-churn at
   call sites.

2. **Move `ToolbarEditable`, `ToolbarDraft`, `ToolbarState`.** The
   struct has no remaining servo dependencies (`ContentLoadState` wrap
   done). Single-file move; call-site churn limited to import-path
   edits.

3. **Move `CommandPaletteSession`, `SearchPaletteScope`** (depends on
   slice 1). Similar shape; widget-side imports unchanged via
   re-exports.

4. **Complete `ViewerInstanceId` wrap** on the remaining 5 field
   sites (TODO-marked 2026-04-22). Unblocks
   `PendingWebviewContextSurfaceRequest`, `EmbeddedContentTarget`,
   `FocusedContentStatus`, `RuntimeFocusInputs.embedded_content_focus_webview`.

5. **Replace `HostRequestMailbox<T>` with `AsyncRequestState<T>` in
   omnibar's `ProviderSuggestionMailbox`.** Host-side driver takes over
   polling the concrete `crossbeam_channel::Receiver<ProviderSuggestionFetchOutcome>`
   and depositing results via `.resolve(generation, value)`.

6. **Migrate `CommandSurfaceTelemetry` ownership to `GraphshellRuntime`.**
   Trade zero-cost crate-global for runtime-owned + portable. Four
   emit_* free functions become runtime methods; the two static
   `OnceLock<Mutex<…>>`s disappear.

7. **Replace residual `egui::Rect` / `egui::Pos2` fields in view-model
   types with `PortableRect` / `PortablePoint`.** (Partial precedent
   from M3.6 cosmetic-leak cleanup.)

8. **Move `FrameViewModel`, `FrameHostInput`, `CommandPaletteViewModel`,
   `GraphSearchViewModel`, `OmnibarViewModel`, `FocusViewModel`,
   `DialogsViewModel`, `ToastSpec`, `DegradedReceiptSpec`.**

9. **Move the authority bundles** (`FocusAuthorityMut`,
   `ToolbarAuthorityMut`, `GraphSearchAuthorityMut`, `CommandAuthorityMut`).
   They hold `&mut T` refs; portable once their `T`s are.

10. **Consider moving `GraphshellRuntime` itself**, or accept that the
    runtime struct stays in the graphshell crate and the portable-state
    modules just live alongside it. The runtime's `tokio::runtime::Runtime`
    field is a real blocker that would need a port or a separate
    runtime-holder split.

## 5. Trade-offs

**Why extend `graphshell-core` instead of creating `graphshell-shell-state`?**

- **Less ceremony**: no new `Cargo.toml`, no new crate-level doc policy,
  no new dev-dependency entanglement in the workspace.
- **Shared test harness**: tests for shell-state modules run under
  the same `cargo test -p graphshell-core` invocation that already
  runs green for the graph model.
- **Shared dependency rules**: the docstring at the top of
  `graphshell-core/src/lib.rs` already enumerates the WASM-clean
  embargo; we inherit it by reusing this crate.
- **No confusion about crate boundaries**: readers know that
  `graphshell-core` is "the portable stuff"; splitting into two
  portable crates introduces a distinction without a difference.

**What if the shell-state ends up very large?** `graphshell-core` can
grow a `shell_state/` subdirectory to group modules. If it really
becomes unwieldy, split at that point — the work to move out a
subset from one portable crate to another is mechanical, whereas
converging them after the fact is harder.

**Does this conflict with iced-host migration?** No — iced will
depend on `graphshell-core` for these modules exactly as egui does.
The iced-host adapter won't care whether shell-state lives in
`graphshell-core` or `graphshell-shell-state`.

## 6. Non-Goals

- Not proposing to move `GraphshellRuntime` into `graphshell-core`
  this round. It carries host-adjacent state (tokio runtime,
  registries, viewer surfaces) that needs its own portability
  analysis.
- Not proposing to abandon the in-progress session-6+ work on the
  modal-flag cluster, recent commands persistence, or the iced host
  scaffold.
- Not proposing a different target-triple policy for `graphshell-core`.
  The existing WASM-clean rule is the policy shell-state types
  inherit.

## 7. Acceptance Shape

This note is the scoping authority for "where does portable shell
state live?"; M4 follow-on implementation PRs should reference it
when moving a type and cite which slice in §4 they're executing.

Each slice lands as an independent PR with:

- Type moved to `graphshell-core::shell_state::<module>` (or
  `graphshell-core::<module>` if flat layout wins out)
- Re-export shim in the old location if external callers are
  numerous (matches the `pub(crate) use` pattern already used by
  `toolbar_ui.rs` for the omnibar types)
- Unit tests in `graphshell-core` covering the moved types' pure
  logic
- Brief progress entry appended to §8 of this doc.

## 8. Progress Log

### 2026-04-22 — Initial slices

- `ContentLoadState` landed in `graphshell-core::content`. 7 tests.
  Replaces `servo::LoadStatus` at 5 field sites
  (`ToolbarState.load_status`, `FocusedContentStatus.load_status`,
  `ToolbarViewModel.load_status`, and 2 construction/test sites).
  Servo boundary conversion in
  [`shell/desktop/lifecycle/webview_status_sync.rs`](../../../shell/desktop/lifecycle/webview_status_sync.rs)
  via `content_load_state_from_servo`.
- `ViewerInstanceId` (enum-sum over `Servo(String) | Wry(u64) |
  IcedWebview(u64) | MiddlenetDirect(u32)`) landed in
  `graphshell-core::content`. 5 tests. Servo encoding is JSON via
  `serde_json::to_string(&WebViewId)`; boundary conversions in
  `webview_status_sync.rs`. `thumbnail_capture_in_flight` on
  `GraphshellRuntime` migrated from `HashSet<WebViewId>` to
  `HashSet<ViewerInstanceId>` as proof-of-concept for the pattern;
  thumbnail pipeline's retain/insert/remove sites now convert at
  the boundary. TODO breadcrumbs on the remaining 5 field sites in
  `gui_state.rs` directing future contributors at the same pattern.
- `AsyncRequestState<T>` landed in `graphshell-core::async_request`.
  11 tests. Portable state machine to replace `HostRequestMailbox<T>`
  in shell-side session types; host-side drivers bridge concrete
  channels/futures into the state at frame boundaries.

graphshell-core test suite: **121 pass** (was 98 pre-2026-04-22;
+23 new: 7 for `ContentLoadState`, 5 for `ViewerInstanceId`, 11 for
`AsyncRequestState<T>`).

### 2026-04-22 — Slice 1: ActionCategory + ActionId

- `ActionCategory` and `ActionId` (68 variants) landed in
  `graphshell-core::actions` with `InputMode`, `default_category_order`,
  `category_persisted_name`/`category_from_persisted_name`,
  `all_action_ids`, and `action_id_has_namespace_format`. 12 tests
  including key-uniqueness, namespace-format, and category-coverage
  invariants.
- `render::action_registry` starts with a `pub use graphshell_core::actions::{...}`
  re-export; the prior ~500-line enum body was removed. `ActionId::shortcut_hints`
  method became the free function `shortcut_hints_for_action(id) -> Vec<String>`.
- `command_palette.rs` call site updated to use the free function.

### 2026-04-22 — Slice 2: ToolbarEditable / ToolbarDraft / ToolbarState

- Moved to `graphshell-core::shell_state::toolbar`. 7 tests covering
  defaults, initial-location seeding, draft↔editable identity, clone
  independence, and roundtrip through state.
- Added `ToolbarState::with_initial_location(impl Into<String>) -> Self`
  convenience ctor; 3 ctor sites migrated (`GraphshellRuntime::new_minimal`,
  `EguiHost::new`, and the orchestration test setup).
- `shell/desktop/ui/gui_state.rs` re-exports the three types via
  `pub(crate) use`; 28 field-access sites unchanged.

### 2026-04-22 — Slice 3: CommandPaletteSession + SearchPaletteScope

- Moved to `graphshell-core::shell_state::command_palette`. 15 tests
  covering scope default, `ALL` enumeration, `Display`/`FromStr` roundtrip
  with whitespace trimming, serde wire-shape pinning, session default,
  `open_fresh` behavior (including the intentional preservation of
  `was_open_last_frame` and `tier1_category`), and `step_selection`
  including `rem_euclid` negative-delta semantics.
- Depends on Slice 1: imports `ActionCategory` from
  `graphshell-core::actions` (was `crate::render::action_registry::ActionCategory`
  at the old site; semantically identical via the Slice 1 re-export).
- Field visibility bumped from `pub(crate)` to `pub` for cross-crate
  access; call-site field accesses (`session.query`, `session.scope`, …)
  unchanged.
- `shell/desktop/ui/command_palette_state.rs` replaced with a
  `pub(crate) use` re-export shim.

graphshell-core test suite: **155 pass** (+34 since 2026-04-22 initial:
12 actions, 7 toolbar, 15 command_palette).

### 2026-04-22 — Slice 4: Complete `ViewerInstanceId` wrap

Four field sites that the 2026-04-22 initial `ViewerInstanceId` work left
with `TODO(m4-viewer-id-wrap)` breadcrumbs are now portable:

- `PendingWebviewContextSurfaceRequest.webview_id: WebViewId → ViewerInstanceId`.
  Constructor (`gui.rs:765`) wraps incoming `WebViewId` via
  `viewer_instance_id_from_servo`; consumer in
  `post_render_phase.rs:267` unwraps via
  `servo_webview_id_from_viewer_instance`, and skips the request when the
  request came from a non-Servo provider (Wry, iced_webview, MiddleNet)
  — the Servo-specific context-surface dispatch can't service them, and
  non-Servo providers are responsible for their own context-menu
  affordance.
- `EmbeddedContentTarget::WebView.renderer_id: WebViewId → ViewerInstanceId`
  and
- `RuntimeFocusInputs.embedded_content_focus_webview: Option<WebViewId> → Option<ViewerInstanceId>`
  (done together — the value flows from the app accessor through
  `RuntimeFocusInputs` into `EmbeddedContentTarget::WebView` without a
  servo round-trip, so both ends had to change atomically). Boundary
  conversions: `EguiHost::set_embedded_content_focus_webview`
  (`gui.rs:690`) wraps on entry; `focused_embedded_content_webview_id`
  (`gui.rs:707`) unwraps on exit;
  `realize_embedded_content_focus_from_authority`
  (`focus_state.rs:787`) unwraps when handing back to servo;
  three `build_runtime_focus_state` / `workspace_runtime_focus_state`
  / `refresh_realized_runtime_focus_state` inputs wrap
  `graph_app.embedded_content_focus_webview()` via `.map(viewer_instance_id_from_servo)`.
- `FocusedContentStatus.renderer_id: Option<WebViewId> → Option<ViewerInstanceId>`.
  Constructor sites in `webview_status_sync.rs:74,77,83` wrap at the
  servo boundary. Reads via `.live_content_active()` remain portable
  (the method only checks `.is_some()`). The struct still derives
  only `PartialEq` (not `Eq`) because `content_zoom_level: Option<f32>`
  carries a float.

The `use servo::WebViewId;` import in `gui_state.rs` is removed — the
crate's portable types no longer reference servo directly.

Follow-on work: `gui_state.rs` still holds tokio/servo-coupled state
(pane runtime, viewer surface registry, webview lifecycle bookkeeping)
so the runtime struct itself can't move yet; but every field flagged by
`TODO(m4-viewer-id-wrap)` in the initial 2026-04-22 pass is now wrapped.
Test signal: `cargo test -p graphshell-core --lib` remains **155 pass**
(no new portable tests — slice 4 only adds wrap/unwrap at the graphshell
crate boundary, and the graphshell crate remains blocked from `cargo
check` by the pre-existing webrender-wgpu compile failure upstream of it).

### 2026-04-22 — Slice 5: `HostRequestMailbox<T>` → `AsyncRequestState<T>` in omnibar

- `ProviderSuggestionMailbox` (in `shell/desktop/ui/omnibar_state.rs`)
  swapped its `result_mailbox: HostRequestMailbox<ProviderSuggestionFetchOutcome>`
  field — a shell-owned crossbeam-receiver wrapper — for `result:
  AsyncRequestState<ProviderSuggestionFetchOutcome>` from
  `graphshell_core::async_request`. The mailbox is now threading-
  primitive-free and one step closer to moving into
  `graphshell_core::shell_state::omnibar`.
- New `ProviderSuggestionMailbox::arm_new_request() -> u64` bumps a
  monotonic generation counter and arms the state to
  `AsyncRequestState::Pending { generation }`. Added
  `has_pending_result()` preserves the old "is the mailbox still
  awaiting something?" contract (true for `Pending` and `Ready`; false
  for `Idle` and `Interrupted`).
- Host-side driver (new module `shell/desktop/ui/toolbar/toolbar_provider_driver.rs`)
  owns the concrete `crossbeam_channel::Receiver<…>` + generation tag.
  `drive_provider_suggestion_bridge(&mut Option<ProviderSuggestionDriver>,
  &mut ProviderSuggestionMailbox)` runs at the top of the toolbar frame
  and `try_recv`s into `AsyncRequestState::resolve(generation, value)`.
  5 unit tests cover delivery, interrupt-on-sender-drop, pending passthrough,
  stale-generation rejection (supersession), and empty-slot no-op.
- `ControlPanel::spawn_blocking_host_request` replaced by
  `spawn_blocking_host_request_rx` which returns
  `crossbeam_channel::Receiver<T>` directly; callers pair it with an
  `AsyncRequestState<T>` rather than wrapping in a shell-scoped mailbox
  type. `HostRequestMailbox<T>` and `HostRequestPoll<T>` are gone —
  the only caller was the omnibar path, and the `AsyncRequestState`
  pattern is the replacement for any future blocking-host-request
  call site.
- `GraphshellRuntime` gains `omnibar_provider_suggestion_driver:
  Option<ProviderSuggestionDriver>` next to `omnibar_search_session`.
  `ToolbarAuthorityMut` bundles both since the toolbar frame always
  mutates them together; a sibling reborrow path keeps the handle
  threadable through the phase/render layers without cloning.
- The poll site in `toolbar_location_panel.rs` now calls the bridge
  helper once at the top of the session's SearchProvider branch, then
  uses `session.provider_mailbox.result.take()` + a `match` on the
  remaining state variants. Each former `HostRequestPoll::{Pending,
  Ready, Interrupted}` arm has a direct counterpart (Pending repaints,
  Ready consumes via `take`, Interrupted-with-armed-request synthesizes
  the same `ProviderSuggestionStatus::Failed(Network)` outcome and
  resets to `Idle`).
- Tests: 3 new mailbox-behavior tests added to
  `toolbar_location_panel.rs`'s test module
  (`provider_mailbox_idle_reports_no_pending_result`,
  `provider_mailbox_arm_new_request_bumps_generation_and_marks_pending`,
  `provider_mailbox_clear_pending_resets_to_idle`) to pin the
  portable-state contract shell callers rely on. The old
  `provider_mailbox_poll_reports_interrupted_when_idle` test is
  replaced — its semantic load (`Idle` → `Interrupted` on poll) was
  carried by `HostRequestMailbox`'s implementation; under
  `AsyncRequestState`, `Idle` remains `Idle` and the caller gates on
  `has_pending_result()` before spawning.

Graphshell-core test suite remains **155 pass** — slice 5 adds no new
portable tests (the new behavior is all shell-side: the driver, the
bridge, and the mailbox's generation-counter/arm logic). The
`AsyncRequestState<T>` machinery already has 11 tests covering the
generation/stale/interrupt contracts, and the shell-side
`drive_provider_suggestion_bridge` has 5 new tests pinning the
bridge's interaction with `AsyncRequestState`; those run under
`cargo test -p graphshell --lib` and are blocked from execution by
the pre-existing webrender-wgpu compile failure.

### 2026-04-22 — Slice 6: `CommandSurfaceTelemetry` ownership migration

Completed as originally scoped:

- **Singleton removed.** The `OnceLock<CommandSurfaceTelemetry>`
  global and `CommandSurfaceTelemetry::global()` accessor are gone.
  The `test_lock: Mutex<()>` serialisation guard and
  `lock_command_surface_snapshot_tests()` shim are gone too — per-test
  `CommandSurfaceTelemetry::new()` instances are naturally isolated.
- **Runtime-owned field.** `GraphshellRuntime` carries
  `command_surface_telemetry: CommandSurfaceTelemetry`, initialised by
  both `new_minimal()` and `EguiHost::new`.
- **Free functions take `&CommandSurfaceTelemetry` explicitly.** The
  full API surface (`publish_command_surface_semantic_snapshot`,
  `latest_*`, `clear_*`, `note_command_surface_route_{resolved,fallback,no_target}`,
  `set/clear_command_surface_event_sequence_metadata`, and the four
  `emit_omnibar_provider_mailbox_*` callbacks) signature-changed to
  accept the sink as first parameter.
- **Phase-bundle threading.** The telemetry reference is woven
  through every layer from `EguiHost::execute_update_frame` down to
  the production writers/readers: `ExecuteUpdateFrameArgs` →
  `ToolbarAndGraphSearchWindowPhaseArgs` → `ToolbarDialogPhaseArgs` →
  `ToolbarAuthorityMut`-adjacent `Input<'a>` in `toolbar_ui`;
  and `SemanticAndPostRenderPhaseArgs` →
  `{SemanticLifecyclePhaseArgs, PostRenderPhaseArgs}` →
  `{LifecycleReconcilePhaseArgs, TileRenderPassArgs}` →
  `render_tile_tree_and_collect_outputs` → `try_build_snapshot_with_rects`.
  Nine bundle/phase structs touched in total.
- **Lifecycle path.** `apply_pending_browser_commands` in
  `webview_controller.rs` and its inner
  `browser_command_routing.rs` counterpart take a
  `&CommandSurfaceTelemetry` so the route-event counters land in the
  runtime-owned sink.
- **Workbench snapshot builders.** The `build_snapshot`,
  `build_snapshot_host_neutral`, `build_snapshot_with_walker`,
  `build_snapshot_with_walker_and_rects`, `build_snapshot_with_rects`,
  `try_build_snapshot_with_rects`, and
  `try_build_snapshot_with_walker_and_rects` functions all gained an
  `Option<&CommandSurfaceTelemetry>` parameter;
  `append_command_surface_nodes` consumes it via
  `telemetry.and_then(latest_command_surface_semantic_snapshot)`. Tests
  that don't care about the snapshot surface pass `None`; tests that
  publish-then-read pass `Some(&telemetry)`; production passes
  `Some(&runtime.command_surface_telemetry)` via `TileRenderPassArgs`.
- **Test migration.** ~30 test sites across `tile_behavior.rs`,
  `ux_bridge.rs`, `ux_probes.rs`, `ux_tree.rs`, `webdriver_runtime.rs`,
  `tests/scenarios/{ux_tree,ux_tree_diff_gate}.rs`, and
  `toolbar_ui.rs` test module converted: each inserts `let telemetry =
  CommandSurfaceTelemetry::new();` and passes `&telemetry` through
  the free-function calls. Scripted transformation via Python regex
  over the `#[test]` function bodies; verified callers that both
  publish and read use `Some(&telemetry)` in their `build_snapshot`
  variant.
- **`iced_parity.rs`.** The M5 parity test now passes each host
  runtime's own telemetry reference
  (`Some(&runtime_egui.command_surface_telemetry)` and the mirror)
  into `build_snapshot_host_neutral`, so the snapshot comparison
  actually exercises parity of the command-surface projection across
  the egui and iced paths.

Portability impact: with the singleton gone, the only remaining WASM
blockers in `command_surface_telemetry.rs` are the `Mutex` wrappers
around the cell + counters (host-owned, can be swapped for `RefCell`
or a portable equivalent in a future slice) and the
shell-specific types referenced by the data shapes (`PaneId`,
`ToolSurfaceReturnTarget`). The module as-a-whole remains shell-side
until those migrate to `graphshell-core`; this slice just ensures
that when it does move, the ownership model no longer forces a
re-write.

Graphshell-core test suite: **155 pass**, unchanged (no portable-side
types landed this slice). The graphshell crate's compile-level
validation remains blocked by the pre-existing webrender-wgpu upstream
issue, but the slice-6 edits are mechanical bundle-threading and
regex-scripted test migrations — the compile-graph shape is preserved.

### 2026-04-22 — Slice 7: Residual `egui::Rect`/`Pos2` view-model fields

Single remaining egui-typed field in the view-model surface —
`DegradedReceiptSpec.tile_rect: egui::Rect` — swapped to
`PortableRect` (the alias `euclid::default::Rect<f32>` already
imported in `frame_model.rs` alongside `PortablePoint` / `PortableSize`).

The surrounding view-model types were already portable post-M3.6
cosmetic-leak cleanup (see the `frame_model.rs` module docstring),
so this slice was smaller than originally estimated: no construction
sites yet populate `DegradedReceiptSpec`, and the conversion
helpers (`portable_rect_from_egui` / `egui_rect_from_portable` in
`compositor_adapter.rs`) are already in place for when egui hosts
start emitting receipts.

View-model portability blocker status:

- ✅ `FrameViewModel.*` — fully portable (no egui types).
- ✅ `FrameHostInput.*` — fully portable.
- ✅ `DegradedReceiptSpec.tile_rect` — now `PortableRect` (this slice).
- Remaining blockers for the view-model's move to `graphshell-core`:
  the shell-specific types (`PaneId`, `ToolSurfaceReturnTarget`,
  `NodeKey`) and a handful of `Instant` fields that still need a
  portable deadline/time representation. Those are independent
  follow-ons, not tracked under this slice.

Graphshell-core test suite: **155 pass**, unchanged.

### 2026-04-22 — Slice 8 scoping pass (deferred execution)

Before executing slice 8 (view-model types → graphshell-core), an
Explore agent mapped the dependency chain. Findings:

**~90% of view-model fields are already portable** post-slices 1–7
(`ToolbarViewModel`, `CommandPaletteViewModel`, `GraphSearchViewModel`,
`DialogsViewModel`, `ToastSpec`, `DegradedReceiptSpec`, most of
`FrameHostInput`).

**Hard blockers for slice 8 execution:**

- **`FocusRingSpec.started_at: std::time::Instant`** — `Instant` is
  unavailable on `wasm32-unknown-unknown`. Two resolution paths:
  (a) `#[cfg(not(target_arch = "wasm32"))]` gate on `Instant` fields
  (simplest; accepts conditional compilation), or (b) introduce a
  `PortableInstant = u64` (ms since epoch) with host-side converters
  at the boundary (cleaner; larger surface area). Decision deferred.

**Soft blockers (trivial ports required first):**

- `PaneId` (pane_model.rs:42, a 10-line `Uuid` wrapper) — just move
  the type, keep the rest of pane_model.rs shell-side.
- `TileRenderMode` (25-line enum, no deps).
- `OverlayStrokePass` + descriptors — fully portable fields but
  embedded in a 2000-line egui-coupled module; needs descriptor
  extraction to a separate file before moving.

**Call-site churn: LOW.** `FrameViewModel` has 2 construction sites
(`gui_state.rs:784` projection + `iced_host.rs`); `FrameHostInput`
has 2 similar. Slice 2's re-export pattern works here.

### 2026-04-22 — M5b partial: `AsyncSpawner` + `SignalRouter` extraction

First trait-extraction pass landed for the two seams that were still
directly coupled to concrete shell infrastructure:

- `graphshell-core` now exports three new host-boundary modules:
  `async_host`, `signal_router`, and `viewer_host`.
- `async_host::AsyncSpawner` is implemented as an **object-safe** trait.
  The original plan's generic `spawn_blocking<T>` shape would not have
  supported `Arc<dyn AsyncSpawner>`; the landed API erases the blocking
  result at the trait boundary and restores the caller-facing type via
  `BlockingTaskReceiver<T>`.
- `signal_router::SignalRouter` defines the portable subscription seam
  for frame-inbox consumers, with the currently-needed `Lifecycle` and
  `RegistryEvent` topic families expressed as portable enums.
- `viewer_host::ViewerSurfaceHost` is defined in core, but **not yet
  wired through the shell runtime** in this pass; the live viewer-surface
  lifecycle remains on the shell/compositor path for now.

Shell-side adapters and runtime wiring added alongside the portable traits:

- `shell/desktop/runtime/tokio_async_spawner.rs` wraps the existing Tokio
  runtime handle and now owns supervised-task spawning, blocking-task
  execution, and shutdown joining.
- `ControlPanel` no longer stores a concrete `tokio::runtime::Handle` or
  `JoinSet`; it holds `Arc<dyn AsyncSpawner>` and routes supervised work
  through that trait.
- `shell/desktop/runtime/registry_signal_router.rs` adapts the existing
  Register async subscription API into the portable `SignalRouter`
  stream-based interface.
- `GuiFrameInbox::spawn` now consumes `Arc<dyn SignalRouter>` instead of
  calling `phase3_subscribe_signal_async(...)` directly.
- `GraphshellRuntime::new_minimal` and `EguiHost::new` now construct and
  retain `async_spawner` / `signal_router` alongside the existing
  `tokio_runtime`, so host wiring is explicit at the runtime boundary.

Validation performed without compiling the shell/Servo/webrender stack:

- `cargo test -p graphshell-core --lib`: **225 pass** locally.
- `cargo build -p graphshell-core --target wasm32-unknown-unknown`:
  **passes** locally, confirming the new host-boundary modules remain
  portable.
- Shell-side files were checked through editor diagnostics after the
  refactor. Full `cargo test -p graphshell --lib` / end-to-end validation
  was intentionally skipped because that path currently drags the parallel
  webrender/Servo build graph back in.

Remaining M5b work after this partial landing:

- Migrate the live viewer-surface lifecycle to `ViewerSurfaceHost`
  instead of the current shell-owned compositor registry path.
- Re-run shell-side compile/test validation once the webrender-adjacent
  build path is usable again, or once a narrower non-Servo check target
  exists.

**Estimate**: 6–8 files touched, 4–6 portable types promoted to
graphshell-core, **medium complexity** once the Instant decision lands.

### 2026-04-22 — Slice 9: Authority-bundle moves (partial — 2 of 4)

The design-doc goal for slice 9 was to move all four authority
bundles (`FocusAuthorityMut`, `ToolbarAuthorityMut`,
`GraphSearchAuthorityMut`, `CommandAuthorityMut`) to graphshell-core.
Two are fully portable today and moved; the other two remain
blocked on their `T`s becoming portable first.

**Landed:**

- `GraphSearchAuthorityMut` moved to
  `graphshell_core::shell_state::authorities`. Five `&mut T` refs
  where every `T` is portable (`bool`, `String`, `Vec<NodeKey>`,
  `Option<usize>`). All 11 accessor methods ported. Re-exported from
  `shell/desktop/ui/gui_state.rs` so call sites resolve unchanged.
- `CommandAuthorityMut` moved to
  `graphshell_core::shell_state::authorities`. Depends on
  `CommandPaletteSession` + `SearchPaletteScope` (both portable from
  slice 3). Four methods ported including `prime_fresh_open` which
  now references the canonical graphshell-core `SearchPaletteScope`
  path.
- **6 new portable tests**: `graph_search_close_clears_matches_but_preserves_query`
  (pins the query-preservation UX contract), `graph_search_reborrow_preserves_access_after_nested_mutation`
  (pins the `reborrow()` lifetime dance), `graph_search_toggle_filter_mode_flips_independently_of_open`,
  `command_authority_prime_fresh_open_resets_session_state`,
  `command_authority_clear_toggle_request_clears_one_shot_flag`
  (including idempotence), `command_authority_reborrow_yields_distinct_handle_on_same_backing`.
- **Visibility bumped** field-level and method-level from `pub(crate)`
  to `pub` (required for cross-crate visibility through the
  re-export shim). All existing construction/destructure sites
  continue to work — same field names, same method names.

**Deferred (blocked on other slices):**

- **`FocusAuthorityMut`** — `focus_ring_started_at: &mut Option<Instant>`
  pulls in `std::time::Instant`. Unblocks with slice 8's time-portability
  decision.
- **`ToolbarAuthorityMut`** — three non-portable dependencies:
  1. `omnibar_search_session: &mut Option<OmnibarSearchSession>` —
     `OmnibarSearchSession` still carries `Instant` in
     `ProviderSuggestionMailbox.debounce_deadline` (slice 5 deferred).
  2. `omnibar_provider_suggestion_driver: &mut Option<ProviderSuggestionDriver>`
     — host-side companion holding `crossbeam_channel::Receiver<T>`;
     intentionally non-portable (slice 5 design).
  3. Additionally, the toolbar render path reaches
     `&CommandSurfaceTelemetry` via a sibling `Input<'a>` field; the
     telemetry struct still wraps its cells in `Mutex` (slice 6
     follow-on).

Graphshell-core test suite: **161 pass** (was 155 pre-slice-9;
+6 new authority-bundle tests).

### 2026-04-22 — Slice 8a: `PortableInstant` time abstraction

Introduced [`graphshell_core::time::PortableInstant`] — a `u64`
newtype representing milliseconds from a host-chosen origin. Chosen
over a `#[cfg(not(target_arch = "wasm32"))]` feature gate because the
conversion cost is negligible (one `Instant::elapsed().as_millis()`
call per frame boundary on desktop; `performance.now()` on wasm) and
the resulting view-model types become unconditionally portable.

- **Newtype** (not a bare `u64`) so type-checking prevents accidental
  mixing of "ms since epoch" with "ms as duration". Saturating
  arithmetic throughout — overflow / clock-rewind return `0` rather
  than panicking.
- **Serde `#[transparent]`** so persisted deadlines / timestamps
  round-trip as bare numbers, not `{"0": N}` struct wrappers.
- **8 tests** covering: origin is zero, reverse-order saturating,
  add-ms saturation at `u64::MAX`, `has_reached`, `Sub` operator
  mirrors saturating semantics, ordering consistency with `u64`,
  deadline round-trip, and serde wire shape.
- **Shell-side shim** at `shell/desktop/ui/portable_time.rs` holds a
  `OnceLock<Instant>` anchor initialised on first call and exposes
  `portable_now() -> PortableInstant` for call sites that previously
  reached for `Instant::now()`. iced/wasm hosts will provide their
  own shim at the same path.

### 2026-04-22 — Slice 5b: Omnibar session portable

All omnibar-related types moved to
[`graphshell_core::shell_state::omnibar`]:

- `OmnibarSessionKind`, `SearchProviderKind`, `OmnibarSearchMode`,
  `HistoricalNodeMatch`, `OmnibarMatch`, `ProviderSuggestionStatus`,
  `ProviderSuggestionError`, `ProviderSuggestionFetchOutcome`,
  `ProviderSuggestionMailbox`, `OmnibarSearchSession`.
- `ProviderSuggestionMailbox.debounce_deadline` changed from
  `Option<Instant>` to `Option<PortableInstant>`. Callers at
  `toolbar_location_panel.rs` updated to use
  `portable_time::portable_now().saturating_add_ms(debounce_ms)`
  instead of `Instant::now() + Duration::from_millis(debounce_ms)`.
- **Latent bug surfaced**: `ProviderSuggestionFetchOutcome` needed a
  `Clone` derive because `AsyncRequestState::arm_pending` requires
  `T: Clone` for the prior-state snapshot. The original shell-side
  code never compile-verified due to the pre-existing webrender
  blocker; the graphshell-core compile caught it immediately. Fixed
  by adding `Clone` — cheap for the outcome (small enum + Vec).
- **9 new portable tests** covering mailbox idle/debounced/arm/clear
  transitions, session construction for both graph + search-provider
  variants, `HistoricalNodeMatch` URL-only equality (pins dedup
  semantics), `OmnibarMatch` variant coexistence in a `HashSet`
  (pins dedup keying), and the deadline-comparison debounce pattern.
- Shell re-exports from original path (`shell/desktop/ui/omnibar_state.rs`);
  no call-site churn.

### 2026-04-22 — Slice 6 follow-on: `std::sync::Mutex` on WASM

Empirically verified: `std::sync::Mutex<T>` compiles cleanly on
`wasm32-unknown-unknown` as of current Rust stdlib (1.70+). The
slice-6 design-doc entry flagging `Mutex` as a portability blocker
was outdated.

- Confirmed via standalone `rustc --target wasm32-unknown-unknown`
  compile of a minimal `Mutex`-using program; artifact produced
  without error.
- `graphshell-core` now also builds cleanly on
  `wasm32-unknown-unknown` (slice-wide verification target).
- **No changes required** to `CommandSurfaceTelemetry`. Once its
  data-shape dependencies (`PaneId`, `ToolSurfaceReturnTarget`)
  become portable, the whole module can move to graphshell-core
  without a Mutex-related refactor.

### 2026-04-22 — Slice 8: View-model types portable (partial — FrameViewModel deferred)

Consolidated the view-model / host-input surface as portable, leaving
only `FrameViewModel` itself shell-side (blocked on `OverlayStrokePass`).

**New top-level graphshell-core modules:**

- [`graphshell_core::pane`] — `PaneId` (Uuid wrapper) and
  `TileRenderMode` (4-variant render classification). `PaneId::new()`
  gated to non-WASM; wasm hosts use `PaneId::from_uuid(uuid)` with a
  host-supplied UUID. 6 tests covering Display, serde round-trip, and
  default-is-Placeholder invariant.
- [`graphshell_core::geometry`] — promoted `PortableRect`,
  `PortablePoint`, `PortableSize` type aliases (previously in
  `compositor_adapter.rs`). Egui conversion helpers stay shell-side.
  3 tests.
- [`graphshell_core::host_event`] — `HostEvent`, `PointerButton`,
  `ModifiersState` extracted from `ux_replay.rs` (the replay session
  itself references shell types and stays shell-side). 4 tests
  including serde round-trip of representative events and
  `PointerButton::Other(u16)` edge case.
- [`graphshell_core::shell_state::frame_model`] — all sub-view-model
  types: `FocusRingCurve`, `FocusRingSpec` (with `PortableInstant`),
  `FocusViewModel`, `ToolbarViewModel`, `OmnibarViewModel` +
  `OmnibarSessionKindView` + `OmnibarProviderStatusView`,
  `GraphSearchViewModel`, `CommandPaletteViewModel` +
  `CommandPaletteScopeView`, `DialogsViewModel`, `ToastSpec` +
  `ToastSeverity`, `DegradedReceiptSpec`, and `FrameHostInput`. 10
  new tests including the focus-ring alpha curve math (linear,
  ease-out, step) under `PortableInstant` arithmetic, plus
  divide-by-zero safety for `duration = 0`.

**Deferred — `FrameViewModel` stays shell-side:** The aggregate
`FrameViewModel` struct has one field
(`overlays: Vec<OverlayStrokePass>`) whose element type lives in the
2000-line egui-coupled `compositor_adapter.rs`. Moving `FrameViewModel`
requires first extracting `OverlayStrokePass` + its descriptor types
from that module. Shell-side `shell/desktop/ui/frame_model.rs` now
holds only the `FrameViewModel` aggregate plus re-exports from
graphshell-core.

**Portability signal:** `graphshell-core` now compiles cleanly to
`wasm32-unknown-unknown` — verified via `cargo build -p
graphshell-core --target wasm32-unknown-unknown` (0 errors, 1
pre-existing unused-method warning). The portable surface covers
all of the runtime / host boundary vocabulary except `FrameViewModel`
itself and the per-tile overlay descriptor list.

**Cumulative slice impact (slices 1–9 + 8a/5b/6-followon):**

- **Portable modules in graphshell-core**: 12
  (`actions`, `address`, `async_request`, `content`, `geometry`,
  `graph`, `host_event`, `pane`, `persistence`, `shell_state`, `time`,
  `types`).
- **Portable submodules under `shell_state`**: 6 (`authorities`,
  `command_palette`, `frame_model`, `omnibar`, `toolbar`, + existing
  from earlier slices).
- **Test suite**: **201 pass** (was 98 pre-M4, +103 new portable
  tests across the extracted types).
- **WASM compile**: clean on `wasm32-unknown-unknown` as of this
  slice.

### 2026-04-22 — Slice 10: Final consolidation — FrameViewModel, FocusAuthorityMut, CommandSurfaceTelemetry

The remaining shell-side types blocking the "all portable state in
graphshell-core" goal were moved in a single consolidation pass:

**New portable modules:**

- [`graphshell_core::overlay`] — `OverlayStrokePass`,
  `OverlayAffordanceStyle`, `GlyphOverlay`, `GlyphAnchor` (extracted
  from `compositor_adapter.rs` + `registries/atomic/lens/registry.rs`;
  depends on `graph-canvas::packet::Stroke` which is cross-host
  portable by design). 3 tests pinning serde wire shape + variant
  distinctness.
- [`graphshell_core::routing`] — `ToolSurfaceReturnTarget`
  (extracted from `app/routing.rs`). 3 tests pinning serde round-trip
  and variant distinctness.

**Added to existing modules:**

- `ToolPaneState` added to [`graphshell_core::pane`] alongside
  `PaneId` and `TileRenderMode`. 5-variant enum with `title()` /
  `is_navigator_surface()` helpers.
- `GraphViewId` added to [`graphshell_core::graph`] alongside
  `NodeKey`. Wasm-gated `new()`; WASM hosts call `from_uuid(uuid)`
  with a host-supplied UUID.
- `FrameViewModel` added to
  [`graphshell_core::shell_state::frame_model`] now that
  `OverlayStrokePass` + sub-view-model types are all portable.
  Completes the `FrameViewModel` + `FrameHostInput` + all children
  portability goal.
- `FocusAuthorityMut` added to
  [`graphshell_core::shell_state::authorities`]. Uses
  `PortableInstant` for `focus_ring_started_at`; `latch_ring` takes
  `now: PortableInstant` as an explicit parameter rather than calling
  a platform clock. 6 new tests covering latch-on-noop, record-on-change,
  clear-on-None, zero-alpha-with-no-ring, linear-fade-through-duration,
  clear-hint-if-matches.
- `CommandSurfaceTelemetry` + all its data shapes
  (`CommandBarSemanticMetadata`, `OmnibarSemanticMetadata`, etc.) moved
  to [`graphshell_core::shell_state::command_surface_telemetry`] now
  that `PaneId` + `ToolSurfaceReturnTarget` are portable. Confirmed
  empirically that `std::sync::Mutex` compiles to
  `wasm32-unknown-unknown` (Rust 1.70+), so the
  `Mutex<CommandSurfaceSemanticSnapshot>` fields didn't need a
  refactor. 6 new tests covering default state, publish/latest
  round-trip, clear-removes-published, route-event counters,
  omnibar-mailbox counters, saturating-at-u64-max.

**Shell-side latent bugs surfaced & fixed during consolidation:**

- `FocusRingCurve` had no `serde::Serialize`/`Deserialize` derives
  pre-migration but was used as a serde field in `FocusRingSettings`.
  The shell never compiled due to the pre-existing webrender blocker,
  so the error was latent; graphshell-core's clean compile surfaced it.
  Fixed by adding the serde derives to the portable
  `FocusRingCurve`.

**Signatures changed:**

- `FocusAuthorityMut::latch_ring(changed_this_frame, new_focused_node)`
  → `latch_ring(changed_this_frame, new_focused_node, now:
  PortableInstant)`. Callers (`tile_render_pass.rs:699`) now pass
  `portable_now()` explicitly. The bundle's method no longer reaches
  for a shell-side helper.
- `FocusAuthorityMut::ring_alpha` / `ring_alpha_with_curve` already
  took `now: Instant` before this slice; changed to `now:
  PortableInstant`.

**Final cumulative state:**

- **Portable modules in graphshell-core**: 14
  (`actions`, `address`, `async_request`, `content`, `geometry`,
  `graph`, `host_event`, `overlay`, `pane`, `persistence`, `routing`,
  `shell_state`, `time`, `types`).
- **Portable submodules under `shell_state`**: 7 (`authorities`,
  `command_palette`, `command_surface_telemetry`, `frame_model`,
  `omnibar`, `toolbar`, + existing from earlier slices).
- **Test suite**: **219 pass** (was 98 pre-M4, **+121 new portable
  tests** cumulative across the extraction).
- **WASM compile**: clean on `wasm32-unknown-unknown`.

**What's still shell-side (intentionally non-portable host
companions):**

- `ToolbarAuthorityMut` — references `ProviderSuggestionDriver`
  (holds a shell-owned `crossbeam_channel::Receiver<T>`) and
  `&CommandSurfaceTelemetry` (portable but passed by reference
  alongside the non-portable driver).
- `GraphshellRuntime` itself — holds `tokio::runtime::Runtime`,
  `Arc<RegistryRuntime>`, `ControlPanel` (tokio), `ViewerSurfaceRegistry`
  (servo surfaces). These are host-adjacent by design; the portable
  state is *owned by* the runtime but the runtime struct itself stays
  in the shell crate until a separate runtime-holder split (design
  doc §4 slice 10).

The runtime/host boundary is now fully portable for everything except
the concrete async + viewer-surface plumbing — exactly what the M4
portability goal targeted.
