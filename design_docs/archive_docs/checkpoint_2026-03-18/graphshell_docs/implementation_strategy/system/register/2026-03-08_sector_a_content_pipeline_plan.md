<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Sector A — Content Pipeline Registry Development Plan

**Doc role:** Implementation plan for the content pipeline registry sector
**Status:** Implemented / updated 2026-03-10
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

## Implementation Reality Note (2026-03-10)

Sector A is now implemented against the code structure that actually existed in the repo:

1. `ViewerSurfaceRegistry` was not created as a brand-new runtime registry. The existing
   layout-domain `viewer_surface` authority was promoted and extended to resolve viewer-specific
   surface profiles (`web`, `document`, `embedded`, `native_overlay`).
2. `ProtocolRegistry` and `ViewerRegistry` were already partially chained. Sector A completion
   converged that path, fixed host-suffix MIME inference bugs, and added cancellable HTTP
   content-type probes through `ControlPanel`.
3. `LensRegistry` was not a one-line stub in code; it already existed as an atomic registry.
   Sector A completion promoted it into a content-aware lens authority with
   `resolve_for_content()` / `compose()` and a built-in semantic-overlay lens.

---

## Current State

| Registry | Struct | API completeness | Wired | Tested | Diag |
|---|---|---|---|---|---|
| `ProtocolRegistry` | ✅ | ✅ cancellable MIME probe + provider-wired scheme registry | ✅ | ✅ | ✅ |
| `ViewerRegistry` | ✅ | ✅ capability description + fallback floor | ✅ | ✅ | ✅ |
| `ViewerSurfaceRegistry` | ✅ (layout-domain authority) | ✅ viewer-specific surface profile resolution | ✅ | ✅ | ✅ |
| `LensRegistry` | ✅ | ✅ content-aware resolution + semantic-overlay composition | ✅ | ✅ | ✅ |

### Key gaps

Residual non-blockers that should stay explicit:
- provider-wired scheme registration exists, but richer scheme-source/conflict metadata still lives
  in the protocol-contract layer rather than a dedicated runtime handler descriptor object
- viewer-surface overlay policy is now derived from viewer capability/render mode and the
  compositor path; it is not yet stored as a first-class standalone `ViewportPolicy` struct
- lens resolution is now content-aware and semantic-tag-aware, but its concrete application point
  remains graph-view lens refresh rather than per-viewer content post-processing

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

- [x] A URI entered in the omnibar resolves scheme → MIME → viewer → viewer-surface profile →
  lens profile without ad hoc per-tile viewer selection in the runtime path.
- [x] The full chain is exercised in a scenario test in `shell/desktop/tests/scenarios/`.
- [x] `http(s)://` MIME probe completes and triggers viewer re-selection via reducer intent and
  `SignalRoutingLayer` publication.
- [x] Provider-wired scheme registration participates in protocol resolution.
- [x] Unsupported MIME always resolves to a fallback viewer (never panics or no-ops).
- [x] `ViewerSurfaceRegistry` owns viewer-specific surface profile resolution and workbench/tile
  code now delegates through runtime registry helpers.
- [x] `LensRegistry` is a real struct; `LENS_ID_SEMANTIC_OVERLAY` produces a semantic-overlay
  profile for tagged semantic content.
- [x] The pipeline diagnostic channels (`registry:protocol:*`, `registry:viewer:*`,
  `registry:lens:*`) remain active on the runtime path.

---

## Related Documents

- [protocol_registry_spec.md](protocol_registry_spec.md)
- [viewer_registry_spec.md](viewer_registry_spec.md)
- [viewer_surface_registry_spec.md](viewer_surface_registry_spec.md)
- [lens_compositor_spec.md](lens_compositor_spec.md)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) — routing decision table
- [../2026-03-08_servoshell_debtclear_plan.md](../2026-03-08_servoshell_debtclear_plan.md) — overlay affordance / TileRenderMode dependency
- [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md) — master index
