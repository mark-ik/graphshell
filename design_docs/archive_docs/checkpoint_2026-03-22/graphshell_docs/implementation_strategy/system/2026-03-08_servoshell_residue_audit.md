# Servoshell Residue Audit

**Date**: 2026-03-08
**Status**: Historical audit — findings substantially actioned; retained as pre-remediation record
**Scope**: `shell/desktop/` — host, platform, and UI layers
**Related**:
- `2026-03-08_graph_app_decomposition_plan.md`
- `../technical_architecture/ARCHITECTURAL_CONCERNS.md`
- `../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md`
- `../viewer/2026-02-26_composited_viewer_pass_contract.md`

**Historical note (2026-03-22)**:
- This audit captured the state of the shell layer before the debt-clear and embedder decomposition follow-through landed.
- It is no longer an active source of open findings. Use it as a historical explanation of why the cleanup work was necessary, not as a current-state inventory.

---

## 1. Background

Graphshell was forked from servoshell. The Phase E1 rename pass
(`ServoShellWindow` → `EmbedderWindow`, etc.) cleaned up identifiers but did
not audit the underlying control-flow assumptions. This document records the
servoshell-origin patterns that remain in the shell layer and explains why
each conflicts with graphshell's intended architecture.

The two-phase apply model (`apply_intents()` + `reconcile_webview_lifecycle()`)
and the `GraphSemanticEvent` boundary are the authoritative integration
contracts. Any shell-layer code that bypasses those contracts is residue.

The deeper problem is not just leftover naming or shortcuts. Servoshell
collapsed several different concepts into one "active webview" control path:

- durable content identity
- focused input target
- visible renderer target
- toolbar/title/status source
- dialog owner
- repaint target

Graphshell's architecture requires those concepts to be separate. As long as
the shell layer still treats one `WebViewId` as standing in for all of them,
servoshell assumptions will continue to leak through even when identifiers have
been renamed.

---

## 2. Findings by File

---

### 2.1 `shell/desktop/host/window.rs`

#### 2.1.1 `WebViewCollection::activate_webview` (lines ~133–141)

```rust
pub(crate) fn activate_webview(&mut self, id_to_activate: WebViewId) {
    assert!(self.creation_order.contains(&id_to_activate));
    self.active_webview_id = Some(id_to_activate);
    if let Some(webview) = self.webviews.get(&id_to_activate) {
        webview.show();
        webview.focus();
    }
}
```

**Origin**: Verbatim servoshell. Enforces a single globally-active webview by
calling `show()` + `focus()` exclusively on one ID.

**Conflict**: Graphshell's pane layout makes multiple webviews simultaneously
visible and does not have a single "active" webview. Visibility is managed by
the compositor/render pass contract, not by this method.

**Impact**: Every call to `activate_webview` is a potential silent layout
override that bypasses the intent system.

---

#### 2.1.2 `activate_webview_by_index` / `get_active_webview_index` (lines ~269–282)

```rust
pub(crate) fn activate_webview_by_index(&self, index_to_activate: usize) { ... }
pub(crate) fn get_active_webview_index(&self) -> Option<usize> { ... }
```

**Origin**: Servoshell tab-bar switching. Identifies webviews by their
sequential creation order — i.e. tab position.

**Conflict**: Graph nodes are not linearly ordered. There is no meaningful
"index" for a node in a spatial graph.

**Impact**: Dead concept. The only call sites are the keyboard shortcuts
documented in §2.2.

---

#### 2.1.3 `create_and_activate_toplevel_webview` (lines ~150–157)

```rust
pub(crate) fn create_and_activate_toplevel_webview<T>(&self, state: Rc<T>, url: Url) -> WebView {
    let webview = self.create_toplevel_webview(state, url);
    self.activate_webview(webview.id());
    webview
}
```

**Origin**: Servoshell "new tab" creation — create then immediately make
active.

**Conflict**: Graph node creation should be driven by a `GraphIntent`, not by
a direct webview call. The `activate_webview` call inside re-introduces the
single-active assumption (§2.1.1).

**Impact**: New webview creation from Ctrl+T bypasses the graph reducer
entirely (see §2.2 and §2.4).

---

#### 2.1.4 `repaint_webviews` (lines ~176-188)

```rust
pub(crate) fn repaint_webviews(&self) {
    let Some(webview_id) = self.platform_window().preferred_input_webview_id(self) else {
        return;
    };
    let Some(webview) = self.webview_by_id(webview_id) else {
        return;
    };
    webview.paint();
}
```

**Origin**: Servoshell single-focused-webview paint path.

**Conflict**: Graphshell's compositor contract is about the set of visible
renderers, not the preferred input target. Painting should be driven by the
composited visible renderer set, not by one heuristic "active" webview.

**Impact**: Repaint ownership is still coupled to focus/input heuristics. This
is a deeper control-flow leak than shortcut handling because it affects the
frame loop itself.

---

### 2.2 `shell/desktop/host/headed_window.rs`

#### 2.2.1 Tab-navigation keyboard shortcuts (lines ~440–490)

```rust
.shortcut(CMD_OR_CONTROL, '1', || window.activate_webview_by_index(0))
// ... through '8'
.shortcut(CMD_OR_CONTROL, '9', || { /* last tab */ })
.shortcut(Modifiers::CONTROL, Key::Named(NamedKey::PageDown), || { /* next tab */ })
.shortcut(Modifiers::CONTROL, Key::Named(NamedKey::PageUp),  || { /* prev tab */ })
```

**Origin**: Standard browser Ctrl+1–9 / Ctrl+PageDown/Up tab switching.

**Conflict**: These directly manipulate webview activation order using a
creation-order index. They have no awareness of graph topology and cannot be
correct in a spatial layout.

**Impact**: Pressing Ctrl+1 in graphshell calls `activate_webview_by_index(0)`
which calls `show()`/`focus()` on the first-created webview, silently
overriding whatever the graph compositor is doing.

---

#### 2.2.2 Ctrl+T new-tab shortcut (lines ~494–501)

```rust
.shortcut(CMD_OR_CONTROL, 'T', || {
    let child_webview = window.create_and_activate_toplevel_webview(
        state.clone(),
        Url::parse("servo:newtab").unwrap(),
    );
    window.notify_create_new_webview(active_webview.clone(), child_webview);
})
```

**Origin**: Servoshell new-tab.

**Conflict**: Creates a webview and activates it before emitting a
`GraphSemanticEvent`. The graph reducer receives the event after the fact —
the webview already exists and is already "active". This inverts the
intended flow (intent → reduce → reconcile).

**Impact**: A new webview created by Ctrl+T is structurally outside the graph
until the event propagates. If the event is dropped or the reducer rejects it,
the webview is orphaned.

---

#### 2.2.3 `location_has_focus` in keyboard routing (lines ~850–855)

```rust
&& self.gui.borrow().location_has_focus()
```

**Origin**: Servoshell URL bar focus check — Enter key submits the location bar.

**Conflict**: The URL bar is a servoshell-era UI concept. Graphshell's
navigation is driven through graph node context, not a global location field.

**Impact**: The Enter key path is gated on a servoshell concept that may not
have a valid graphshell equivalent. If the location bar is removed from the
UI, this branch silently stops working.

---

#### 2.2.4 `for_each_active_dialog` (lines ~538–583)

```rust
pub(crate) fn for_each_active_dialog(
    &self,
    window: &EmbedderWindow,
    focused_input_webview_id: Option<WebViewId>,
    ...
)
```

**Origin**: Servoshell dialog anchoring — dialogs (alerts, confirms, prompts)
are keyed to the single focused webview.

**Conflict**: In a multi-pane layout, dialogs should be anchored to the graph
node/pane that owns the webview, not to a global "focused input webview".

**Impact**: Low severity for now; the function correctly scopes to the given
webview ID. Becomes a problem if two panes can simultaneously show dialogs
from different webviews.

---

#### 2.2.5 Direct browser verbs in `handle_intercepted_key_bindings`

Representative paths:

```rust
.shortcut(CMD_OR_CONTROL, 'R', || active_webview.reload())
.shortcut(CMD_OR_CONTROL, 'W', || { window.close_webview(active_webview.id()); })
.shortcut(CMD_OR_ALT, Key::Named(NamedKey::ArrowRight), || {
    active_webview.go_forward(1);
})
.shortcut(CMD_OR_ALT, Key::Named(NamedKey::ArrowLeft), || {
    active_webview.go_back(1);
})
```

**Origin**: Servoshell browser-window command routing.

**Conflict**: These are still host-direct browser commands operating on a
chosen webview. In graphshell they should resolve to graph/app commands first,
then reconcile into renderer effects. The current shape preserves the old
"browser chrome drives the webview directly" control flow.

**Impact**: Even when the toolbar/UI looks graphshell-native, core navigation
verbs still bypass the reducer and talk to Servo directly.

---

#### 2.2.6 `preferred_input_webview_id` fallback and title/chrome projection

Representative paths:

```rust
fn preferred_input_webview_id(&self, window: &EmbedderWindow) -> Option<WebViewId> {
    if let Ok(gui) = self.gui.try_borrow() {
        return gui
            .focused_node_key()
            .and_then(|node_key| gui.webview_id_for_node_key(node_key));
    }
    window.webview_collection.borrow().active_id()
}
```

```rust
let title = self
    .preferred_input_webview(window)
    .and_then(|webview| { ... })
```

**Origin**: Servoshell single-browsing-context chrome projection, with a
graphshell transitional heuristic layered on top.

**Conflict**: "Preferred input webview", "window title source", and "toolbar
state source" are different concepts. The fallback to `active_id()` preserves
the servoshell assumption that one webview can stand in for them all.

**Impact**: The shell still has a hidden global browsing-context heuristic.
Even when node focus exists, chrome state is still derived through a webview
selection heuristic instead of an explicit pane/node projection contract.

---

### 2.3 `shell/desktop/ui/gui.rs`

#### 2.3.1 URL bar state (lines ~120–130, ~283–291, ~411–417)

```rust
location: String,
location_dirty: bool,
location_submitted: bool,
can_go_back: bool,
can_go_forward: bool,
load_status: LoadStatus,

pub(crate) fn location_has_focus(&self) -> bool { ... }
pub(crate) fn request_location_submit(&mut self) { ... }
```

**Origin**: Servoshell browser toolbar — URL bar with back/forward/load state
for a single browsing context.

**Conflict**: These fields describe one webview's navigation state as if it
were global. In a graph layout, each node has its own load/nav state. There
is no single "location" for the application.

**Impact**: The toolbar currently renders one webview's URL and nav buttons
globally. If a user has multiple panes open, the toolbar reflects only the
"preferred input" webview, which is a servoshell-era heuristic.

---

#### 2.3.2 `favicon_textures` (line ~133)

```rust
favicon_textures: HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
```

**Origin**: Servoshell tab bar — each tab shows its favicon.

**Conflict**: Not wrong per se, but the ownership model is tab-centric. In
graphshell, favicons belong to graph nodes, not webviews directly. This
becomes a mismatch once the `WebViewId` ↔ node mapping is explicit.

**Impact**: Low severity now; becomes a refactor target once graph node
metadata owns favicon state.

---

### 2.4 `shell/desktop/host/running_app_state.rs`

#### 2.4.1 `request_create_new` WebView delegate (lines ~813–834)

```rust
fn request_create_new(&self, parent_webview: WebView, request: CreateNewWebViewRequest) {
    // ...
    window.add_webview(webview.clone());
    window.notify_create_new_webview(parent_webview, webview.clone());  // event emitted here
    if self.app_preferences.webdriver_port.get().is_none() {
        window.activate_webview(webview.id());   // activated immediately after
    }
}
```

**Origin**: Servoshell `window.open()` / popup handling.

**Conflict**: Same inversion as §2.2.2. The webview is added and activated
before the graph reducer can respond. `activate_webview` is called
unconditionally (outside WebDriver mode) — the graph has no opportunity to
decide whether or where to place this new node.

**Impact**: `window.open()` popups and child webviews created by servo content
are structurally outside the graph until after the fact.

---

#### 2.4.2 `open_window` bootstraps by creating a toplevel webview directly

```rust
pub(crate) fn open_window(...) -> Rc<EmbedderWindow> {
    let window = Rc::new(EmbedderWindow::new(...));
    window.create_and_activate_toplevel_webview(self.clone(), initial_url);
    self.embedder_core.insert_window(window.clone());
    ...
}
```

**Origin**: Servoshell window bootstrap assumes the first thing a window owns
is a top-level browsing context.

**Conflict**: Graphshell windows/panes should be created around graph/workspace
state, with renderer creation delegated to reconcile. Creating the first
webview as part of window bootstrap keeps the host-first control-flow model.

**Impact**: The "initial page" still exists because the host creates it, not
because the graph reducer accepted it. This makes future pane-first and
node-first boot paths harder.

---

#### 2.4.3 `UserInterfaceCommand` enum (lines ~153–157)

```rust
pub(crate) enum UserInterfaceCommand {
    ReloadAll,
}
```

**Origin**: Servoshell had a richer `UserInterfaceCommand` enum
(`Go(String)`, `Back`, `Forward`, `Reload`, `NewWebView`, `CloseWebView`,
`NewWindow`). The graphshell rename pass removed most variants but kept the
pattern.

**Conflict**: `ReloadAll` is the only remaining variant. It is dispatched
outside `GraphIntent` — a legacy command path that bypasses the intent system.

**Impact**: Minor. If `ReloadAll` is the only needed command, it should be
expressed as a `GraphIntent` variant or an `AppCommand`, not a parallel
command enum.

---

### 2.5 `shell/desktop/host/gamepad.rs`

#### 2.5.1 `handle_gamepad_events` signature (lines ~42–135)

```rust
pub(crate) fn handle_gamepad_events(&self, active_webview: WebView) {
    // ...
    active_webview.notify_input_event(InputEvent::Gamepad(event));
}
```

**Origin**: Servoshell single-focus gamepad routing — all gamepad events go to
the one active webview.

**Conflict**: Graphshell's gamepad design (per `2026-02-24_control_ui_ux_plan.md`)
routes input through ActionRegistry and the radial/context menu system, not
directly to a webview. Gamepad navigation events should produce graph intents
(node selection, pane focus), not raw `InputEvent::Gamepad` dispatches.

**Impact**: Gamepad input does not interact with the graph layer at all. All
events go directly to a webview, bypassing the radial menu and ActionRegistry.

---

## 3. Cross-Cutting Structural Gaps

There is no bidirectional mapping between `WebViewId` and the owning graph node
anywhere in the shell layer. This is the root cause of most findings above.
Because the shell layer does not know which graph node owns a given webview,
it cannot route events, visibility changes, or input through the graph reducer.
Instead it falls back to servoshell heuristics: creation-order index, single
"active" ID, global toolbar state.

Until this mapping exists, the shell layer will continue to need servoshell
fallbacks. The mapping is a prerequisite for cleanly removing the patterns
listed in §2.

### 3.1 Missing identity split

The shell layer still does not explicitly model the distinction between:

- durable content identity (`NodeKey` today, future `NodeId`)
- durable graph-view identity (`GraphViewId`)
- durable UI container identity (`PaneId`)
- ephemeral renderer identity (`WebViewId` / `RendererId`)

Servoshell did not need this split because one top-level browsing context was
the unit of identity. Graphshell does.

### 3.2 Overloaded `active_webview` / `preferred_input_webview`

The shell still uses one selected webview for multiple independent jobs:

- input routing target
- global title/toolbar projection source
- repaint target
- dialog ownership fallback
- focus retargeting fallback

Those should be separate state machines. As long as they are collapsed into one
heuristic, the control flow will keep drifting back toward servoshell.

### 3.3 Hidden host-first control-flow assumption

Several flows still assume:

1. host creates or activates a webview
2. shell emits a semantic event about what just happened
3. graph state catches up afterward

Graphshell wants the inverse:

1. intent/request enters the reducer boundary
2. graph/workspace state accepts or rejects it
3. reconcile creates, destroys, shows, hides, or focuses renderers

This inversion is the architectural center of gravity. The audit findings are
symptoms of places where the old ordering still survives.

### 3.4 Hidden repaint/chrome assumptions

The audit should treat the following as first-class residue, not incidental UI
details:

- repaint keyed off `preferred_input_webview`
- title keyed off `preferred_input_webview`
- toolbar nav state keyed off a single chosen webview

These are servoshell assumptions about a single browsing context wearing many
hats. In graphshell they must become explicit projections from pane/node state.

---

## 4. Severity Summary

| Finding | File | Severity | Bypasses Intent System? |
|---------|------|----------|------------------------|
| `activate_webview` exclusive show/focus | window.rs | **High** | Yes — direct webview call |
| `activate_webview_by_index` / index concept | window.rs | **High** | Yes |
| `create_and_activate_toplevel_webview` | window.rs | **High** | Yes |
| `repaint_webviews` keyed to preferred input | window.rs | **High** | Yes — render loop uses focus heuristic |
| Ctrl+1–9 / PageDown/Up shortcuts | headed_window.rs | **High** | Yes |
| Ctrl+T inverted create flow | headed_window.rs | **High** | Yes — reducer is notified after |
| direct browser verbs in `handle_intercepted_key_bindings` | headed_window.rs | **High** | Yes — direct webview commands |
| `request_create_new` inverted flow | running_app_state.rs | **High** | Yes — reducer is notified after |
| `open_window` direct toplevel webview bootstrap | running_app_state.rs | **High** | Yes — host creates before reducer |
| `handle_gamepad_events` direct dispatch | gamepad.rs | **High** | Yes — bypasses ActionRegistry |
| `location_has_focus` / URL bar state | gui.rs + headed_window.rs | **Medium** | No — UI only, but concept is wrong |
| `preferred_input_webview` as chrome/title fallback | headed_window.rs | **Medium** | Partial — hidden authority heuristic |
| `UserInterfaceCommand::ReloadAll` | running_app_state.rs | **Low** | Partial |
| `for_each_active_dialog` single-focus | headed_window.rs | **Low** | No |
| `favicon_textures` by WebViewId | gui.rs | **Low** | No |

---

## 5. Recommended Remediation Order

### Step 1 — Write the shell identity/control-flow contract

Before deleting more residue, write one short architectural contract that
defines:

- `NodeKey` today, future `NodeId`, as durable content identity
- `GraphViewId` as graph-owned scoped view identity
- `PaneId` as durable UI-container identity
- `WebViewId`/`RendererId` as ephemeral renderer identity
- the difference between input target, dialog owner, visible renderer, and
  toolbar/title projection source

Without this contract, the codebase will keep replacing one servoshell
heuristic with another.

### Step 2 — Establish `WebViewId` ↔ node and pane bindings

Add thin registries in the shell host layer (or in `GraphBrowserApp`) that
map:

- `WebViewId` ↔ owning graph node (`NodeKey` today)
- `PaneId` ↔ renderer id when a renderer is attached
- pane/surface identity ↔ node/view identity through the existing workbench
  layer, not by treating `GraphViewId` as a pane surrogate

This unlocks correct event routing and makes the high-severity findings above
actionable.

### Step 3 — Invert the new-webview creation flow

`create_and_activate_toplevel_webview` and `request_create_new` should both
enter an explicit host open-request boundary first and let the workbench/graph
authorities accept or reject the request before
`reconcile_webview_lifecycle()` creates the renderer. For Servo
`request_create_new`, that means storing the owned callback request object in a
pending host queue until acceptance rather than extracting a URL and creating a
renderer immediately.

### Step 4 — Remove single-active semantics

Delete the idea that one window-global webview is:

- the visible renderer
- the input owner
- the chrome projection source
- the repaint target

Replace it with explicit state for focus/input, chrome projection, and visible
renderers.

### Step 5 — Remove tab-navigation shortcuts

Ctrl+1–9, Ctrl+PageDown/Up, and Ctrl+T should either be removed or rerouted
to graph-aware intents (e.g. node selection by some non-index criterion).
`activate_webview_by_index` and `get_active_webview_index` can be deleted once
these shortcuts are gone.

### Step 6 — Move browser verbs behind graph/app commands

Reload, back, forward, close, new-page, and similar shell actions should no
longer call Servo directly from keyboard or toolbar code. They should resolve
through `GraphIntent` or `AppCommand`, with reconcile performing the renderer
effect.

### Step 7 — Route gamepad through ActionRegistry

`handle_gamepad_events` should produce `GraphIntent` or `AppCommand` values
for navigation/selection events, dispatching raw `InputEvent::Gamepad` only
for content-bound events (scroll, pointer) on the focused pane's webview.

### Step 8 — Scope toolbar/title/dialog state to pane ownership

Replace global `location`, `can_go_back`, `can_go_forward`, `load_status` in
`Gui`/`ToolbarState` with per-pane state sourced from the graph node owning
the currently focused pane when the pane is node-backed. The global fields
become a projection, not the source of truth.

Dialogs should likewise be anchored to pane/node ownership, not a fallback
"focused input webview" concept.

### Step 9 — Rebase repaint and visibility on the compositor contract

The frame loop should paint the set of visible renderers selected by the
compositor/render pass. `preferred_input_webview` must not be used as a paint
proxy.

### Step 10 — Migrate `ReloadAll` to `GraphIntent`

Add a `GraphIntent::ReloadAll` variant (or equivalent `AppCommand`) and delete
the `UserInterfaceCommand` enum.

### Step 11 — Add explicit invariants/tests

Add tests that fail if:

- host callbacks create renderers before reducer acceptance
- shell code routes by creation-order index
- repaint is keyed off preferred input rather than visible renderers
- chrome state reads from a window-global active webview heuristic

---

## 6. Technical Opportunities

The cleanup is not just risk reduction; it enables better architecture:

- **Renderer as ephemeral lease**: treating `WebViewId` as disposable makes
  crash recovery, renderer swapping, headless operation, and mobile/browser
  hosts cleaner.
- **Pane-first UX**: once toolbar/title/dialog ownership is pane-derived, the
  multi-pane model becomes explicit instead of a browser-tab model in disguise.
- **Cleaner core extraction**: the more renderer lifecycle becomes pure host
  reconciliation, the easier it is to move durable identity and reducer logic
  into `graphshell-core`.
- **Better projection discipline**: favicons, nav state, and load status can
  become node/pane projections rather than shell-owned globals.
- **Composable input routing**: ActionRegistry can become the first stop for
  keyboard/gamepad policy, with raw Servo input only for content-bound events.

---

## 7. Non-Goals

- No changes to Servo's embedder API surface.
- No removal of the `WebViewCollection` struct itself — it is still needed as
  a registry; only its "active" semantics are removed.
- No UI redesign in this pass — toolbar layout changes belong to a separate
  UI plan.
