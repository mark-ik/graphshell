# Servoshell Debt Clearance Plan

**Date**: 2026-03-08
**Status**: Active — not yet in execution
**Scope**: `shell/desktop/` host, platform, and UI layers
**Prerequisite reading**:
- `2026-03-08_servoshell_residue_audit.md` — the findings this plan addresses
- `2026-02-26_composited_viewer_pass_contract.md` — render pipeline contract
- `../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md`

---

## 1. Thesis

The audit established that the servoshell residue is not incidental. It is a
**structural inversion**: the shell layer still assumes that the host creates
renderers and the graph catches up, when graphshell's architecture requires the
opposite ordering (request/intents enter the authority boundary first; reconcile
applies renderer effects afterward).

The inversion also means the shell layer still collapses several distinct
concepts into one `WebViewId`-shaped control path:

| Concept | Correct owner in graphshell |
|---|---|
| Durable content identity | `NodeKey` today, future core `NodeId`; graph/app reducer |
| Durable graph-view identity | `GraphViewId`; graph-owned scoped view state |
| Durable pane identity | `PaneId`; workbench layout and pane registry |
| Ephemeral renderer identity | `WebViewId` / `RendererId`; host reconcile only |
| Focused input target | Explicit focus/input state keyed by pane/surface |
| Chrome/title/toolbar projection source | Explicit `ChromeProjectionSource` derived from the focused pane's active surface |

Until these are separated, every cleanup will drift back toward servoshell.

This plan works in four phases:

1. **Boundary contract** — define and enforce the node/view/pane/renderer split
2. **Host request boundary + creation inversion** — make workbench/graph
   acceptance happen before renderer creation
3. **Single-active removal** — remove exclusive show/focus semantics
4. **Surface cleanups** — shortcuts, toolbar state, gamepad, UI command enum

Phases 1–2 are blockers for phases 3–4. Within each phase, stages are ordered
so that each stage is a compilable, testable landing.

---

## 2. Phase 1 — Boundary Contract

**Goal**: Give the shell layer an explicit model of the real authority split.
Without this, phases 2–4 will keep re-inventing webview-selection heuristics
under new names.

### Stage 1A — Write the shell/workbench/graph boundary contract

Write a short architectural contract (new doc under `technical_architecture/`)
that defines:

- `NodeKey` today, future `NodeId` — durable content identity; owned by the
  graph/app reducer; never created by the shell host as an implicit side effect
- `GraphViewId` — graph-owned scoped view/lens state; independent of pane
  hosting
- `PaneId` — durable workbench UI-container identity; owned by the pane/layout
  layer; may host a node surface, viewer surface, tool surface, or a
  `GraphViewId`
- `WebViewId` / `RendererId` — ephemeral renderer identity; created only by
  reconcile after authority acceptance; may be destroyed/recreated without
  changing node or pane identity
- `InputTarget`, `DialogOwner`, `VisibleRendererSet`, and
  `ChromeProjectionSource` as separate concepts, not aliases for one selected
  webview
- `ChromeProjectionSource` explicitly replaces the current
  `preferred_input_webview_id` title/toolbar/dialog fallback and is a distinct
  type rather than "just a field on the focused pane" because focused pane,
  active surface, and chrome projection are related but not identical
- the two-authority open flow:
  1. host/UI/Servo emits an explicit open request
  2. workbench authority resolves pane placement / binding
  3. graph/app authority accepts or rejects durable node/open semantics
  4. reconcile creates or binds the renderer

This contract becomes the acceptance criterion for all subsequent stages.

**Done gate**: Doc exists; reviewed; linked from `PLANNING_REGISTER.md` and
`ARCHITECTURAL_CONCERNS.md`.

### Stage 1B — Add `RendererRegistry` and pane attachment lookups

Add a thin `RendererRegistry` struct (not `WebViewCollection`) that:

- maps `WebViewId` ↔ owning `NodeKey`
- maps `PaneId` ↔ attached `WebViewId` when a renderer is currently attached to
  a pane
- exposes lookup methods only; no "activate", "active", or tab-order concept

Do not use `GraphViewId` as the pane key. The workbench remains the authority
for `PaneId -> surface` and `GraphViewId -> scoped view` relationships.

This is a pure additive change. `WebViewCollection` is kept as-is; existing call
sites continue to work. The registry is empty until phase 2 wires creation
through it.

**Done gate**:
- `RendererRegistry` exists and compiles
- `PaneId` is the only UI-container identity used in host registries
- `WebViewCollection` is untouched
- No behavior change

### Stage 1C — Add explicit pane focus and chrome projection placeholders

Add explicit placeholder state to `EmbedderWindow` (or equivalent host state):

- `focused_pane: Option<PaneId>`
- `chrome_projection_source: Option<ChromeProjectionSource>`

This does not change behavior yet. It just makes the missing concepts
compile-visible so later stages stop backsliding into `active_webview_id`.

Remove `active_webview_id` from `WebViewCollection` only after phase 3 proves
it is no longer load-bearing.

**Done gate**:
- Fields exist and compile
- No behavior change
- `active_webview_id` still present in `WebViewCollection`

---

## 3. Phase 2 — Host Request Boundary and Creation Flow Inversion

**Goal**: A renderer must not exist before the workbench/graph authorities have
accepted the corresponding open request. This is the highest-impact change in
the plan.

**Landing rule**: Phase 2 lands through additive shims. Stage 2A introduces the
request type, ingress queue/dispatcher, and accepted-open state representation
without migrating callers. Stages 2B, 2C, and 2D then move Ctrl+T,
`request_create_new`, and bootstrap one entry point at a time. Stage 2E removes
legacy helpers only after all live callers are gone.

### Stage 2A — Introduce an explicit host open request boundary

Define a boundary type for host-originated open requests, for example
`HostOpenRequest` or `OpenSurfaceRequest`, instead of introducing a new
graph-root `GraphIntent::RequestOpenNode`.

The request carries the semantic payload the authorities need, for example:

- `url: Url`
- `parent_node: Option<NodeKey>` when an opener exists
- `source: OpenSurfaceSource` (`KeyboardShortcut`, `WindowOpen`, `InitialBoot`,
  toolbar action, etc.)
- optional placement hints such as `target_pane: Option<PaneId>`
- optional pending host-create token for Servo-owned requests

The request must enter the existing bridge model:

- workbench authority decides where the surface should appear
- graph/app authority decides what durable node/open semantics are accepted
- reconcile remains the only layer allowed to call `create_toplevel_webview`

Do not add a new root `GraphIntent::RequestOpenNode` unless the existing routed
open surfaces (`WorkbenchIntent`, `AppCommand::OpenNode`, or equivalent bridge
types) are demonstrably insufficient.

`WorkbenchIntent`, `AppCommand`, and similar bridge-carrier enums are ingress
surfaces, not the authority boundary by themselves. Merely emitting one of
those carrier values does **not** count as acceptance. For this plan,
"accepted" means the request has been materialized into durable workbench/app
state that `reconcile_webview_lifecycle()` can observe directly: for example, a
resolved pane/surface target plus approved durable open semantics. Pending Servo
create tokens from Stage 2C may be consumed only when reconcile observes that
accepted state.

**Done gate**:
- Boundary type exists and is documented
- Host-originated opens route through existing workbench/app bridge surfaces
- Acceptance semantics are documented explicitly and do not equate carrier
  emission with acceptance
- `reconcile_webview_lifecycle()` is the only creation site for accepted opens

### Stage 2B — Reroute Ctrl+T through the host open request boundary

Replace the Ctrl+T handler so it enqueues an explicit open request and returns.
It must not call `create_and_activate_toplevel_webview` directly.

**Prereq**: Stage 2A has landed the request type, dispatcher/queue, and
accepted-open state representation, even if legacy create helpers still exist
for other call sites.

The exact command surface may be `WorkbenchIntent`, `AppCommand::OpenNode`, or
another existing bridge helper, but the order must be:

1. keyboard shortcut emits open request
2. workbench/graph accept the request
3. reconcile creates and attaches the renderer

**Done gate**:
- Ctrl+T produces an open request, not a direct renderer
- New node/surface appears through the normal routing path
- `create_and_activate_toplevel_webview` call site removed from
  `headed_window.rs`

### Stage 2C — Reroute `request_create_new` through a pending Servo create queue

Servo's `CreateNewWebViewRequest` is an owned callback object. It cannot be
reduced to "just take the URL and create the renderer later" because the
embedder must eventually consume the request object itself to build the new
renderer.

**Prereq**: Stage 2A has landed the request type, dispatcher/queue, and
accepted-open state representation. The pending-create queue is introduced in
this stage and remains additive until Stage 2E deletes the old path.

Add a host-side pending-create queue:

- callback receives owned `CreateNewWebViewRequest`
- host stores it in `PendingCreateRequests` keyed by an opaque token
- host emits `HostOpenRequest { pending_create_token: Some(token), ... }`
  together with opener metadata (`parent_node`, `source`, pane hints)
- if workbench/graph accept the request, reconcile consumes the stored owned
  request and builds the renderer there
- if the request is rejected or superseded, the queue entry is dropped and no
  renderer is created

`notify_create_new_webview` cannot be deleted until this path is real.

**Done gate**:
- `window.open()` from content enters the same open-request boundary as other
  opens
- no renderer is created inside the callback itself
- the owned Servo request is consumed only after acceptance in reconcile
- rejected requests are dropped from `PendingCreateRequests` without consuming
  the Servo callback into a renderer
- `notify_create_new_webview` no longer has live callers

### Stage 2D — Reroute `open_window` bootstrap through the same boundary

The initial boot flow in `RunningAppState::open_window` currently creates the
first renderer eagerly. It should instead:

1. create `EmbedderWindow` with no renderers
2. enqueue an initial `HostOpenRequest`
3. let the first authority/reconcile cycle create the initial renderer

**Prereq**: Stage 2A has landed the request type, dispatcher/queue, and
accepted-open state representation. Stages 2B/2C may still be in flight.

This is the deepest change in phase 2 because it touches boot sequencing. It
may require a boot-specific request source or a guaranteed first reconcile pass
before the window becomes interactive. The native window may exist before the
first renderer exists, but for this plan it is not considered interactive for
renderer-targeted commands until the initial open request has been accepted and
the first reconcile pass has either attached the initial renderer or produced an
explicit boot placeholder/error surface. If the platform requires an earlier
frame, that frame must be an explicit zero-renderer boot frame, not an implicit
fallback to host-created renderer state.

**Done gate**:
- `open_window` does not call `create_and_activate_toplevel_webview`
- `open_window` creates the shell window with zero renderers and queues exactly
  one initial open request
- initial renderer is created in `reconcile_webview_lifecycle()`
- renderer-targeted commands do not become interactive until that accepted boot
  request has been reconciled
- any pre-reconcile frame is an explicit boot placeholder/zero-renderer frame
- initial URL load begins from the reconciled renderer path

### Stage 2E — Delete legacy create-then-notify helpers

Once 2B, 2C, and 2D are complete:

- delete `notify_create_new_webview`
- delete the `GraphSemanticEvent::CreateNewWebView` path if it is now dead
- delete `create_and_activate_toplevel_webview`
- keep `create_toplevel_webview` only as an internal reconcile helper

**Done gate**:
- create-then-notify helpers deleted
- compilation clean

---

## 4. Phase 3 — Single-Active Semantics Removal

**Goal**: Remove the idea that one `WebViewId` is globally "active" and speaks
for input routing, visibility, repaint, and chrome projection simultaneously.

This phase is safe only after phase 2, because phase 2 eliminates the creation
paths that eagerly call `activate_webview`.

### Stage 3A — Audit remaining `activate_webview` call sites

After phase 2, grep all remaining call sites of `activate_webview` and
`active_id()`. For each:

- determine which concept it is actually serving (input focus? visibility?
  toolbar source? dialog ownership?)
- map it to the correct explicit mechanism from phase 1

**Done gate**: Each call site has a documented replacement.

### Stage 3B — Rebase repaint on the visible renderer set

`EmbedderWindow::repaint_webviews` currently paints one preferred-input
renderer. Replace this with painting all renderers attached to panes that the
compositor marks visible.

The source of truth must be:

- the Pass 1 layout/occlusion visibility output defined by
  `2026-02-26_composited_viewer_pass_contract.md`
- `RendererRegistry` for pane/renderer attachment

It must not be:

- `preferred_input_webview_id`
- `active_webview_id`
- creation order

**Done gate**:
- `repaint_webviews` iterates visible renderers, not preferred-input renderer
- visible renderer selection is sourced from the compositor's Pass 1
  pane/surface visibility result
- multi-pane: all visible panes repaint
- `preferred_input_webview_id` is no longer consulted for repaint

### Stage 3C — Remove `active_webview_id` from `WebViewCollection`

Once `activate_webview` call sites are replaced (Stage 3A) and repaint is
rebased (Stage 3B), remove:

- `active_webview_id: Option<WebViewId>` field
- `activate_webview()` method
- `activate_webview_by_index()` method
- `active_id()` method
- `get_active_webview_index()` on `EmbedderWindow`
- `activate_webview_by_index()` on `EmbedderWindow`
- `activate_webview()` on `EmbedderWindow`
- `create_and_activate_toplevel_webview` (already deleted in Stage 2E)

`WebViewCollection` becomes a pure registry with no "active" concept.

**Done gate**:
- All deleted; compilation clean
- `preferred_input_webview_id` fallback to `active_id()` removed or replaced
  with explicit pane-focus lookup

### Stage 3D — Rebase chrome/title projection on `PaneId` and `ChromeProjectionSource`

`preferred_input_webview_id` is currently used as the implicit source for:

- window title
- toolbar nav state (back/forward/load)
- dialog anchor fallback

Replace each with explicit lookup keyed by pane/surface ownership:

- window title: from `ChromeProjectionSource`
- toolbar nav state: from the focused pane's active surface when that surface
  is node-backed
- dialog anchor: from pane ownership, not a global fallback

Tool panes and viewer panes must be allowed to provide their own projection, or
explicitly provide none. "Focused pane's node" is not a valid universal rule.

`preferred_input_webview_id` can then be deleted or reduced to a pure
input-routing helper with no title/toolbar/dialog authority.

**Done gate**:
- window title source is explicit
- toolbar state source is explicit
- dialog ownership is pane-derived
- `preferred_input_webview_id` no longer acts as a chrome fallback

---

## 5. Phase 4 — Surface Cleanups

These are the remaining medium/low-severity findings. Most can land
independently after phase 3.

### Stage 4A — Remove tab-navigation keyboard shortcuts

Delete from `headed_window.rs`:

- Ctrl+1 through Ctrl+9 shortcuts calling `activate_webview_by_index`
- Ctrl+PageDown / Ctrl+PageUp cycling shortcuts

These shortcuts have no graphshell-appropriate equivalent. If spatial
navigation shortcuts are desired later, they are a separate feature.

**Done gate**: Shortcuts deleted; no compilation errors.

### Stage 4B — Route browser verbs through graph/app/workbench commands

The following shortcuts in `handle_intercepted_key_bindings` currently call
Servo directly:

- Ctrl+R -> reload
- Ctrl+W -> close current renderer
- Alt+Right -> go forward
- Alt+Left -> go back

Replace each with the appropriate routed command surface. The exact target may
be `GraphIntent`, `AppCommand`, `WorkbenchIntent`, or a small bridge helper,
but it must preserve the existing two-authority model instead of introducing a
new shell-direct control path. Emitting a bridge-carrier command is ingress,
not the effect boundary; the actual Servo operation still occurs only when
reconcile observes accepted routed state and applies the renderer effect.

**Done gate**:
- Each verb goes through graph/app/workbench routing first
- Carrier emission is not treated as effect application
- No direct `.reload()` / `.go_forward()` / `.go_back()` calls in keyboard
  handling code

### Stage 4C — Route gamepad through `ActionRegistry`

`AppGamepadProvider::handle_gamepad_events` currently takes an active renderer
and dispatches `InputEvent::Gamepad` directly. Change it to:

- produce graph/app/workbench actions for navigation/selection events
- dispatch raw `InputEvent::Gamepad` only for content-bound events targeting
  the focused pane's attached renderer

This aligns with `2026-02-24_control_ui_ux_plan.md`.

**Done gate**:
- `handle_gamepad_events` no longer takes a global active renderer
- navigation/selection events flow through `ActionRegistry`
- content scroll/pointer events route to the focused pane's renderer

### Stage 4D — Scope toolbar state to the focused pane and projection source

In `gui.rs` / `ToolbarState`:

- `location`, `location_dirty`, `location_submitted` become pane-scoped
  projections
- `can_go_back`, `can_go_forward`, `load_status` become pane-scoped projections
- `location_has_focus` / `request_location_submit` become scoped to the focused
  pane's input context

The global toolbar state becomes a cache/projection of
`ChromeProjectionSource`, not the source of truth.

If the focused pane is tool-backed or viewer-backed rather than node-backed,
the toolbar must project that explicitly instead of reading stale node state.

**Done gate**:
- toolbar state reads from explicit pane/surface projection
- global fields are projections, not sources
- multiple panes open: toolbar reflects the focused pane correctly

### Stage 4E — Separate durable favicon projection from renderer cache

The codebase already has both node-keyed tile favicon state and
renderer-keyed raw texture caches. The architectural requirement is not "all
favicon maps must become node-keyed"; it is:

- durable favicon identity belongs to `NodeKey` today, future `NodeId`
- renderer-keyed caches are allowed only as ephemeral implementation detail
- no toolbar/pane/UI semantics should treat `WebViewId` as the durable favicon
  owner

Audit remaining favicon flows and finish rekeying any durable semantics away
from `WebViewId`.

**Done gate**:
- durable favicon projection is node-keyed
- any remaining `WebViewId` favicon maps are explicitly renderer-local caches
- UI lookups do not treat `WebViewId` as durable favicon identity

### Stage 4F — Migrate `UserInterfaceCommand::ReloadAll` to routed intent/app command

Map `ReloadAll` onto the existing routed command surface. Delete
`UserInterfaceCommand` if this is the final caller.

**Done gate**:
- enum deleted or reduced to zero live architectural responsibility
- compilation clean

### Stage 4G — Anchor dialogs to pane ownership

Update dialog dispatch to look up dialog ownership by `PaneId`, with
`RendererRegistry` supplying renderer attachment and workbench state supplying
the pane/surface binding.

Do not use `GraphViewId` as a pane surrogate.

**Done gate**:
- dialog dispatch uses pane ownership
- no global `focused_input_webview_id` fallback

---

## 6. Phase Dependency Map

```text
Phase 1A (boundary contract doc)
    └── Phase 1B (RendererRegistry + pane attachment lookups)
            └── Phase 1C (focused_pane + chrome_projection_source placeholders)
                    └── Phase 2A (host open request boundary)
                            ├── Phase 2B (Ctrl+T reroute)
                            ├── Phase 2C (pending Servo create queue)
                            └── Phase 2D (open_window bootstrap reroute)
                                    └── Phase 2E (delete legacy create helpers)
                                            └── Phase 3A (audit activate call sites)
                                                    ├── Phase 3B (rebase repaint)
                                                    ├── Phase 3C (remove active_webview_id)
                                                    └── Phase 3D (rebase chrome projection)
                                                            ├── Phase 4A (tab shortcuts)
                                                            ├── Phase 4B (browser verbs)
                                                            ├── Phase 4C (gamepad)
                                                            ├── Phase 4D (toolbar state)
                                                            ├── Phase 4E (favicon projection)
                                                            ├── Phase 4F (ReloadAll)
                                                            └── Phase 4G (dialog anchoring)
```

Phases 4B–4G are largely independent and can be parallelized after phase 3.
Stage 4A must land before or together with Stage 3C, because Stage 3C deletes
the activation/index methods that the tab-shortcut handlers currently call.

Cross-plan sequencing note: Phase 2A and Phase 4B should land against the
stabilized bridge-carrier surfaces from Stage 1 of
`2026-03-08_graph_app_decomposition_plan.md` when available. This is a staging
aid, not a hard architectural dependency of the debt-clear plan itself.

---

## 7. Acceptance Criteria (Plan-Level)

The plan is complete when:

1. No shell-layer code creates a renderer outside of
   `reconcile_webview_lifecycle()`, except for storing a pending owned Servo
   create request.
2. No host callback creates a renderer before workbench/graph acceptance of an
   explicit open request. Carrier emission alone does not count as acceptance.
3. No shell-layer code reads `active_webview_id` or calls `activate_webview`.
4. Repaint is driven by the visible renderer set, not by a focus heuristic.
5. Window title, toolbar state, and dialog anchoring are derived from explicit
   pane/surface projections.
6. Keyboard, toolbar, and gamepad navigation verbs go through routed
   graph/app/workbench command surfaces before touching Servo APIs.
7. `WebViewCollection` has no "active" concept; it is a pure registry.
8. Durable favicon state is keyed by node identity; any renderer-keyed favicon
   cache is explicitly ephemeral.
9. `UserInterfaceCommand` no longer carries architectural control-flow
   responsibility.

---

## 8. Relation to Other Active Plans

| Plan | Relationship |
|---|---|
| `2026-03-08_graph_app_decomposition_plan.md` | Adjacent and partially coupled. This plan will touch app intent/routing support surfaces and should stage with graph-app extraction rather than pretending the host boundary is isolated. |
| `2026-03-08_render_mod_decomposition_plan.md` | Adjacent. Stage 3B depends on clearer visible-renderer/compositor seams, so render decomposition should preserve those boundaries rather than re-embed host heuristics. |
| `2026-03-08_servoshell_residue_audit.md` | This plan implements the audit's remediation section. |
| `2026-02-26_composited_viewer_pass_contract.md` | Stage 3B (repaint rebase) must stay consistent with the three-pass composition model. |
| `../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md` | Phase 1A is prerequisite input for core extraction. Cleaner node/view/pane/renderer boundaries reduce what must remain host-only. |
| `2026-03-06_reducer_only_mutation_enforcement_plan.md` | Strongly aligned. Phase 2 removes host-first mutation paths that currently escape reducer-only discipline. |
| `2026-02-24_control_ui_ux_plan.md` | Stage 4C (gamepad) and Stage 4D (toolbar) implement the action-routing and pane-scoped projection requirements from that plan. |

---

## 9. Non-Goals

- No changes to Servo's embedder API surface.
- No crate split during this plan; `graphshell-core` extraction is a separate
  plan.
- No UI redesign; toolbar layout and pane chrome are separate concerns.
- No new graph features; this plan removes servoshell assumptions only.
- No requirement that every renderer-local cache become durable state.
