# Graphshell Runtime Crate Extraction Plan

Status: Done gate met — every `GraphshellRuntime` field is portable or cfg-gated; next work: iced-host ungate or bookmark_import_dialog  
Last updated: April 27, 2026

Related docs:

- [SHELL.md](SHELL.md)
- [2026-04-14_iced_host_migration_execution_plan.md](2026-04-14_iced_host_migration_execution_plan.md)
- [archive_docs/checkpoint_2026-04-17/graphshell_docs/implementation_strategy/shell/2026-04-16_runtime_boundary_design.md](../../../../archive_docs/checkpoint_2026-04-17/graphshell_docs/implementation_strategy/shell/2026-04-16_runtime_boundary_design.md)

## Why this lane exists

`iced_parity` is still expensive because the parity tests live inside the main
`graphshell` crate. Even when the tests only exercise host-neutral runtime
behavior, `cargo test --features iced-host` still has to compile the heavy
desktop crate and its Servo/render stack.

The fix is not another test filter. The fix is to move the host-neutral runtime
kernel behind a smaller crate boundary so parity-focused tests can eventually
compile against that lighter target instead of the monolithic shell crate.

## Current diagnosis

`GraphshellRuntime::tick` does not depend on the entire host-port surface.
Today its portable side effects are limited to:

- toast emission for runtime-owned finalize actions
- clipboard reads/writes for runtime-owned finalize actions

That makes the toast/clipboard port subset the cheapest extraction slice. It is
already portable, already trait-shaped, and does not require touching the
viewer/compositor path or the user’s in-flight M4.5/M5 work.

2026-04-24 refinement: that cheapest slice has now landed, and the plan is
already beyond the original done gate. The remaining extraction risk is no
longer the tick-owned service ports; it is the projection side of
`GraphshellRuntime::project_view_model`. That projection still reads a broad
runtime/shell state surface: GraphTree layout caches, focus authority, toolbar
state, omnibar/search sessions, command-palette session, dialog flags,
settings, thumbnail state, and the shell-local UxTree snapshot. Moving more code
without first naming those read dependencies would create a fake-light crate
that still smuggles the monolithic shell shape through a trait.

## Target end-state

Add a lightweight `graphshell-runtime` workspace crate that gradually becomes
the home for the host-neutral runtime boundary:

1. portable frame boundary vocabulary
2. tick-owned service ports
3. runtime-side projection/finalize helpers
4. a narrower runtime kernel that parity tests can compile without the full
   shell host stack

This plan does not move `GraphshellRuntime` wholesale in one shot. The main
crate remains the integration point while the portable kernel is carved out in
small, validated slices.

## Slice 1 - completed

Scope for this session:

- create `crates/graphshell-runtime`
- move the tick-owned toast/clipboard trait definitions there as
  `RuntimeToastPort` and `RuntimeClipboardPort`
- add `RuntimeTickPorts` as the composite bound used by
  `GraphshellRuntime::tick`
- preserve the shell-side import path by re-exporting those traits from
  `shell/desktop/ui/host_ports.rs`

Done gate for Slice 1:

- `graphshell-runtime` compiles independently
- `GraphshellRuntime::tick` no longer depends on the broader shell-only
  `HostPorts` bundle for finalize actions
- existing egui/iced host bundles continue to satisfy the runtime tick bound
  without call-site churn

Progress log:

- 2026-04-24: Landed the crate scaffold and first port move. New
  `graphshell-runtime` crate now owns `RuntimeToastPort`,
  `RuntimeClipboardPort`, and `RuntimeTickPorts`; the shell preserves old
  import paths by re-exporting those traits from
  `shell/desktop/ui/host_ports.rs`.
- 2026-04-24: `GraphshellRuntime::tick` now binds to `RuntimeTickPorts`
  instead of the wider shell-only `HostPorts` bundle.
- 2026-04-24: Landed the next payload extraction slice. Portable queued
  finalize-action payloads now live in `graphshell-runtime`
  (`ClipboardCopyKind`, `ClipboardCopyRequest`, `UiNotificationLevel`,
  `NodeStatusNotice`), while the app keeps the audit-carrying wrapper
  `NodeStatusNoticeRequest` and re-exports the moved types to preserve the
  existing `crate::app::*` import surface.
- 2026-04-24: Landed the next finalize-helper sub-slice on the toast side.
  `graphshell-runtime` now owns the generic notice drain helper
  (`drain_pending_node_status_notices`) plus the shared toast helpers
  (`emit_node_status_toast`, `port_error`) behind a narrow
  `RuntimeNodeStatusNoticeState` trait; the shell's `toast_flow` now only
  adapts `GraphBrowserApp` to that runtime trait.
- 2026-04-24: Landed the matching clipboard-side finalize-helper sub-slice.
  `graphshell-runtime` now owns the generic clipboard drain helper
  (`drain_pending_clipboard_copy_requests`), clipboard message shaping, and a
  narrow `RuntimeClipboardCopyState` seam over queue-pop plus resolved visible
  title/url lookup; the shell's `clipboard_flow` now only adapts
  `GraphBrowserApp` to that runtime trait and keeps the shell-local
  diagnostics event on clipboard write failure.
- 2026-04-24: Added a shell-local `ui/finalize_actions.rs` facade so
  `gui_state.rs` no longer reaches through `gui_orchestration` just to trigger
  runtime finalize drains. `GraphshellRuntime::tick` now calls that local
  facade, and `gui_state.rs` started the frame-boundary re-export slice by
  consuming `FrameHostInput` / `FrameViewModel` from `graphshell-runtime`
  directly instead of the shell-local re-export module.
- 2026-04-24: Continued the frame-boundary re-export slice through the iced
  host stack. `iced_host.rs`, `iced_app.rs`, and the top-level
  `iced_parity.rs` imports now consume `FrameHostInput` / `FrameViewModel`
  from `graphshell-runtime` directly instead of the shell-local
  `ui::frame_model` alias.
- 2026-04-24: Continued the same frame-boundary re-export slice through the
  egui host update/render pipeline. `gui.rs` now builds `FrameHostInput` and
  caches `FrameViewModel` via direct `graphshell-runtime` imports, and the
  adjacent `gui/update_frame_phases.rs` plus
  `gui_frame/post_render_phase.rs` cached-view-model plumbing now carries the
  same runtime type directly instead of spelling it through the shell-local
  `ui::frame_model` alias.
- 2026-04-24: Finished the next closest facade cleanup in the desktop host
  path. `workbench/tile_render_pass.rs` now carries cached `FrameViewModel`
  through a direct `graphshell-runtime` import, and both `ui/egui_host_ports.rs`
  plus `ui/iced_host_ports.rs` now consume `ToastSeverity` / `ToastSpec`
  directly from `graphshell-runtime` instead of through the shell-local
  `ui::frame_model` facade.
- 2026-04-24: Extended `graphshell-runtime` to re-export the remaining
  portable view-model vocabulary still needed by the runtime-adjacent shell
  state (`CommandPaletteScopeView`, `CommandPaletteViewModel`,
  `DialogsViewModel`, `FocusRingSpec`, `FocusViewModel`,
  `GraphSearchViewModel`, `Omnibar*View`, `ToolbarViewModel`) and rebound
  `ui/gui_state.rs` plus the `FocusRingSpec` fallback in
  `workbench/tile_render_pass.rs` to consume those types directly from
  `graphshell-runtime`.
- 2026-04-24: Removed the now-dead shell-local `ui/frame_model.rs` shim and
  its `ui/mod.rs` module export after the desktop runtime path stopped
  importing `crate::shell::desktop::ui::frame_model`. The portable vocabulary
  now enters that path directly from `graphshell-runtime` or
  `graphshell_core`.
- 2026-04-24: Finished the last direct `graphshell_core::shell_state::frame_model`
  spellings in the desktop runtime path by extending `graphshell-runtime` to
  re-export the remaining settings/accessibility vocabulary
  (`SettingsViewModel`, `FocusRingSettingsView`, `Thumbnail*View`,
  `AccessibilityViewModel`, plus `FocusRingCurve`) and rebinding the
  settings/accessibility projection block in `ui/gui_state.rs` to consume
  those types from `graphshell-runtime`.
- 2026-04-24: Validation receipts:
  `cargo check -p graphshell-runtime` passed; `cargo test focus_view_model
  --lib` passed (7 tests).

Additional validation notes:

- 2026-04-24: Added direct unit coverage for the extracted runtime toast
  helper seam in `graphshell-runtime`.
- 2026-04-24: Extended the direct `graphshell-runtime` unit coverage to the
  extracted clipboard helper seam; isolated `cargo test -p graphshell-runtime
  --lib` passed with 5 tests in a dedicated target dir.
- 2026-04-24: Editor diagnostics stayed clean for the touched runtime and
  shell adapter files. A heavier shell-level focused notice test was started
  in an isolated target directory but remained dominated by the repo's shared
  compile wall during this slice.
- 2026-04-24: The shell-local finalize-actions facade files
  (`ui/finalize_actions.rs`, `ui/gui_state.rs`, `ui/mod.rs`) stayed
  diagnostics-clean. A focused `cargo test pending_node_status_notice --lib`
  rerun again dropped into the same full webrender/Servo compile wall, so this
  slice keeps its executable receipt at the lighter runtime-crate boundary.
- 2026-04-24: The iced-side import rebinding files (`ui/iced_host.rs`,
  `ui/iced_app.rs`, `ui/iced_parity.rs`) stayed diagnostics-clean. A focused
  `cargo test runtime_tick_parity_across_host_ports --lib --features iced-host`
  rerun did not surface a local compile error before dropping into the heavy
  feature-enabled dependency build, so this slice likewise stops short of
  claiming a full iced-host executable receipt.
- 2026-04-24: The egui-side rebind files (`ui/gui.rs`,
  `ui/gui/update_frame_phases.rs`, `ui/gui_frame/post_render_phase.rs`) stayed
  diagnostics-clean, and a follow-up grep confirmed the nearby
  `ui::frame_model::FrameHostInput` / `FrameViewModel` spellings were cleared
  from the shell update pipeline. A focused `cargo test focus_view_model --lib`
  rerun in an isolated target dir again fell into the shared Servo/webrender
  compile wall before reaching a slice-local compile or test result, so this
  step also keeps its executable receipt at diagnostics plus prior warmed
  receipts.
- 2026-04-24: The follow-on facade-cleanup files
  (`workbench/tile_render_pass.rs`, `ui/egui_host_ports.rs`,
  `ui/iced_host_ports.rs`) stayed diagnostics-clean, and a targeted search no
  longer found direct `frame_model::{FrameHostInput, FrameViewModel,
  ToastSeverity, ToastSpec}` spellings under `shell/desktop` after this slice.
- 2026-04-24: The follow-on runtime-vocabulary rebind files
  (`crates/graphshell-runtime/src/lib.rs`, `ui/gui_state.rs`,
  `workbench/tile_render_pass.rs`) stayed diagnostics-clean. A targeted search
  no longer found `crate::shell::desktop::ui::frame_model` imports/usages
  under `shell/desktop`, indicating the desktop runtime path now reaches the
  portable vocabulary directly through `graphshell-runtime` or
  `graphshell_core` instead of the shell-local shim.
- 2026-04-24: The shim-removal files (`ui/mod.rs`, deleted `ui/frame_model.rs`)
  stayed diagnostics-clean at the editor level. A follow-up `cargo check -p
  graphshell --lib` run in an isolated target dir did not surface any
  shim-removal error before dropping back into the shared Servo/webrender
  compile wall, so this step records the deletion with local diagnostics plus
  prior targeted search receipts rather than a completed graphshell crate
  receipt.
- 2026-04-24: The final settings/accessibility rebind files
  (`crates/graphshell-runtime/src/lib.rs`, `ui/gui_state.rs`) stayed
  diagnostics-clean. A targeted search no longer found direct
  `graphshell_core::shell_state::frame_model::` spellings under
  `shell/desktop`, so the desktop runtime path now consistently consumes the
  portable frame vocabulary through `graphshell-runtime`.

## Follow-on slices

### Completed by Slice 1

The first two previously listed follow-ons are no longer future work:

1. ~~Re-export the portable frame boundary (`FrameHostInput`, `FrameViewModel`,
   toast types) from `graphshell-runtime` and migrate runtime-owned helpers to
   consume that crate directly.~~ **Done — see progress log 2026-04-24.**
2. ~~Move finalize-action helpers out of the main shell crate once their direct
   dependencies are portable.~~ **Done — see progress log 2026-04-24.** The
   shell still owns the thin `ui/finalize_actions.rs` adapter because
   `GraphBrowserApp` and diagnostics/audit side effects remain shell/app-owned.

### Completed slice: AppState -> FrameViewModel seam inventory, then extraction

Do not move `GraphshellRuntime` wholesale next. The next useful step is to make
`project_view_model` extractable without pretending the whole shell state is
portable. This slice is specifically the shell AppState -> `FrameViewModel`
transformation: the read/model shaping pass that turns runtime and chrome state
into the frame view model a host can render. It is not Graph Cartography / GC
projection vocabulary, which names graph-memory aggregate projections for
Navigator/scorer/annotation consumers.

Scope:

- Add a short AppState -> `FrameViewModel` source inventory next to this plan
  or in the progress log, grouping every `project_view_model` read into one of
  four buckets:
  already portable, portable-but-shell-owned, shell-local adapter, or not ready
  to move.
- Split only the pure shaping helpers that already consume portable or
  near-portable inputs. Likely first candidates are focus/settings/accessibility
  projection helpers, because they already return `graphshell-runtime` view
  types and have narrow inputs.
- Keep shell-owned adapters in the main crate. In particular, UxTree snapshot
  lookup, audit/diagnostics logging, and `GraphBrowserApp` graph/runtime access
  should not move until they have explicit portable source traits.
- Add direct unit tests for any extracted pure helper in the lightest crate that
  can own it. If a helper still needs shell types, keep the test in the shell
  crate and treat it as preparation, not runtime-crate extraction.

Initial source inventory (2026-04-25):

- Already portable:
  focus projection inputs (`NodeKey`, `PaneId`, `PortableInstant`,
  `FocusRingSettingsView`, graph-surface focus flag, pane activation, pane ->
  node order), plus settings projection inputs once shell-owned thumbnail enum
  variants are adapted into `Thumbnail*View` mirrors, accessibility summary
  fields after the shell-local UxTree lookup, and graph-search scalar/query
  state after the shell has counted matches, toolbar/session mirrors after the
  shell has selected the active-pane draft, omnibar fields after shell-local
  kind/status adaptation, command-palette fields after scope adaptation, plus
  dialog/open-state flags after shell-owned dialog objects are reduced to
  booleans, and transient output placeholders plus thumbnail capture count.
  This bucket now has runtime helpers for focus, settings, accessibility,
  graph-search, toolbar, omnibar, command-palette, dialog, and transient-output
  assembly.
- Portable-but-shell-owned:
  `graph_runtime` frame caches (`active_pane_rects`, pane render modes, viewer
  IDs, tree rows, tab order, split boundaries), toolbar state/drafts,
  command-palette state, and the shell-owned app settings / graph-search match
  collections / dialog objects / thumbnail capture set before they are mirrored,
  counted, or reduced into portable view-model inputs.
- Shell-local adapter:
  egui-rect to portable-rect conversion, `portable_time::portable_now()`, UxTree
  snapshot lookup for accessibility metadata, and shell enum adaptation before
  settings projection.
- Not ready to move:
  direct `GraphBrowserApp` / workspace ownership reads, shell dialog state,
  audit/diagnostics side effects, and any helper that would need most of
  `GraphshellRuntime` just to compile.

Progress log:

- 2026-04-25: Added `graphshell-runtime::frame_projection` with
  `project_focus_view_model`, `FocusProjectionInput`, and
  `FocusProjectionOutput`. `ui/gui_state.rs::project_view_model` now adapts
  the shell's active-pane rect roster into a portable `(PaneId, NodeKey)` order
  and delegates focus shaping to the runtime crate. This preserves the existing
  focused-node semantics (`focused_node` follows the first rendered pane when
  graph-surface focus is false; `active_pane` follows pane activation with a
  first-pane fallback) while moving focus-ring alpha/spec expiry math into the
  light crate.
- 2026-04-25: Added direct runtime-crate unit coverage for active-pane fallback,
  graph-surface focus gating, live/expired focus-ring publishing, and disabled
  focus-ring zero-alpha behavior. Receipts: `cargo test -p graphshell-runtime
  --lib` passed (9 tests); `cargo check -p graphshell-runtime` passed. Existing
  upstream warnings remain in `graph-canvas`, `graph-tree`, and
  `graphshell-core` and are unrelated to this slice.
- 2026-04-25: Added `project_settings_view_model` and
  `SettingsProjectionInput` to `graphshell-runtime::frame_projection`.
  `ui/gui_state.rs::project_view_model` still performs the shell-owned enum
  adaptation from `app::Thumbnail*` variants into portable `Thumbnail*View`
  variants, then delegates `SettingsViewModel` assembly to the runtime crate.
  Direct unit coverage now pins focus-ring and thumbnail settings preservation.
  Receipts: `cargo fmt --package graphshell-runtime --
  shell\\desktop\\ui\\gui_state.rs`; `cargo test -p graphshell-runtime --lib`
  passed (10 tests); `cargo check -p graphshell-runtime` passed. Existing
  upstream warnings remain in `graph-canvas`, `graph-tree`, and
  `graphshell-core` and are unrelated to this slice.
- 2026-04-25: Added `project_accessibility_view_model` and
  `AccessibilityProjectionInput`. `ui/gui_state.rs::project_view_model` still
  owns the shell-local UxTree snapshot lookup, then delegates the portable
  `AccessibilityViewModel` summary assembly to the runtime crate. Direct unit
  coverage pins focused-node, snapshot-version, and published-flag preservation.
- 2026-04-25: Added `project_graph_search_view_model` and
  `GraphSearchProjectionInput`. The shell still owns query storage, match list
  ownership, and match counting; the runtime crate now owns the host-facing
  `GraphSearchViewModel` assembly once those portable inputs are supplied.
  Direct unit coverage pins open/query/filter/match-count/active-index
  preservation.
- 2026-04-25: Validation receipts for the accessibility + graph-search slices:
  `cargo fmt --package graphshell-runtime -- shell\\desktop\\ui\\gui_state.rs`;
  `cargo test -p graphshell-runtime --lib` passed (12 tests); `cargo check -p
  graphshell-runtime` passed. Existing upstream warnings remain in
  `graph-canvas`, `graph-tree`, and `graphshell-core` and are unrelated to this
  slice.
- 2026-04-25: Added `project_dialogs_view_model` and `DialogsProjectionInput`.
  The shell still owns concrete dialog/session objects (`bookmark_import_dialog`,
  toolbar clear-data confirmation state, and chrome UI flags), but now reduces
  them to portable dialog/open-state inputs before delegating
  `DialogsViewModel` assembly to the runtime crate. Direct unit coverage pins
  all dialog flags plus the clear-data deadline.
- 2026-04-25: Validation receipts for the dialogs slice: `cargo fmt --package
  graphshell-runtime -- shell\\desktop\\ui\\gui_state.rs`; `cargo test -p
  graphshell-runtime --lib` passed (13 tests); `cargo check -p
  graphshell-runtime` passed. Existing upstream warnings remain in
  `graph-canvas`, `graph-tree`, and `graphshell-core` and are unrelated to this
  slice.
- 2026-04-25: Finished the remaining pure AppState -> `FrameViewModel` helper
  extraction candidates for this phase. `graphshell-runtime::frame_projection`
  now owns toolbar, omnibar, and command-palette assembly in addition to the
  earlier focus/settings/accessibility/graph-search/dialog helpers;
  `ui/gui_state.rs::project_view_model` still performs shell-local enum/status
  adaptation before calling those helpers.
- 2026-04-25: Validation receipts for the final AppState -> `FrameViewModel`
  helper pass: `cargo fmt --package graphshell-runtime --
  shell\\desktop\\ui\\gui_state.rs`; `cargo test -p graphshell-runtime --lib`
  passed (16 tests); `cargo check -p graphshell-runtime` passed. Existing
  upstream warnings remain in `graph-canvas`, `graph-tree`, and
  `graphshell-core` and are unrelated to this slice.
- 2026-04-25: Added `project_transient_frame_outputs`,
  `TransientFrameOutputsProjectionInput`, and `TransientFrameOutputsProjection`.
  This keeps the current placeholder outputs (`overlays`, `toasts`,
  `surfaces_to_present`, `degraded_receipts`) explicitly grouped in the runtime
  projection module while preserving the shell-owned thumbnail capture set as a
  reduced `captures_in_flight` count. Direct unit coverage pins the empty
  placeholder vectors plus capture count preservation.
- 2026-04-25: Validation receipts for the transient-output slice: `cargo fmt
  --package graphshell-runtime -- shell\\desktop\\ui\\gui_state.rs`; `cargo test
  -p graphshell-runtime --lib` passed (18 tests); `cargo check -p
  graphshell-runtime` passed. Existing upstream warnings remain in
  `graph-canvas`, `graph-tree`, and `graphshell-core` and are unrelated to this
  slice.

Done gate - met 2026-04-25:

- `project_view_model` is decomposed enough that each remaining shell read has
  an explicit owner bucket.
- At least one projection helper moves or is isolated behind a named seam
  without adding `graphshell` as a dependency of `graphshell-runtime`.
- `cargo test -p graphshell-runtime --lib` passes at 24 tests after the
  cross-lane extension slices.
- `cargo check` with default features is clean.

### Later slice: lighter parity target

Only after the projection seam is real should parity tests move or be rebuilt
against the lighter crate. The first cheap parity target should not instantiate
`GraphshellRuntime`; it should exercise extracted finalize/projection helpers
against tiny test states. Full cross-host parity stays in the main crate until
the runtime kernel has a genuine portable state trait.

Progress log:

- 2026-04-25: Seeded the first lightweight projection parity target inside
  `graphshell-runtime` without instantiating `GraphshellRuntime` or the shell
  host stack. The test composes extracted focus, toolbar, graph-search,
  command-palette, and dialog projection helpers from tiny portable inputs.
- 2026-04-25: Validation receipts for the lightweight parity-target slice:
  `cargo fmt --package graphshell-runtime -- shell\\desktop\\ui\\gui_state.rs`;
  `cargo test -p graphshell-runtime --lib` passed (18 tests); `cargo check -p
  graphshell-runtime` passed. Existing upstream warnings remain in
  `graph-canvas`, `graph-tree`, and `graphshell-core` and are unrelated to this
  slice.

### Cross-lane additions (servo-into-verso S3a + extension slices)

The 2026-04-25 servo-into-verso lane (see
[2026-04-25_servo_into_verso_plan.md](2026-04-25_servo_into_verso_plan.md))
added complementary surface to the same `graphshell-runtime` crate:

- 2026-04-25 (S3a host-port traits): the broader host-port trait
  surface (HostInputPort, HostSurfacePort with associated
  `BackendContext` type, HostPaintPort, HostTexturePort,
  HostAccessibilityPort with portable `request_focus`) moved into
  `graphshell-runtime::ports` alongside the existing
  RuntimeClipboardPort/RuntimeToastPort. Plus host-neutral
  `BackendViewportInPixels` (was in `shell::desktop::render_backend`)
  and `ViewerSurfaceId` (host-neutral viewer/webview identity, two
  u32 fields mirroring `servo::WebViewId`'s shape). The Servo-keyed
  tree-update injection split into a separate
  `ServoAccessibilityInjectionPort` extension trait that lives in
  graphshell main, gated on `servo-engine`.
- 2026-04-25 (graph_runtime layout-cache projection): added
  `project_graph_runtime_layout_view_model` +
  `GraphRuntimeLayoutProjectionInput` /
  `GraphRuntimeLayoutProjection`. Consumes the per-frame layout
  outputs (active_pane_rects post egui→portable conversion,
  pane_render_modes, pane_viewer_ids, cached_tree_rows,
  cached_tab_order, cached_split_boundaries) and derives
  `is_graph_view`. Two new unit tests pin empty-layout and
  populated-layout passthrough behavior. `gui_state.rs::project_view_model`
  delegates to it. `graph-tree` added as a direct
  `graphshell-runtime` dep (already transitive via graphshell-core's
  frame_model).
- 2026-04-25 (portable_time relocation): `portable_now()` moved from
  `shell::desktop::ui::portable_time` into
  `graphshell-runtime::portable_time`. Both desktop hosts (egui +
  iced) anchor monotonic clocks identically; the runtime crate is
  the natural home. Shell-side `portable_time.rs` is now a tiny
  re-export shim so existing call sites work unchanged. Two new
  unit tests pin monotonic non-decreasing + advance-with-time
  behavior.

Validation: `cargo test -p graphshell-runtime --lib` passed at 24
tests post-extension; `cargo check` (default features) clean.

## Closure note and next moves

The canonical plan's explicit done-gate criteria are now met. Going further
into "extract more from `GraphshellRuntime`" would cross the plan's stated
guardrails: the remaining fields that still block iced launch without
`servo-engine` (`viewer_surfaces`, `webview_creation_backpressure`,
`frame_inbox`, `bookmark_import_dialog`, and adjacent runtime/app-owned state)
all belong to the current "not ready to move" bucket. They depend on
`GraphBrowserApp` ownership, audit/diagnostics side effects, shell dialog
state, or shell-local UxTree lookup patterns that are not yet fronted by narrow
portable source traits.

The iced-launch-without-`servo-engine` goal is still reachable, but the next
runtime-crate progress should happen one source trait at a time rather than by
moving more of `GraphshellRuntime` behind a broad trait. Two clean fresh-session
options are:

- Pick one source-trait extraction, such as wrapping the
  `webview_creation_backpressure` metadata view behind a narrow
  `RuntimeWebviewBackpressureMetadataSource`-style seam before moving any
  field.
- Pivot to the servo-into-verso S2c body-level cascade pass, which is
  mechanical, bounded, and currently tracked at roughly 75 remaining errors.

### Source-side audit - webview creation backpressure (2026-04-25)

Audit target: `GraphshellRuntime::webview_creation_backpressure`, currently a
`HashMap<NodeKey, WebviewCreationBackpressureState>` owned by the shell runtime
state in `ui/gui_state.rs`.

Current source ownership:

- Storage owner: `GraphshellRuntime`. The field is transient retry/probe state,
  initialized empty in `new_minimal`, cleared on empty graph or no active pane
  work in lifecycle reconcile, and cleared during graph snapshot workspace reset.
- Metadata reader path: `tile_render_pass` publishes per-node attach-attempt
  metadata from the map each frame; `ux_tree` later consumes the published
  metadata when building the semantic snapshot.
- Creation/reconcile writers: `ensure_webview_for_node` and
  `reconcile_webview_creation_backpressure` own the retry state machine. The
  visible-pane path calls creation from `tile_render_pass`, toolbar/keyboard
  tile toggles call creation through `tile_view_ops`, and selected-node prewarm
  calls creation through `lifecycle_reconcile`.
- Effectful dependencies: the creation path reaches `GraphBrowserApp`,
  `EmbedderWindow`, `RunningAppState`, Servo `WebViewId`, Servo rendering
  contexts, `ViewerSurfaceRegistry`, `ViewerSurfaceHost`, diagnostics channels,
  pending host-create tokens, and reducer intents.

Classification:

- Ready to isolate: the read-only attach-attempt metadata view
  (`retry_count`, pending age, cooldown remaining) and map reset/clear behavior.
  This is the smallest honest source seam because it exposes what consumers
  need without pretending webview allocation is portable.
- Portable after small vocabulary work: the probe identity inside
  `WebviewCreationBackpressureState`. It is Servo `WebViewId` today; moving the
  state type to `graphshell-runtime` would require converting it to the already
  host-neutral `ViewerSurfaceId` or keeping a Servo-specific adapter in
  graphshell main.
- Not ready to move: `ensure_webview_for_node` as a whole. It performs host
  allocation and Servo webview creation, consumes `RunningAppState`, mutates
  viewer surfaces, emits diagnostics, maps/unmaps renderer IDs, and pushes
  lifecycle intents.
- Not ready to move as one broad trait: a generic
  `RuntimeWebviewBackpressureSource` over the whole map would either expose
  Servo-shaped internals or become a giant shell-ownership trait. That would
  violate this plan's guardrail.

Recommended next slice:

- Prefer a narrow metadata/source seam first, such as
  `RuntimeNodePaneAttachAttemptSource` or
  `RuntimeWebviewBackpressureMetadataSource`, backed by the existing map in
  graphshell main.
- Keep `ensure_webview_for_node` and Servo creation/reconcile effects in
  graphshell main for now, likely behind `servo-engine` as part of the S2c
  body-level cascade pass.
- If moving state is still desired after the metadata seam, split the type into
  host-neutral retry/cooldown data plus a host-specific pending-probe adapter
  that converts Servo `WebViewId` to/from `ViewerSurfaceId` at the graphshell
  boundary.

Progress log:

- 2026-04-25: Landed the first source-side extraction from this audit.
  `graphshell-runtime::webview_backpressure` now owns the host-neutral
  `NodePaneAttachAttemptMetadata` payload plus the
  `RuntimeWebviewBackpressureMetadataSource` trait. The shell keeps the
  Servo-backed retry/probe state machine and implements the metadata source via
  a small local wrapper over the existing
  `HashMap<NodeKey, WebviewCreationBackpressureState>`. Existing shell import
  paths are preserved by re-exporting `NodePaneAttachAttemptMetadata` from
  `shell::desktop::lifecycle::webview_backpressure`.
- 2026-04-25: Validation receipts for the metadata-source slice:
  `cargo fmt --package graphshell-runtime --
  shell\desktop\lifecycle\webview_backpressure.rs`; `cargo test -p
  graphshell-runtime --lib` passed at 26 tests; `cargo check -p
  graphshell-runtime` passed; `cargo check -p graphshell --lib` passed.
  Existing upstream warnings remain in `graph-canvas`, `graph-tree`,
  `graphshell-core`, `webrender`, `wr_glyph_rasterizer`, and a deprecated egui
  call in `egui_host_ports`; they are unrelated to this slice.
- 2026-04-26: Landed the **retry/cooldown core extraction** — the
  natural follow-on to the metadata-source seam. Added
  `WebviewAttachRetryState` (host-neutral: `retry_count`, `cooldown_step`,
  plus methods `cooldown_delay_ms_for_step`, `advance_cooldown_step`,
  `record_attempt`, `is_retry_exhausted`, `reset`, `reset_retry_count`)
  to `graphshell-runtime::webview_backpressure`. Reimplemented the
  exponential cooldown delay as a pure `min*2^step`-clamp-to-`[MIN, MAX]`
  function (matches existing `backon::ExponentialBuilder` semantics
  bit-for-bit at every step), keeping graphshell-runtime free of
  `backon` and `Instant`. The shell-side
  `WebviewCreationBackpressureState` now composes
  `retry: WebviewAttachRetryState` alongside the Servo-typed
  `pending: Option<WebviewCreationProbe>` and
  `cooldown_until: Option<Instant>` — the explicit boundary the audit
  named (probe identity + deadline arithmetic stay shell-side because
  they depend on Servo `WebViewId` and `std::time::Instant`).
  All shell-side numeric constants
  (`WEBVIEW_CREATION_MAX_RETRIES`, `WEBVIEW_CREATION_COOLDOWN_MIN`,
  `WEBVIEW_CREATION_COOLDOWN_MAX`, `WEBVIEW_CREATION_COOLDOWN_MAX_STEP`)
  now live as `WebviewAttachRetryState::MAX_RETRIES` etc. on the
  runtime side. Migrated the cooldown-delay-bounds test plus added 8
  new tests on the runtime side (cooldown step doubling, advance
  semantics, saturation, reset variants, retry exhaustion, attempt
  saturation). Validation: graphshell-runtime tests 26 → 33 pass;
  shell-side webview_backpressure tests (7) all pass; full
  engine-feature matrix (default / no-default wry / no-default
  iced-host,wry) all 3/3 PASS. Side fix: added missing
  `ServoAccessibilityInjectionPort` import to the
  `egui_host_ports.rs` test mod (a leftover from yesterday's S3a
  trait split that surfaced the moment `cargo test --lib` was
  exercised; one-line correction). Sidequest noted: `backon` is no
  longer referenced anywhere in graphshell main; awaiting user
  confirmation before removing the dependency from `Cargo.toml`.
- 2026-04-27: Landed **viewer_surfaces Step 2** — host-neutral
  `RenderingContextProducer` trait in
  `graphshell-runtime::rendering_context_producer`, plus a shell-side
  `ServoRenderingContextProducer` adapter at
  `shell/desktop/render_backend/`. Trait surface is the minimum the
  compositor's `ViewerSurfaceBacking::rendering_context()` consumers
  actually touch on the wgpu path: `size_in_pixels()`, `resize()`,
  `present()` — primitives only, no external trait dependencies. GL
  `make_current` / `prepare_for_rendering` are deliberately NOT in the
  trait: graphshell is wgpu-first (Servo at `servo-wgpu`, renderer at
  `webrender-wgpu`), and the GL-compat fallback is gated behind the
  deprecated `gl_compat` feature inside `OffscreenRenderingContext`
  consumers (handled at the path-specific call site in
  `paint_offscreen_content_pass`, not at the producer trait level). Servo's `RenderingContextCore` (which
  drags `embedder_traits::RefreshDriver`, `webrender_api::units`,
  `surfman`, `gleam`/`glow`) stays in Servo; the adapter bridges. Per
  the source-side review, the alternatives all carried sharper costs:
  re-extracting the full Servo trait would defeat the lightweight-runtime
  goal; opaque trait-object handles would lose concrete-type access on
  the shell side; full registry parameterization is a separate question.
  `ViewerSurfaceBacking` deliberately stays Servo-typed in this slice:
  `compositor_adapter.rs` is gated on `servo-engine` anyway, and Servo
  webview construction (`webview_backpressure.rs:328`) consumes
  `Rc<dyn RenderingContextCore>` directly. The reshape that swaps
  `NativeRenderingContext` to `Rc<dyn RenderingContextProducer>` is the
  follow-on triggered when iced-host plugs in its own producer.
  Validation: graphshell-runtime tests 37 → 40 (3 new trait tests:
  resize observation, present count, object-safety); engine-feature
  matrix all 3/3 PASS.
- 2026-04-27: Landed **viewer_surfaces Step 1** — the host-neutral
  lifecycle types from `compositor_adapter.rs`. `ContentSurfaceHandle<T>`
  is now generic over the host's texture-token type and lives in
  `graphshell-runtime::content_surface` along with
  `ViewerSurfaceFramePath`. Shell-side keeps a type alias bound to
  `BackendTextureToken` plus a free `content_surface_handle_for_node`
  function for the static-map lookup (the only inherent-impl method that
  needed shell-owned context). `content_generation: u64` already lived
  as a portable counter and stays a field on `ViewerSurface`. Per the
  servo-into-verso plan's audit, Step 2 will introduce a portable
  `RenderingContextProducer` trait so the host-neutral parts of
  `ViewerSurfaceBacking` can join the runtime crate; today the backing
  stays shell-side because it references Servo
  `RenderingContextCore`/`OffscreenRenderingContext`. Validation:
  graphshell-runtime tests 35 → 37; engine-feature matrix all 3/3 PASS.
- 2026-04-27: Landed the **frame_inbox extraction** — the next
  portable-but-shell-owned input from the closure-note inventory
  (line 448 above). `FrameInboxState` (the typed `mpsc::Receiver`
  bag with the `FrameSignalRelay<T>` `drain_flag`/`drain_all`
  helpers and the four per-frame `take_*` consumers) now lives in
  `graphshell-runtime::frame_inbox`, with the two drain-coalescing
  tests migrated alongside it. Shell-side
  `shell/desktop/ui/gui/frame_inbox.rs` is now a thin wiring shim:
  `pub(crate) type GuiFrameInbox = FrameInboxState` plus a free
  `spawn_gui_frame_inbox(&mut ControlPanel, Arc<dyn SignalRouter>)
  -> GuiFrameInbox` constructor that owns the ControlPanel-driven
  subscription wiring (signal types are already
  `graphshell_core::signal_router::*`, so the spawn body stays
  portable except for the `&mut ControlPanel` parameter). The
  control-panel spawn test stays shell-side. Two call sites
  updated (`gui.rs:419`, `gui_state.rs:769`) from
  `GuiFrameInbox::spawn(...)` to `spawn_gui_frame_inbox(...)`.
  Validation: graphshell-runtime tests 33 → 35 (two drain tests
  added); shell-side `frame_inbox` spawn test still passes;
  engine-feature matrix all 3/3 PASS. Remaining "not ready to
  move" closure-note items are now `viewer_surfaces`,
  `webview_creation_backpressure`, and `bookmark_import_dialog`;
  per the servo-into-verso plan's audit, `viewer_surfaces` is the
  next recommended take (with `bookmark_import_dialog` deferred
  since it's already reduced to a `bool` projection).
- 2026-04-27: Landed **webview_creation_backpressure extraction** —
  the last active closure-note item before the done gate. Two new
  portable types joined `graphshell-runtime::webview_backpressure`:
  `WebviewCreationProbeState` (viewer identity as `ViewerSurfaceId`
  packed through the renderer-id registry + `started_at:
  PortableInstant`) and `WebviewCreationBackpressureState` (composes
  `WebviewAttachRetryState` + optional probe + optional cooldown
  deadline as `PortableInstant` + `cooldown_notified: bool`). The
  `cooldown_notified` flag replaces the original `Option<Instant>`
  equality comparison that would have been impossible with
  `PortableInstant` storage; it suppresses redundant
  `MarkRuntimeBlocked` pushes within a single cooldown window and is
  reset whenever `cooldown_until` is armed or cleared. Shell-side
  `webview_backpressure.rs` gained two adapter fns:
  `viewer_surface_id_from_servo_webview` (packs `RendererId::as_raw()`
  into `ViewerSurfaceId::from_u64`) and
  `servo_webview_id_from_viewer_surface` (reverses via the registry).
  `MarkRuntimeBlocked.retry_at: Option<std::time::Instant>` stays
  unchanged at the app-domain level; push sites compute the `Instant`
  deadline from `Instant::now() + Duration::from_millis(delay_ms)`
  independently of the portable `cooldown_until` field. All 8 shell
  import sites for `WebviewCreationBackpressureState` migrated from
  the shell lifecycle module to `graphshell_runtime::`:
  `gui_state.rs`, `gui_orchestration.rs`, `gui_frame.rs`,
  `gui_frame/keyboard_phase.rs`, `gui/semantic_lifecycle_flow.rs`,
  `gui/toolbar_phase_flow.rs`, `lifecycle/lifecycle_reconcile.rs`
  (split `self, State` import), `workbench/tile_view_ops.rs` (same
  split). Validation: graphshell-runtime tests 40 → 43 (3 new:
  `backpressure_state_default_is_idle`,
  `probe_state_viewer_surface_id_roundtrip_via_u64`,
  `cooldown_until_ordering_reflects_ms_comparison`); shell
  `test_arm_creation_cooldown_advances_step_and_deadline` updated to
  use `PortableInstant(10_000)` fixed point; engine-feature matrix
  all 3/3 PASS. Done gate met: every `GraphshellRuntime` field is
  now either portable or cfg-gated.

## Risks and guardrails

- Do not widen this slice into viewer/compositor extraction. That is a
  different dependency wall.
- Do not break existing shell imports while the migration is partial; preserve
  current paths with re-exports.
- Do not claim cheap parity yet. Slice 1 creates the seam and gives the runtime
  crate direct helper coverage; it does not by itself remove the heavy
  `graphshell` compile for full iced/egui parity.
- Do not hide shell ownership behind a giant trait just to move code. If a
  projection helper needs most of `GraphshellRuntime`, it is not ready for the
  runtime crate.
