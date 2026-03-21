# Workbench Layout Policy Spec

**Date**: 2026-03-21
**Status**: Active / planning
**Scope**: Semantic layout policy — per-surface layout constraint model, user-facing configuration mode, first-use preference prompts, and the policy evaluator that translates layout preferences into workbench intents.

**Related**:
- `workbench_frame_tile_interaction_spec.md` — canonical workbench/frame/tile contract
- `pane_presentation_and_locking_spec.md` — `PaneLock` and `PanePresentationMode`
- `workbench_profile_and_workflow_composition_spec.md` — `WorkbenchProfile` shape
- `frame_persistence_format_spec.md` — `PersistedFrame` layout serialization
- `pane_chrome_and_promotion_spec.md` — chrome mode taxonomy
- `2026-03-20_arrangement_graph_projection_plan.md` — arrangement graph as layout authority

---

## 1. Motivation

The workbench today uses a tiling model where pane positions are determined by the arrangement graph. This model is correct for content tiles, but it does not express persistent, role-aware layout constraints a user might want across sessions:

- "I want the Navigator permanently anchored to the bottom of the screen."
- "I want a fixed left sidebar that persists as I navigate the graph."
- "Unlock the toolbar, move it to a different edge, then lock it."
- "Always keep this diagnostics pane pinned to the right, no matter what nodes are open."

These are *semantic* layout preferences: they attach to surface roles (Navigator, Toolbar, DiagnosticsPane) rather than to specific tiles or node keys. They need to survive frame navigation, session reload, and arrangement graph mutations.

This spec defines:

1. `WorkbenchLayoutConstraint` — a per-surface-role constraint applied on top of the arrangement graph
2. `UxConfigMode` — a per-surface toggle exposing layout affordances to the user
3. `SurfaceFirstUsePolicy` — contextual preference prompt on first encounter of a surface
4. `WorkbenchLayoutPolicyEvaluator` — pure function `(UxTreeSnapshot, WorkbenchProfile) → Vec<WorkbenchIntent>`

---

## 2. Definitions

**Surface role**: The semantic identity of a workbench surface — e.g. `Navigator`, `Toolbar`, `DiagnosticsPane`, `NodeTile`, `FacetRail`. Distinct from tile identity (which is a `NodeKey`) and pane position (which is a UI coordinate). Surface roles map to `UxDomainIdentity` variants in the UxTree.

**Layout constraint**: A persistent, role-keyed declaration that a surface must occupy a specific region of the workbench frame, regardless of the arrangement graph's content layout.

**Navigation region**: The portion of the workbench frame available for content tile routing — the area that remains after constrained surfaces have claimed their anchor regions.

**Config mode**: A transient, per-surface mode that temporarily reveals drag handles, resize affordances, and edge-anchor targets. Exiting config mode persists any changes.

---

## 3. `WorkbenchLayoutConstraint`

### 3.1 Type Definition

```rust
/// A persistent layout constraint attached to a surface role.
///
/// Constraints are applied *after* the arrangement graph projects the tile
/// tree. A constrained surface is removed from arrangement-driven flow
/// and placed into a fixed anchor region.
pub enum WorkbenchLayoutConstraint {
    /// Surface participates in arrangement-graph-driven layout normally.
    Unconstrained,

    /// Surface is anchored to an edge of the workbench frame and
    /// occupies a fixed fraction of that dimension.
    AnchoredSplit {
        /// Which surface role this constraint applies to.
        surface_role: SurfaceRole,
        /// Which edge of the workbench frame the surface is anchored to.
        anchor_edge: AnchorEdge,
        /// Fraction of the workbench frame dimension the anchored surface claims.
        /// Must be in (0.0, 1.0). The rest becomes the navigation region.
        anchor_size_fraction: f32,
        /// Whether the anchor size can be resized interactively.
        resizable: bool,
    },
}

pub enum AnchorEdge {
    Top,
    Bottom,
    Left,
    Right,
}

/// Semantic surface roles that can carry layout constraints.
/// These map to UxDomainIdentity variants and survive tile-key churn.
pub enum SurfaceRole {
    Navigator,
    Toolbar,
    DiagnosticsPane,
    FacetRail,
    Sidebar,
    /// Named custom surface (for future extension).
    Named(Arc<str>),
}
```

### 3.2 Semantics

- A `WorkbenchLayoutConstraint::AnchoredSplit` removes the matched surface from arrangement-graph-driven layout and places it against the named edge.
- The remaining workbench area becomes the **navigation region** — the canvas where content tiles are arranged normally.
- Multiple constraints are allowed, but they must not produce overlapping anchor regions. If two constraints claim the same edge, the policy evaluator emits `CHANNEL_UX_LAYOUT_CONSTRAINT_CONFLICT` and falls back to `Unconstrained` for the conflicting surface.
- Constraints are keyed by `SurfaceRole`, not by tile key or node identity. They survive graph navigation, session reload, and arrangement graph mutations.
- Anchor splits are **semantically persistent**: if the surface's tile is evicted (e.g. the Navigator is temporarily hidden), re-opening the surface restores it to its constrained position.

### 3.3 Constraint Priority

When evaluating which surface occupies which region:

1. `AnchoredSplit` constraints are applied in order: Top → Bottom → Left → Right.
2. After all constraints are applied, the remaining area is the navigation region.
3. `Unconstrained` surfaces participate in the navigation region's arrangement graph layout.

---

## 4. `UxConfigMode`

### 4.1 Purpose

Users need a clear, safe mode for reconfiguring layout without accidentally repositioning surfaces during normal use. `UxConfigMode` is a per-surface transient state that:

- Reveals layout affordances (drag handles, edge-anchor targets, size fraction slider)
- Makes the current constraint's edge and fraction visible via chrome overlays
- Commits any changes on exit

### 4.2 Type Definition

```rust
pub enum UxConfigMode {
    /// Normal operating mode. No layout affordances are visible.
    Locked,
    /// Configuration mode. Layout affordances are visible and interactive.
    /// The surface can be dragged, anchored to a different edge, or resized.
    Configuring {
        /// The surface currently being configured.
        surface_role: SurfaceRole,
    },
}
```

### 4.3 Entry and Exit

**Entry** — `UxConfigMode::Configuring` is entered via:
- A dedicated "Unlock layout" button on the surface's chrome (visible in all `PaneLock` states except `FullyLocked`)
- A command palette action: `WorkbenchUnlockSurfaceLayout { surface_role }`
- A `SurfaceFirstUsePolicy` prompt accepting a "Configure now" response

Only one surface may be in `Configuring` mode at a time. Entering config mode on a new surface exits it for any previous surface (committing its changes).

**Exit** — `UxConfigMode::Locked` is restored via:
- Clicking the "Lock layout" button that replaces "Unlock layout" during config mode
- Pressing Escape
- Clicking outside the surface while it is in `Configuring` mode
- Executing `WorkbenchLockSurfaceLayout { surface_role }` from the command palette
- The surface being navigated away from (deferred commit)

**Commit** — On exit, the current constraint state is written to `WorkbenchProfile.layout_constraints`. The `WorkbenchLayoutPolicyEvaluator` re-runs on the next frame.

### 4.4 Affordances During Configuring Mode

While a surface is in `Configuring` mode, its chrome renders:

- **Edge-anchor targets**: four translucent drop zones at the edges of the workbench frame. Dragging the surface to a zone sets `anchor_edge`.
- **Unconstrain target**: a center drop zone. Dropping onto center sets `Unconstrained`.
- **Size fraction slider**: a resize handle on the constrained edge, showing the current fraction as a percentage label.
- **Constraint label**: a pill overlay on the surface showing the current constraint state (e.g. "Anchored to bottom — 30%").

These affordances are rendered by the compositor and are invisible when `UxConfigMode::Locked`.

---

## 5. `SurfaceFirstUsePolicy`

### 5.1 Purpose

When a user first opens a surface that supports layout constraints, a contextual preference prompt appears in or near the surface's chrome. This is a one-time, per-surface offer to configure layout preferences before or immediately after first use.

### 5.2 Type Definition

```rust
pub struct SurfaceFirstUsePolicy {
    /// The surface role this policy applies to.
    pub surface_role: SurfaceRole,
    /// Whether the first-use prompt has been shown for this surface.
    pub prompt_shown: bool,
    /// The outcome of the prompt, if any.
    pub outcome: Option<FirstUseOutcome>,
}

pub enum FirstUseOutcome {
    /// User chose to configure now (enters UxConfigMode::Configuring).
    ConfigureNow,
    /// User accepted the default layout.
    AcceptDefault,
    /// User dismissed the prompt without choosing.
    Dismissed,
    /// User chose "Remember this preference" after configuring.
    RememberedConstraint(WorkbenchLayoutConstraint),
}
```

### 5.3 Prompt Semantics

The prompt appears inline in the surface's chrome region — not as a modal. It contains:

- A short description of the surface and what it does (max 2 lines)
- Three actions:
  - **"Set up layout"** → emits `WorkbenchUnlockSurfaceLayout` to enter `Configuring` mode
  - **"Use default"** → records `AcceptDefault` and hides the prompt
  - **"Dismiss"** → records `Dismissed` and hides the prompt

If the user configures via "Set up layout" and then exits `Configuring` mode, the prompt transitions to a "Remember this preference?" confirmation:

- **"Yes, remember"** → records `RememberedConstraint` with the current constraint; prompt is closed permanently
- **"Just this session"** → applies the constraint for this session only; prompt recurs next session
- **"Discard"** → reverts to default; prompt is closed permanently

The prompt state is persisted in `WorkbenchProfile.first_use_policies` keyed by `SurfaceRole`.

### 5.4 Triggering Rules

- The prompt fires at most once per surface per profile, unless `outcome` is `Dismissed` (in which case it may re-trigger after a configurable number of sessions, default 5).
- The prompt does not fire if the surface has an existing `WorkbenchLayoutConstraint` that is not `Unconstrained` — the user has already made a choice.
- The prompt does not fire if the surface is in `PaneLock::FullyLocked` — the surface cannot be configured in that state.

---

## 6. `WorkbenchProfile` Extensions

The `WorkbenchProfile` gains two new fields to carry layout policy state:

```rust
pub struct WorkbenchProfile {
    // ... existing fields ...

    /// Per-role layout constraints. Keyed by SurfaceRole.
    /// Absent keys are treated as Unconstrained.
    pub layout_constraints: HashMap<SurfaceRole, WorkbenchLayoutConstraint>,

    /// Per-role first-use policy tracking.
    pub first_use_policies: HashMap<SurfaceRole, SurfaceFirstUsePolicy>,
}
```

These fields are persisted in `PersistedFrame` alongside the existing profile blob. The serialization format uses stable string keys for `SurfaceRole` variants (e.g. `"Navigator"`, `"Toolbar"`, `"Named:my-sidebar"`).

**Migration**: On first load of an older `PersistedFrame` that lacks these fields, `layout_constraints` defaults to empty (all `Unconstrained`) and `first_use_policies` defaults to empty (all unseen).

---

## 7. `WorkbenchLayoutPolicyEvaluator`

### 7.1 Purpose

The evaluator is a pure function that takes the current `UxTreeSnapshot` and `WorkbenchProfile` and produces a sequence of `WorkbenchIntent`s that apply the layout constraints.

It is called once per frame after the UxTree is published, before the compositor pass.

### 7.2 Signature

```rust
pub fn evaluate_layout_policy(
    snapshot: &UxTreeSnapshot,
    profile: &WorkbenchProfile,
) -> Vec<WorkbenchIntent> {
    // Pure: reads snapshot and profile, produces intents.
    // Does not write to any shared state.
}
```

### 7.3 Evaluation Logic

1. **Collect active constraints**: Iterate `profile.layout_constraints`. Skip `Unconstrained` entries.

2. **Match constraints to live surfaces**: For each constraint, search `snapshot.semantic_nodes` for a node whose `UxDomainIdentity` matches the `SurfaceRole`. If no live node matches, skip (the surface is not currently open).

3. **Detect conflicts**: If two constraints claim the same `AnchorEdge`, emit `CHANNEL_UX_LAYOUT_CONSTRAINT_CONFLICT` for both and skip both constraints.

4. **Produce intents**: For each non-conflicting constraint with a live match, emit a `WorkbenchIntent::ApplyLayoutConstraint { surface_role, constraint }`. The workbench reducer applies these after the arrangement graph projection pass.

5. **Detect drift**: Compare the UxPresentationNode bounds of each constrained surface against the expected bounds given the constraint. If drift exceeds 2px, emit `CHANNEL_UX_LAYOUT_CONSTRAINT_DRIFT`.

### 7.4 Intent Variants

Two new `WorkbenchIntent` variants:

```rust
pub enum WorkbenchIntent {
    // ... existing variants ...

    /// Apply a layout constraint to a surface role.
    /// Emitted by WorkbenchLayoutPolicyEvaluator each frame.
    ApplyLayoutConstraint {
        surface_role: SurfaceRole,
        constraint: WorkbenchLayoutConstraint,
    },

    /// Enter or exit UxConfigMode for a surface.
    SetSurfaceConfigMode {
        surface_role: SurfaceRole,
        mode: UxConfigMode,
    },
}
```

The workbench reducer handles both variants. `ApplyLayoutConstraint` is idempotent: re-applying the same constraint has no effect.

---

## 8. Diagnostics Channels

Four new diagnostic channels:

| Channel ID | Severity | Description |
|---|---|---|
| `CHANNEL_UX_LAYOUT_CONSTRAINT_CONFLICT` | `Warn` | Two constraints claim the same anchor edge; both skipped |
| `CHANNEL_UX_LAYOUT_CONSTRAINT_DRIFT` | `Warn` | A constrained surface's rendered bounds deviate from expected |
| `CHANNEL_UX_CONFIG_MODE_ENTERED` | `Info` | A surface entered `UxConfigMode::Configuring` |
| `CHANNEL_UX_FIRST_USE_PROMPT_SHOWN` | `Info` | A `SurfaceFirstUsePolicy` prompt was displayed |

These are registered in `PHASE3_CHANNELS` alongside the existing Wave 1–4 channels.

---

## 9. Integration Points

### 9.1 Arrangement Graph

The arrangement graph (per `2026-03-20_arrangement_graph_projection_plan.md`) is the layout authority for content tiles. Layout constraints do not modify the arrangement graph — they operate at the workbench compositor level, after projection. A constrained surface is effectively removed from the arrangement graph's flow region and placed in its anchor region by the compositor.

This preserves the arrangement graph invariant: it remains the source of truth for content tile relationships. Layout constraints are a workbench-level override layer above it.

### 9.2 PaneLock

Layout constraints interact with `PaneLock` as follows:

| PaneLock state | Config mode allowed | Constraint applied |
|---|---|---|
| `Unlocked` | Yes | Yes |
| `PositionLocked` | No (position already locked by other means) | Yes |
| `FullyLocked` | No | No |

A `FullyLocked` surface ignores any `WorkbenchLayoutConstraint` in the profile. The existing lock wins.

### 9.3 PanePresentationMode

Layout constraints only apply to surfaces in `Tiled` or `Docked` presentation mode. `Floating` and `Fullscreen` surfaces are excluded — they manage their own position outside the workbench flow.

### 9.4 UxTree

The evaluator reads `UxTreeSnapshot` to:
- Confirm a surface is live before applying its constraint
- Read `UxPresentationNode.bounds` for drift detection
- Read `UxDomainIdentity` to match `SurfaceRole` to semantic nodes

The `UxConfigMode` state for each surface is projected into the UxTree as a `UxSemanticNode` state flag, making it available to the `UxProbeSet` engine when that is implemented.

### 9.5 Navigator

The Navigator is the most common candidate for layout constraints. The expected first-class use case is:

1. User opens Navigator
2. First-use prompt appears: "Would you like to pin the Navigator to the edge of the screen?"
3. User clicks "Set up layout"
4. Navigator enters `Configuring` mode
5. User drags Navigator to bottom edge
6. Navigator exits `Configuring` mode; constraint `AnchoredSplit { anchor_edge: Bottom, anchor_size_fraction: 0.28, ... }` is written to profile
7. Navigator is anchored to the bottom of the workbench frame for all future sessions, regardless of which nodes are open in the navigation region

---

## 10. Command Surface Integration

Two new command palette actions:

| Action ID | Description |
|---|---|
| `WorkbenchUnlockSurfaceLayout` | Puts the target surface into `UxConfigMode::Configuring` |
| `WorkbenchLockSurfaceLayout` | Exits `UxConfigMode::Configuring` and commits |

Both actions are visible in the command palette and can be triggered via keyboard. `WorkbenchUnlockSurfaceLayout` requires the surface to be focused or passed as a parameter. Neither action fires if the surface is `PaneLock::FullyLocked`.

A third action for system prompt / conversational preference:

| Action ID | Description |
|---|---|
| `WorkbenchRememberLayoutPreference` | Persists the current layout state of the focused surface to `WorkbenchProfile` |

This enables the "remember preference" flow without entering `Configuring` mode — useful for LLM/agent-driven layout commands.

---

## 11. Acceptance Criteria

The feature is complete when:

1. `WorkbenchLayoutConstraint` is persisted in `WorkbenchProfile` and survives session reload.
2. `AnchoredSplit` constraints produce a visible split in the workbench: the constrained surface occupies its anchor edge, and the navigation region occupies the remainder.
3. `UxConfigMode::Configuring` is entered and exited cleanly via both UI affordances and command palette actions.
4. Edge-anchor targets, the unconstrain target, and the size fraction slider are rendered during `Configuring` mode and invisible otherwise.
5. `SurfaceFirstUsePolicy` prompts appear at most once per surface per profile (or per the configured re-trigger interval).
6. The `WorkbenchLayoutPolicyEvaluator` is a pure function with no side effects; its output can be snapshot-tested.
7. All four diagnostic channels are registered and fire under the documented conditions.
8. Conflict detection skips both conflicting constraints and emits `CHANNEL_UX_LAYOUT_CONSTRAINT_CONFLICT`.
9. `PaneLock::FullyLocked` surfaces are excluded from constraint application.
10. `Floating` and `Fullscreen` surfaces are excluded from constraint application.
11. The Navigator first-use flow (described in §9.5) works end-to-end: prompt → config mode → drag to edge → commit → persisted across restart.

---

## 12. Sequencing

This spec depends on:
- `pane_presentation_and_locking_spec.md` — `PaneLock` and `PanePresentationMode` types (landed)
- `workbench_profile_and_workflow_composition_spec.md` — `WorkbenchProfile` shape (specced)
- `frame_persistence_format_spec.md` — persistence layer (specced)
- `2026-03-20_arrangement_graph_projection_plan.md` — arrangement graph projection pass (planned)

Recommended implementation order:

1. **Phase A**: Add `layout_constraints` and `first_use_policies` fields to `WorkbenchProfile`. Add `SurfaceRole` enum. Write persistence/migration.
2. **Phase B**: Implement `WorkbenchLayoutPolicyEvaluator` as a pure function. Add `ApplyLayoutConstraint` intent and reducer handling. Wire into the render pipeline after UxTree publish.
3. **Phase C**: Add `UxConfigMode` state tracking. Wire the "Unlock layout" chrome button and command palette actions. Implement the compositor affordance overlays for config mode.
4. **Phase D**: Implement `SurfaceFirstUsePolicy` prompt rendering and outcome tracking.
5. **Phase E**: Register the four diagnostic channels. Add drift detection in the evaluator.

Phases A and B can land independently. Phase C depends on B. Phase D depends on C. Phase E can land alongside B.
