<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Sector A — Content Pipeline Registry Development Plan

**Doc role:** Implementation plan for the content pipeline registry sector
**Status:** Active / planning
**Date:** 2026-03-08
**Parent:** [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md)
**Registries covered:** `ProtocolRegistry`, `ViewerRegistry`, `ViewerSurfaceRegistry`, `LensRegistry`
**Specs:** [protocol_registry_spec.md](protocol_registry_spec.md), [viewer_registry_spec.md](viewer_registry_spec.md), [viewer_surface_registry_spec.md](viewer_surface_registry_spec.md), [lens_compositor_spec.md](lens_compositor_spec.md)

---

## Purpose

The content pipeline is the chain that resolves a URI into a rendered surface. Every node that
displays content passes through it. The chain runs left to right:

```
URI
 └─► ProtocolRegistry        scheme → MIME hint (+ cancellable async probe)
      └─► ViewerRegistry      MIME → ViewerId (explicit selection, diagnosable)
           └─► ViewerSurface  ViewerId → viewport rendering policy
                └─► Lens      MIME + graph context → LensProfile (overlay / filter)
```

All four registries must be wired together before the pipeline is testable end-to-end.
They are developed in sequence within this sector: Protocol first, then Viewer, then
ViewerSurface, then Lens — because each step depends on the output of the previous one.

---

## Current State

| Registry | Struct | API completeness | Wired | Tested | Diag |
|---|---|---|---|---|---|
| `ProtocolRegistry` | ✅ | ⚠️ MIME probe absent; no mod-provided scheme handlers | ✅ | ✅ | ✅ |
| `ViewerRegistry` | ✅ (atomic) | ⚠️ selection dispatched but viewer capability declaration incomplete | ✅ | ⚠️ partial | ✅ |
| `ViewerSurfaceRegistry` | ❌ | ❌ | ❌ | ❌ | ❌ |
| `LensRegistry` | ⚠️ one-line stub | ❌ | ⚠️ partial | ❌ | ❌ |

### Key gaps

**ProtocolRegistry:**
- `resolve()` returns a MIME hint from extension or data URI prefix only — no async HEAD probe for `http(s)://` URIs.
- Scheme handlers are hardcoded at `new()`. Mods cannot register scheme handlers at runtime.
- `verso://` and `graphshell://` scheme routing is partially hardcoded in `graph_app.rs`; it should be owned by the registry.
- Cancellation is modelled but not propagated into async probe (no actual async resolution exists yet).

**ViewerRegistry:**
- Viewer selection uses `select_for_mime()` but viewer capability declarations (`describe_viewer`) are not surfaced through the domain registry; they exist only in atomic internals.
- No viewer fallback chain is tested: unsupported MIME → fallback viewer path is untested.
- `phase2_resolve_navigation_with_protocol` in `mod.rs` dispatches both protocol and viewer but does not chain their outputs (MIME hint from protocol is not fed into viewer selection).

**ViewerSurfaceRegistry:**
- Does not exist. The viewport policy (scroll behaviour, zoom constraints, overlay affordance mode) is currently per-tile hardcoded state in `tile_behavior.rs`.
- The `viewer_surface_registry_spec.md` defines a `viewport-authority` policy: viewport behaviour is the registry's responsibility, not the tile's.

**LensRegistry:**
- `registries/lens.rs` is a single re-export line: `pub use crate::atomic::lens::LENS_ID_DEFAULT`.
- No `LensRegistry` struct, no `register_lens()`, no `resolve_for_content()`, no composition.
- `phase2_resolve_lens()` in `mod.rs` calls atomic lens directly; it should call through the domain registry.
- The `lens_compositor_spec.md` defines composition of graph-surface, presentation, and knowledge/filter configuration as a three-part `LensProfile` — none of this is implemented.

---

## Phase A1 — ProtocolRegistry: Async MIME probing and mod-provided handlers

**Unlocks:** Viewer selection with reliable MIME; SR4 done gate for `verso://` routing.

### A1.1 — Chain MIME hint from `ProtocolRegistry` into `ViewerRegistry` selection

Currently `phase2_resolve_navigation_with_protocol` calls both registries but does not thread the
MIME hint from the protocol resolution into the viewer select call. Fix the dispatch function to
pass the resolved MIME through:

```rust
let proto_outcome = registries.protocol.resolve_with_control(uri, control);
let mime = proto_outcome.mime_hint.unwrap_or(MIME_FALLBACK);
let viewer_id = registries.viewer.select_for_mime(&mime);
```

Diagnostics: emit `DIAG_VIEWER_SELECT` with the chained MIME source annotated (extension-inferred
vs probed vs fallback).

**Done gates:**
- [ ] `phase2_resolve_navigation_with_protocol` passes resolved MIME into `ViewerRegistry::select_for_mime()`.
- [ ] `DIAG_VIEWER_SELECT` records whether MIME came from extension inference, data URI, or fallback.
- [ ] Unit test: data URI resolves MIME → viewer selected without probe.

### A1.2 — Async HTTP HEAD probe for MIME inference

For `http://` and `https://` URIs, launch a background HEAD probe via `ControlPanel`. The probe:
1. Emits a `GraphIntent::NodeMimeResolved { node_key, mime }` when the `Content-Type` response
   arrives.
2. The reducer handles `NodeMimeResolved` by updating the node's MIME field and emitting a
   `SignalKind::Navigation(MimeResolved)` via the `SignalRoutingLayer`.
3. `ViewerRegistry` observers pick up the signal and re-select the viewer.

The probe task must respect the `ProtocolResolveControl` cancellation token so navigating away
cancels the in-flight probe.

**Done gates:**
- [ ] `ContentTypeProber` struct in `shell/desktop/runtime/protocol_probe.rs`.
- [ ] `ControlPanel::spawn_protocol_probe()` wraps the prober as a supervised worker.
- [ ] `GraphIntent::NodeMimeResolved` variant defined and handled in `apply_reducer_intents()`.
- [ ] Navigation cancellation propagates to cancels the probe task.
- [ ] Unit test: probe task emits `NodeMimeResolved` intent on successful HEAD response.
- [ ] Unit test: probe task cancels cleanly when control token fires.

### A1.3 — Mod-provided scheme handlers

Allow native mods to register URI scheme handlers via `ProtocolRegistry::register_scheme_handler()`.
This unblocks PDF mods, local-file mods, and graphshell-internal schemes from being hardcoded.

```rust
pub fn register_scheme_handler(
    &mut self,
    scheme: &str,
    handler: Box<dyn ProtocolHandler + Send + Sync>,
    source: HandlerSource,  // Builtin | NativeMod(ModId)
)
```

Mod-registered handlers participate in the same `resolve()` and `resolve_with_control()` paths.
Conflicting scheme registrations emit `DIAG_PROTOCOL_RESOLVE` at `Warn` severity.

**Done gates:**
- [ ] `ProtocolHandler` trait defined.
- [ ] `register_scheme_handler()` implemented with conflict diagnostics.
- [ ] `verso://` and `graphshell://` routing moved from `graph_app.rs` into registered handlers.
- [ ] Unit test: mod-registered scheme resolves correctly; conflict emits warn diagnostic.

---

## Phase A2 — ViewerRegistry: Capability declarations and fallback chain

**Unlocks:** Diagnosable viewer selection; viewer platform lane (#92) dependency.

### A2.1 — Surface `describe_viewer` through the domain registry

The `viewer_registry_spec.md` requires `describe_viewer(id) -> ViewerCapability`. Currently
viewer capability data exists only in atomic internals. Expose it through `RegistryRuntime`:

```rust
pub fn describe_viewer(&self, id: &ViewerId) -> Option<ViewerCapability>
```

`ViewerCapability` includes: supported MIME types, accessibility mode support, render mode
compatibility (`TileRenderMode`), and overlay affordance flag.

**Done gates:**
- [ ] `ViewerCapability` struct defined in `registries/viewer.rs` (or atomic viewer module).
- [ ] `RegistryRuntime::describe_viewer()` exposed.
- [ ] `DIAG_VIEWER_SELECT` records the capability of the selected viewer.

### A2.2 — Test the fallback floor

The `viewer_registry_spec.md`'s `fallback-floor` policy requires that unsupported MIME always
resolves to a canonical fallback viewer (never a panic or silent no-op).

**Done gates:**
- [ ] Unit test: unknown MIME → fallback viewer selected, `DIAG_VIEWER_SELECT` emits.
- [ ] Unit test: empty MIME string → fallback viewer selected.
- [ ] Fallback viewer is documented as a `VIEWER_ID_FALLBACK` constant.

---

## Phase A3 — ViewerSurfaceRegistry: Viewport policy authority

**Unlocks:** Tile-tree viewport behaviour separated from tile_behavior.rs hardcoding; viewer platform lane (#92).

### A3.1 — Define `ViewerSurfaceRegistry` struct and `ViewportPolicy`

```rust
pub struct ViewportPolicy {
    pub scroll_mode: ScrollMode,         // Paged | Continuous | Fixed
    pub zoom_constraints: ZoomConstraints,
    pub overlay_affordance: OverlayAffordance,  // from TileRenderMode enum
    pub focus_ring_visible: bool,
    pub accessibility_hints: Vec<AccessibilityHint>,
}

pub struct ViewerSurfaceRegistry {
    policies: HashMap<ViewerId, ViewportPolicy>,
    fallback: ViewportPolicy,
}
```

Register built-in viewport policies for `VIEWER_ID_WEB`, `VIEWER_ID_MARKDOWN`,
`VIEWER_ID_IMAGE`, `VIEWER_ID_FALLBACK`.

**Done gates:**
- [ ] `ViewerSurfaceRegistry` struct in `shell/desktop/runtime/registries/viewer_surface.rs`.
- [ ] `resolve_viewport_policy(viewer_id) -> ViewportPolicy` implemented with fallback.
- [ ] Added to `RegistryRuntime` composition root.
- [ ] `DIAG_VIEWER_SELECT` extended with viewport policy source.

### A3.2 — Remove hardcoded viewport state from `tile_behavior.rs`

The `viewport-authority` policy from the spec requires that tile_behavior delegates viewport
decisions to the registry rather than owning them.

**Done gates:**
- [ ] `tile_behavior.rs` calls `registries.viewer_surface.resolve_viewport_policy()` instead of
  hardcoded scroll/zoom constants.
- [ ] Regression test: default viewport policy produces same scroll/zoom behaviour as before.

### A3.3 — Wire `OverlayAffordance` to `TileRenderMode`

`ViewerSurfaceRegistry` is where `TileRenderMode` (from PLANNING_REGISTER §0) and overlay
affordance policy converge. The `overlay_affordance` field on `ViewportPolicy` must match the
`TileRenderMode` declared by the viewer's capability.

**Done gates:**
- [ ] `ViewportPolicy::overlay_affordance` derived from `ViewerCapability::render_mode`.
- [ ] Spec/code parity: PLANNING_REGISTER §0 overlay affordance policy is implemented here (#99).

---

## Phase A4 — LensRegistry: Compositor implementation

**Unlocks:** Knowledge-capture lane (#98); progressive lens overlays; graph-filter capability.

### A4.1 — `LensRegistry` struct and `LensProfile`

```rust
pub struct LensDescriptor {
    pub id: LensId,
    pub display_name: String,
    pub applicable_mime_types: Vec<String>,
    pub priority: u8,
    pub requires_knowledge: bool,    // needs KnowledgeRegistry semantic data
    pub requires_graph_context: bool, // needs graph topology for filtering
}

pub struct LensProfile {
    pub graph_surface: Option<GraphSurfaceLens>,
    pub presentation: Option<PresentationLens>,
    pub knowledge_filter: Option<KnowledgeFilterLens>,
}

pub struct LensRegistry {
    descriptors: HashMap<LensId, LensDescriptor>,
    fallback: LensId,
}
```

Register built-in lenses: `LENS_ID_DEFAULT` (identity, no filter), `LENS_ID_SEMANTIC_OVERLAY`
(tag highlighting), `LENS_ID_FOCUS_DEPTH` (DOI-based dimming — future).

**Done gates:**
- [ ] `LensRegistry` struct in `shell/desktop/runtime/registries/lens.rs` (replacing the stub).
- [ ] `register_lens()`, `resolve_for_content(mime, context) -> Vec<LensId>`, `compose(parts) -> LensProfile`.
- [ ] `LENS_ID_DEFAULT` and `LENS_ID_SEMANTIC_OVERLAY` built-in registrations.
- [ ] Added to `RegistryRuntime`.

### A4.2 — Replace atomic lens call in `phase2_resolve_lens()`

`mod.rs::phase2_resolve_lens()` currently calls atomic lens directly. Replace with:

```rust
let lens_ids = registries.lens.resolve_for_content(&mime, &graph_context);
let profile = registries.lens.compose(&lens_ids);
```

**Done gates:**
- [ ] `phase2_resolve_lens()` calls `LensRegistry` instead of atomic.
- [ ] Unit test: MIME-matched lens resolves to expected LensId.
- [ ] Unit test: unknown MIME resolves to `LENS_ID_DEFAULT` fallback.

### A4.3 — Wire `KnowledgeRegistry` into lens composition

The `lens_compositor_spec.md` requires that `knowledge_filter` in `LensProfile` can carry
semantic class data from `KnowledgeRegistry`. This wiring depends on Sector F progress.

**Done gate (deferred follow-on after Sector F groundwork):**
- [ ] `LensRegistry::compose()` optionally queries `KnowledgeRegistry` when
  `LensDescriptor::requires_knowledge` is true.
- [ ] `LENS_ID_SEMANTIC_OVERLAY` produces a non-empty `knowledge_filter` for nodes with UDC tags.

Reality note (2026-03-10):
- Sector F now publishes `SemanticIndexUpdated` and the GUI/runtime observer path already
  re-resolves registry-backed view lenses on that signal.
- The remaining A4.3 gap is not signal plumbing anymore; it is lens-composition semantics inside
  `LensRegistry` itself (`requires_knowledge`, `knowledge_filter`, and semantic-overlay profile data).

---

## Acceptance Criteria (Sector A complete)

- [ ] A URI entered in the omnibar resolves scheme → MIME → viewer → viewport policy → lens profile
  without any hardcoded constants in the path.
- [ ] The full chain is exercised in a scenario test in `shell/desktop/tests/scenarios/`.
- [ ] `http(s)://` MIME probe completes and triggers viewer re-selection via intent/signal.
- [ ] Mods can register scheme handlers; `verso://` and `graphshell://` are mod-registered.
- [ ] Unsupported MIME always resolves to a fallback viewer (never panics or no-ops).
- [ ] `ViewerSurfaceRegistry` owns viewport policy; `tile_behavior.rs` delegates to it.
- [ ] `LensRegistry` is a real struct; `LENS_ID_SEMANTIC_OVERLAY` produces a meaningful profile.
- [ ] All diagnostic channels in the pipeline (`DIAG_PROTOCOL_RESOLVE`, `DIAG_VIEWER_SELECT`,
  `DIAG_LENS_RESOLVE`) emit with correct severity.

---

## Related Documents

- [protocol_registry_spec.md](protocol_registry_spec.md)
- [viewer_registry_spec.md](viewer_registry_spec.md)
- [viewer_surface_registry_spec.md](viewer_surface_registry_spec.md)
- [lens_compositor_spec.md](lens_compositor_spec.md)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) — routing decision table
- [../2026-03-08_servoshell_debtclear_plan.md](../2026-03-08_servoshell_debtclear_plan.md) — overlay affordance / TileRenderMode dependency
- [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md) — master index
