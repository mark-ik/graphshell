# Lens Decomposition and View Policy Plan

**Date**: 2026-03-27
**Status**: Implemented with follow-up opportunities; revised 2026-03-28 after `SetViewLens` removal and `LensConfig` replacement
**Purpose**: Refactor the Lens model from a monolithic per-view bundle into a compositional preset layer over explicit graph-view policy surfaces, then document the remaining follow-up opportunities after the persistence transition.

**Related**:

- `../system/register/lens_compositor_spec.md`
- `layout_behaviors_and_physics_spec.md`
- `multi_view_pane_spec.md`
- `2026-03-14_graph_relation_families.md`
- `../../research/2026-02-24_interaction_and_semantic_design_schemes.md`
- `../../research/2026-03-27_ambient_graph_visual_effects.md`

---

## 1. Why this plan exists

The original codebase stored a broad `LensConfig` directly on `GraphViewState`
and replaced it wholesale when a view lens was applied or refreshed from the
registry.

That shape was useful to get runtime lens resolution, fallback diagnostics, and
basic per-view behavior working. But it now creates three problems:

1. **A Lens is overloaded**: it is simultaneously acting as:
   - a user-facing preset identity,
   - a registry product,
   - a view-state storage object,
   - an override carrier,
   - and a behavior contract.
2. **Constituent settings are not first-class**: layout, physics, filters,
   overlays, and future family/emphasis policies cannot be independently owned,
   inspected, or overridden without mutating the whole lens bundle.
3. **Composition is weaker than the design intent**: the current registry can
   resolve and shallow-compose lenses, but it does not provide the richer
   policy layering described in the research and architecture docs.

This plan resolved that mismatch by defining a clear split:

- **Lens** = named preset/composition surface
- **View policies** = first-class graph-view-owned settings that a lens may
  populate, override, or partially leave alone

Current as-built state:

- `GraphViewState` now carries explicit `lens_state`, `layout_policy`,
  `physics_policy`, `filter_policy`, `overlay_policy`,
  `presentation_policy`, and `relation_policy`.
- `SetViewLens` has been removed.
- Lens/layout/physics/filter actions now route through narrower graph-view
  intents and policy-aware mutators.
- Registry lens resolution now returns a narrower resolved preset shape instead
  of a monolithic runtime lens bundle.
- `LensConfig` no longer exists as a stored runtime field; legacy snapshots load
  through a compatibility deserializer that hydrates the policy surfaces.

---

## 2. Design position

### 2.1 Keep Lens as a product concept

This plan does **not** remove lenses.

The UX research is correct that users should be able to adopt a named semantic
mode such as "research", "containment", or "overview" without manually tuning
every sub-setting. That remains valuable.

What changes is the authority model:

- a Lens should no longer be the sole storage authority for view behavior
- a Lens should become a **preset source** that resolves into explicit,
  inspectable view policies

### 2.2 Promote constituent settings to first-class view policy surfaces

The graph-view contract in `multi_view_pane_spec.md` already treats a
`GraphViewId` as the owner of scoped camera/lens/layout state. This plan
extends that idea and makes the constituent settings explicit rather than
implicitly packed into `LensConfig`.

### 2.3 Preserve graph-view scope

Nothing in this plan changes the existing scope boundary:

- Graph owns graph truth.
- `GraphViewId` owns scoped view policy.
- Lens application remains graph-view scoped.
- Workbench still hosts graph views without owning their semantics.

---

## 3. Current-state diagnosis

Today `LensConfig` mixes together:

- identity: `name`, `lens_id`
- physics: `physics`
- layout: `layout`, `layout_algorithm_id`
- presentation: `theme`, `overlay_descriptor`
- filtering: `filter_expr`, legacy `filters`

This creates a structural mismatch with the actual runtime:

- physics behavior is already partially separate in registry/settings surfaces
- layout algorithm selection already has an independent control path
- edge projection is already a separate graph-view policy
- filters already behave like a graph-view-local policy surface
- theme is mostly resolved globally rather than from the active lens
- lens refresh replaces the bundle instead of reconciling constituent policies

The result is that `LensConfig` is serving as a **storage shortcut**, not a
clean semantic boundary.

---

## 4. Proposed model

### 4.1 Introduce explicit graph-view policy structs

This has been implemented. `GraphViewState` now stores a stable view-policy set
instead of a monolithic runtime lens bundle.

Suggested model:

```rust
pub struct GraphViewState {
    pub id: GraphViewId,
    pub name: String,
    pub camera: Camera,
    pub lens_state: ViewLensState,
    pub layout_policy: ViewLayoutPolicy,
    pub physics_policy: ViewPhysicsPolicy,
    pub filter_policy: ViewFilterPolicy,
    pub overlay_policy: ViewOverlayPolicy,
    pub presentation_policy: ViewPresentationPolicy,
    pub relation_policy: ViewRelationPolicy,
    // existing per-view/runtime fields...
}
```

Suggested responsibilities:

```rust
pub struct ViewLensState {
    pub base_lens_id: Option<LensId>,
    pub applied_components: Vec<LensComponentId>,
    pub progressive_source_lens_id: Option<LensId>,
    pub display_name: String,
}

pub struct ViewLayoutPolicy {
    pub mode: LayoutMode,
    pub algorithm_id: String,
}

pub struct ViewPhysicsPolicy {
    pub profile_id: Option<PhysicsProfileId>,
    pub family_physics: Option<FamilyPhysicsPolicy>,
    pub inline_profile: Option<PhysicsProfile>,
}

pub struct ViewFilterPolicy {
    pub facet_expr: Option<FacetExpr>,
    pub named_filters: Vec<FilterToken>,
}

pub struct ViewOverlayPolicy {
    pub overlay_descriptor: Option<LensOverlayDescriptor>,
    pub suppressed_effects: Vec<EffectId>,
}

pub struct ViewPresentationPolicy {
    pub theme_id: Option<ThemeId>,
    pub inline_theme: Option<ThemeData>,
}

pub struct ViewRelationPolicy {
    pub edge_projection_override: Option<EdgeProjectionState>,
    pub family_visibility: FamilyVisibilityPolicy,
}
```

Notes:

- The implemented shape is narrower than the speculative sketch above; it uses
  concrete policy structs already present in code rather than the full
  `Sourced<T>`/`inline_*` design.
- `theme_id` has not been promoted; the current implementation keeps
  presentation policy light and does not over-commit to per-view themed chrome.

### 4.2 Redefine Lens as a preset profile

A lens should become a sparse, compositional preset:

```rust
pub struct LensProfile {
    pub id: LensId,
    pub display_name: String,
    pub layout: Option<ViewLayoutPreset>,
    pub physics: Option<ViewPhysicsPreset>,
    pub filter: Option<ViewFilterPreset>,
    pub overlay: Option<ViewOverlayPreset>,
    pub presentation: Option<ViewPresentationPreset>,
    pub relation: Option<ViewRelationPreset>,
    pub progressive_breakpoints: Option<Vec<ProgressiveLensBreakpoint>>,
}
```

This remains the main future-facing registry refinement:

- a Lens is no longer the exact runtime storage shape
- a Lens becomes an input to policy resolution

### 4.3 Add explicit override provenance

To avoid accidental clobbering, each view policy should be able to distinguish:

- inherited-from-lens value
- explicit per-view override
- workspace default
- registry default/fallback

Suggested pattern:

```rust
pub enum PolicyValueSource {
    RegistryDefault,
    WorkspaceDefault,
    LensPreset(LensId),
    ViewOverride,
}

pub struct Sourced<T> {
    pub value: T,
    pub source: PolicyValueSource,
}
```

This is now lightly implemented for the highest-conflict surfaces. Layout,
physics, filter, overlay, and presentation policy state can now record source
metadata such as `RegistryDefault`, `LensPreset(..)`, `ViewOverride`, and
`LegacySnapshot`.

The current implementation intentionally uses explicit `source` fields on the
policy structs rather than a full generic `Sourced<T>` wrapper. That keeps the
runtime migration small while still enabling future diagnostics and reset UX.

---

## 5. Policy precedence model

This plan needs an explicit precedence rule so settings surfaces, diagnostics,
and runtime reducers all agree.

### 5.1 Canonical precedence

For each constituent policy surface:

1. explicit graph-view override
2. active lens preset contribution
3. workspace/user default for that policy surface
4. registry fallback/default

Important consequence:

- `SetViewLensId` and per-view policy intents replace monolithic lens
  application, so unrelated explicit per-view policy overrides remain intact
  unless the user performs an explicit "reset this policy to lens" action.

### 5.2 Lens application modes

Lens application should support two modes:

```rust
pub enum LensApplyMode {
    ReplaceDerivedPolicies,
    MergePreservingViewOverrides,
}
```

Default behavior should be `MergePreservingViewOverrides`.

Use `ReplaceDerivedPolicies` only for explicit reset flows such as:

- "Reapply lens defaults"
- "Reset this view to lens"

### 5.3 Progressive switching semantics

Progressive lens switching should operate on the same policy model:

- switch the active lens preset contribution
- preserve explicit per-view overrides
- re-evaluate any policy surfaces that are lens-derived

This aligns with the existing progressive-lens design but avoids surprise
replacement of local view choices.

---

## 6. Decomposition map from current `LensConfig`

This section defines the migration target for each current field.

| Current field | Target surface | Notes |
|---|---|---|
| `name` | `ViewLensState.display_name` | Identity/display only |
| `lens_id` | `ViewLensState.base_lens_id` | Preset identity, not storage authority |
| `physics` | `ViewPhysicsPolicy.inline_profile` | Transitional only; prefer `profile_id` long term |
| `layout` | `ViewLayoutPolicy.mode` | First-class view policy |
| `layout_algorithm_id` | `ViewLayoutPolicy.algorithm_id` | First-class view policy |
| `theme` | `ViewPresentationPolicy.inline_theme` or remove | Keep only if per-view theme is intentional |
| `filter_expr` | `ViewFilterPolicy.facet_expr` | First-class view policy |
| `filters_legacy` | `ViewFilterPolicy.named_filters` or compatibility layer | Migration-only |
| `overlay_descriptor` | `ViewOverlayPolicy.overlay_descriptor` | First-class visual policy |

Additional fields that should stop pretending to be "outside" lens semantics:

- family emphasis / `FamilyPhysicsPolicy`
- per-lens ambient effect suppression
- family visibility presets for containment/traversal/arrangement overlays

Those belong in explicit policy surfaces and may be populated by a lens preset.

---

## 7. Runtime behavior changes

### 7.1 Lens Identity Application

Current behavior has been replaced by narrower paths:

- `SetViewLensId` selects the active lens identity
- layout and physics have dedicated per-view intents
- filter state has dedicated per-view intents

Implemented behavior:

1. resolve `LensProfile`
2. update `view.lens_state`
3. apply preset contributions into policy surfaces using precedence rules
4. preserve explicit per-view overrides
5. emit diagnostics indicating which policy surfaces changed

### 7.2 Registry-backed lens refresh

Implemented refresh behavior is a targeted recomposition pass:

- recompute only policy surfaces sourced from the affected lens
- leave `ViewOverride` values intact
- preserve unknown future policy surfaces

### 7.3 Settings and chrome surfaces

Graph-scoped controls no longer edit a cloned `LensConfig` blob.

Instead:

- the Lens chip chooses the active preset
- the Physics chip edits `ViewPhysicsPolicy`
- the Layout chip edits `ViewLayoutPolicy`
- filter chips edit `ViewFilterPolicy`
- future family/layer chips edit `ViewRelationPolicy`

This makes the UI match the actual authority model.

### 7.4 Diagnostics

Diagnostics should report both:

- active lens identity
- resolved constituent policy state with provenance

That gives real observability into "what the lens contributed" vs "what the view
overrode".

---

## 8. Lens utility after decomposition

After this refactor, lenses become more useful, not less.

### 8.1 What a Lens remains good for

- fast semantic mode switching
- shareable presets
- workflow defaults
- progressive zoom-stage transitions
- mod-authored bundles
- named visual/semantic grammar for a view

### 8.2 What should no longer require a Lens

- selecting a different layout algorithm
- changing only the physics profile
- applying or clearing a facet filter
- toggling relation-family emphasis
- suppressing a visual ambient effect that clashes with the current lens

These should become direct policy edits.

---

## 9. Migration plan

### Phase 1: Introduce policy structs without changing product behavior

Completed.

1. Added policy structs to `GraphViewState`.
2. Updated diagnostics/tests to observe the new policy structs.

### Phase 2: Change lens application to policy recomposition

Completed.

1. Replaced wholesale lens replacement with policy-aware application logic.
2. Removed `SetViewLens`.
3. Preserved explicit per-view overrides by default.
4. Changed registry refresh to operate through resolved preset recomposition.

### Phase 3: Move UI/control surfaces to constituent policies

Mostly completed.

1. Lens picker edits `ViewLensState` via `SetViewLensId`.
2. Physics/layout/filter controls edit their dedicated policy surfaces.
3. Remaining follow-up is UX polish for explicit "reset this policy to lens"
   affordances.

### Phase 4: Promote registry contracts

Partially completed.

1. Registry lens resolution now returns a narrower resolved preset shape.
2. The registry still conceptually produces full lens presets; `LensProfile` as
   a first-class sparse registry product is still future work.
3. Theme/physics IDs remain a mix of dedicated IDs and inline values depending
   on surface.

### Phase 5: Remove compatibility shortcuts

Completed for runtime state; compatibility remains only at deserialization edges.

1. Removed direct runtime storage dependence on monolithic `LensConfig`.
2. Retained deserialization upgrade logic for legacy snapshots.
3. `filters_legacy` still exists as a compatibility concern and can be retired
   after the migration window closes.

### Remaining work

1. Decide whether lenses become a first-class sparse `LensProfile` registry
   contract or remain represented by the current resolved preset shape.
2. Surface the new provenance metadata in diagnostics and graph-view chrome.
3. Add explicit reset affordances that consume that provenance cleanly.
4. Retire `filters_legacy` after the migration window closes.

---

## 10. Risks and tradeoffs

### 10.1 More types, less ambiguity

This plan adds more structs and provenance handling. That is real complexity.
But it replaces hidden complexity that already exists in reducer/UI/runtime
behavior.

### 10.2 Theme may not deserve per-view policy

If product direction keeps theme global, do not over-engineer per-view theme.
In that case:

- remove theme from lens runtime authority
- keep only a decorative overlay/presentation role for lenses
- leave theme as workflow/global chrome state

### 10.3 Sparse presets need good tooling

Once lenses become sparse preset bundles, authoring and diagnostics need to make
it obvious which policy surfaces a lens actually controls.

That means:

- `describe_lens(id)` should list affected policy surfaces
- diagnostics should show resolved contributions and override sources

---

## 11. Acceptance criteria

Current acceptance status:

- Done: applying a lens no longer overwrites unrelated explicit per-view
  settings by default.
- Done: registry-backed lens refresh preserves explicit graph-view overrides.
- Done: layout, physics, filter, and overlay policies are individually
  inspectable in `GraphViewState`.
- Done: graph-scoped chrome controls edit constituent policies directly rather
  than mutating a cloned monolithic lens bundle.
- Pending/future: progressive lens switching semantics beyond the current lens
  identity model.
- Done: legacy snapshots containing `LensConfig` still load through a
  deterministic upgrade path.

---

## 12. Recommendation

This decomposition should be treated as the baseline architecture.

The current code now behaves like a decomposed policy-first system with a small
legacy deserialization shim. Making the constituent policy surfaces explicit
has aligned:

- the runtime with the docs,
- the UI with the authority model,
- and the lens concept with its intended product role.

The key principle is simple:

> A Lens should be a named preset over graph-view policy surfaces, not the sole
> storage container for those surfaces.
