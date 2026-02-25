# Progressive Lens Switching & Lens/Physics Binding Policy (2026-02-25)

**Status**: Strategy / Design-Resolved
**Relates to**:
- `2026-02-24_interaction_and_semantic_design_schemes.md` §1 (Progressive Lenses open design question resolved here)
- `2026-02-24_physics_engine_extensibility_plan.md` §User Configuration Surface (Lens-physics binding preference)
- `2026-02-22_registry_layer_plan.md` (`LensCompositor`, `PhysicsProfileRegistry`)
- `2026-02-24_immediate_priorities.md` §2 rank 8 (Progressive Lenses + Lens/Physics binding policy)

## Purpose

This document resolves the two open design questions left in prior plans:

1. **Progressive Lens trigger semantics** — what exactly causes an automatic Lens switch at a
   given zoom level, and how does the user opt in or out?
2. **Lens/physics binding contract** — how does a `LensConfig` reference a `PhysicsProfileId`,
   and how is the binding preference respected at runtime?

This is a policy/interaction document. It intentionally precedes implementation to avoid the
"surprising behavior" failure mode called out in `2026-02-24_interaction_and_semantic_design_schemes.md §5`.
Implementation tickets should reference this document as their authoritative spec.

---

## 1. Lens/Physics Binding Contract

### 1.1 Data Model

`LensConfig` gains one optional field:

```
LensConfig {
    id: LensId,
    name: String,
    physics_profile_id: Option<PhysicsProfileId>,   // NEW — None means "no binding"
    layout_id: Option<LayoutId>,
    theme_id: Option<ThemeId>,
    // …existing fields…
}
```

`PhysicsProfileId` is the existing identifier type in `PhysicsProfileRegistry`.
A `None` value means the Lens has no physics opinion; the current active profile is preserved.

### 1.2 Binding Preference

A per-user preference `lens_physics_binding: LensPhysicsBindingPreference` stored in
`AppPreferences` governs how `LensCompositor` handles a `LensConfig` that carries a
`physics_profile_id`:

```
pub enum LensPhysicsBindingPreference {
    Always,  // auto-switch without confirmation
    Ask,     // show a non-blocking toast/badge; user confirms or dismisses
    Never,   // ignore physics_profile_id entirely; never switch automatically
}
```

**Default**: `Ask`.

This matches the value described in
`2026-02-24_physics_engine_extensibility_plan.md §User Configuration Surface`. The field
is now formally resolved as part of the `LensConfig` contract above.

### 1.3 Runtime Behavior at Lens Apply

When `LensCompositor::apply_lens(lens_id, view_id)` is called:

1. Resolve `LensConfig` via the fallback chain (Workspace → User → Default).
2. If `lens_config.physics_profile_id` is `None` → skip all binding logic.
3. Otherwise, check `AppPreferences::lens_physics_binding`:
   - `Always` → call `PhysicsProfileRegistry::activate(physics_profile_id, view_id)` immediately.
   - `Ask` → emit a `LensPhysicsBindingSuggestion` event to the active view's control surface.
     The control surface renders a non-blocking inline prompt: *"Switch to `<profile name>`
     physics for this Lens? [Apply] [Keep current]"*. No auto-switch occurs until the user
     confirms. If dismissed, store the dismissal as a per-`(LensId, PhysicsProfileId)` skip
     hint in session state (not persisted; reset on restart).
   - `Never` → no-op; active profile is unchanged.

### 1.4 `LensTransitionHook` Registration (Mods)

Lens-physics binding mods (described in
`2026-02-24_physics_engine_extensibility_plan.md §Lens-physics binding mods`) register as
`LensTransitionHook` entries in `LensCompositor`. At hook invocation time, the same
`LensPhysicsBindingPreference` check in §1.3 applies — hooks are subject to the same
`Always/Ask/Never` gate. Hooks must not bypass the preference gate.

---

## 2. Progressive Lens Trigger Semantics

### 2.1 Mechanism: Threshold-Based, Not Continuous Interpolation

Progressive Lens switching is **threshold-based** (discrete transitions at defined zoom
levels), not continuous interpolation between two Lenses. Rationale:

- Continuous interpolation between `LensConfig` values (physics profile, theme, layout)
  would require per-field interpolation contracts that do not yet exist in the registry
  layer. It is premature.
- Threshold-based switching composes cleanly with the `Always/Ask/Never` preference and
  is comprehensible to users.
- Interpolation between physics states can be added later as an `ExtraForce`-level
  transition effect without changing the policy layer.

### 2.2 Progressive Lens Configuration Shape

A `ProgressiveLensConfig` is an ordered list of `(zoom_threshold, lens_id)` breakpoints
stored in `LensConfig` as an optional field:

```
LensConfig {
    // …fields from §1.1…
    progressive_breakpoints: Option<Vec<ProgressiveLensBreakpoint>>,
}

pub struct ProgressiveLensBreakpoint {
    /// Zoom scale at which this Lens activates (zoom_out direction: decreasing value).
    /// Scale is the same unit as the camera's scale factor (1.0 = nominal, <1.0 = zoomed out).
    zoom_scale_threshold: f32,
    lens_id: LensId,
}
```

Breakpoints are sorted descending by `zoom_scale_threshold`; the first breakpoint whose
threshold is ≥ current zoom scale is the active progressive target.

**Example** (matches the research note in §1 of the interaction schemes doc):

```
progressive_breakpoints: Some(vec![
    ProgressiveLensBreakpoint { zoom_scale_threshold: 0.4, lens_id: LensId("overview") },
    // At zoom ≥ 0.4 the default Lens applies (no entry needed; handled by fallback)
])
```

When zoomed out past 0.4 scale, `lens:overview` (using `physics:gas`) activates.
When zooming back in past 0.4 scale, the original Lens reactivates.

### 2.3 Trigger Evaluation

`LensCompositor` evaluates progressive breakpoints on every camera scale change event
(`CameraScaleChanged`). Evaluation is cheap: iterate the sorted breakpoint list and compare
the current scale against thresholds.

**Hysteresis**: To prevent rapid oscillation at threshold boundaries, a hysteresis band of
±10% of the threshold value is applied before a switch is considered triggered. A switch
triggers only when the scale crosses outside the hysteresis band from the prior side.

```
hysteresis_band = zoom_scale_threshold * 0.10
switch_triggers_when: abs(current_scale - zoom_scale_threshold) > hysteresis_band
                      AND side_changed
```

### 2.4 User Confirmation Gate

Progressive breakpoint switches are subject to the same `LensPhysicsBindingPreference`
gate as manual Lens application (§1.3). Additionally, progressive switches are governed by
a separate `progressive_lens_auto_switch: ProgressiveLensAutoSwitch` preference:

```
pub enum ProgressiveLensAutoSwitch {
    Always,  // switch immediately when threshold is crossed
    Ask,     // show non-blocking toast; user confirms or dismisses
    Never,   // disable all progressive Lens switching
}
```

**Default**: `Ask`.

This preference is orthogonal to the physics binding preference. The two preferences chain:

1. Check `progressive_lens_auto_switch` first; if `Never`, stop.
2. If the target Lens carries a `physics_profile_id` and `progressive_lens_auto_switch` is
   `Always` or user confirmed, evaluate `lens_physics_binding` before activating the physics
   profile.

### 2.5 Preference Storage

Both preferences are stored in `AppPreferences`:

```
AppPreferences {
    // …existing fields…
    lens_physics_binding: LensPhysicsBindingPreference,       // default: Ask
    progressive_lens_auto_switch: ProgressiveLensAutoSwitch,  // default: Ask
}
```

Both are surfaced in the settings UI under a **Lens** section. Suggested labels:
- "When applying a Lens, also switch physics preset: **Always / Ask / Never**"
- "When zooming, switch Lens automatically: **Always / Ask / Never**"

---

## 3. Resolved Open Questions

This section lists the specific open questions from prior documents and records their
resolution.

| Open Question (source) | Resolution |
|---|---|
| *"Silent auto-switch is surprising. Resolve trigger semantics (threshold-based vs. continuous interpolation, with or without confirmation) before implementing."* — `2026-02-24_interaction_and_semantic_design_schemes.md §5` | **Threshold-based with confirmation gate.** §2.1 and §2.3 specify breakpoint evaluation. §2.4 specifies the `ProgressiveLensAutoSwitch` preference. |
| *"`Always / Ask / Never`"* preference mentioned but not formally specified in `2026-02-24_physics_engine_extensibility_plan.md §User Configuration Surface` | **Specified.** §1.2 formalizes `LensPhysicsBindingPreference`. §2.4 introduces the parallel `ProgressiveLensAutoSwitch` preference. Both are stored in `AppPreferences`. |
| *"`LensConfig.physics_profile_id: Option<PhysicsProfileId>`"* noted as needed but not added in `2026-02-24_physics_engine_extensibility_plan.md §Cross-Plan Integration Gaps` | **Specified.** §1.1 defines the exact field. |
| *"Resolve trigger semantics (threshold vs. interpolation)"* | **Threshold-based.** Continuous interpolation deferred until per-field interpolation contracts exist at the registry layer. See §2.1 for rationale. |
| *"Hysteresis / oscillation at boundaries"* (implicit — not previously written down) | **Specified.** §2.3 defines a ±10% hysteresis band on each breakpoint threshold. |
| *"Preference chaining — what order do the two `Always/Ask/Never` controls apply?"* (implicit) | **Specified.** §2.4 defines the chain: `progressive_lens_auto_switch` first, then `lens_physics_binding`. |

---

## 4. Implementation Prerequisites and Sequencing

This document does not define implementation phases. It resolves the policy questions needed
before any implementation begins. The sequencing constraints in
`2026-02-24_interaction_and_semantic_design_schemes.md §5` remain in force:

1. **Lens Resolution path** (`LensCompositor.resolve_lens()` active code path, Phase 6.2
   callsite migration complete) — must be done before any progressive Lens switch logic runs.
2. **Distinct physics presets** (Liquid/Gas/Solid perceptually distinct at default zoom) —
   must be done before physics binding is user-visible.

When those prerequisites are met, the implementation order for this spec is:

1. Add `physics_profile_id` field to `LensConfig` (§1.1). Wire `Always` path in
   `LensCompositor::apply_lens`. Add `LensPhysicsBindingPreference` to `AppPreferences`.
2. Implement `Ask` toast/badge surface in the active view control surface.
3. Add `progressive_breakpoints` field to `LensConfig` (§2.2). Wire threshold evaluation
   in `LensCompositor` on `CameraScaleChanged` with hysteresis (§2.3).
4. Add `ProgressiveLensAutoSwitch` to `AppPreferences` (§2.5). Wire preference check and
   chain (§2.4).
5. Surface both preferences in settings UI (§2.5 label text).

---

## 5. Out of Scope

- **Continuous physics interpolation between Lens states** — deferred. Requires per-field
  interpolation contracts at the registry level. Tracked as a future enhancement to the
  `ExtraForce` transition pipeline.
- **Per-Lens region scope preference** — separate setting; see
  `2026-02-24_physics_engine_extensibility_plan.md §Region persistence strategy preference`.
- **3D progressive Lens switching** — follows the same model but depends on `ViewDimension`
  stabilization; see `2026-02-24_physics_engine_extensibility_plan.md §GraphViewState.dimension`.
