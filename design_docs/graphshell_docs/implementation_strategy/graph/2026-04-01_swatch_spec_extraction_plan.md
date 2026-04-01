# Swatch Spec Extraction Plan

**Date**: 2026-04-01
**Status**: Active follow-on
**Priority**: Medium

**Related**:

- `GRAPH.md`
- `multi_view_pane_spec.md`
- `graph_node_edge_interaction_spec.md`
- `../navigator/NAVIGATOR.md`
- `2026-03-05_hybrid_graph_view_overview_atlas_plan.md` — archived migration history; source of the extracted swatch material
- `../../TERMINOLOGY.md`

---

## 1. Goal

Extract the reusable **Swatch Spec** concept from the archived hybrid
overview/atlas plan into a standalone graph-domain plan.

This keeps the idea active after the Atlas plan's archival and gives compact
embedded graph projections a home that is not tied to one Navigator surface.

---

## 2. Why This Exists

Graphshell already ships a concrete Navigator overview swatch. What is still
missing is the generalized contract that would let multiple surfaces share:

- compact graph rendering policy
- host-dependent layout selection
- density and aggregation rules
- reducer-authoritative interaction affordances

Without that contract, each new small graph preview risks becoming another
bespoke minimap widget with inconsistent behavior.

---

## 3. Definition

A `SwatchSpec` is a reusable rendering contract for small, optionally
interactive graph projections embedded throughout the UI.

It selects an appropriate compact layout and interaction profile from:

- source scope
- host context
- size class
- flags/options
- ergonomics constraints

`SwatchSpec` is a rendering and interaction contract only. It does not create a
new graph truth layer, a second persistence model, or an alternate layout
authority.

---

## 4. Ownership Boundary

- Graph owns graph truth, graph-view structure, and mutating semantics.
- Navigator may host one swatch presentation, but does not own the swatch
  concept.
- Workbench may host graph-bearing summary cards or previews that reuse the
  same contract.
- Any mutating swatch interaction must route through the same reducer-owned
  intents used by the canonical graph surface for that action.

Critical rule:

- `SwatchSpec` must never become a backdoor second editor with its own
  persistence or geometry truth.

---

## 5. Candidate Hosts

- Navigator Atlas overview cards
- graph-view summary cards
- graphlet summary cards
- selected-node neighborhood previews
- history or session previews that need compact graph context
- inspector/help/documentation surfaces that need alternate graph projections

First practical trigger for extraction:

- move from one concrete swatch host to two or more materially different hosts
  that need the same compact rendering contract

---

## 6. Contract Dimensions

### 6.1 Source Scope

- whole graph
- graph view
- graphlet
- local neighborhood
- semantic subset
- derived or aggregated subset

### 6.2 Layout Profile

- region overview
- local neighborhood
- hierarchy/tree
- strip
- clustered summary
- other host-selected compact layouts

The same underlying data may be rendered with different compact layouts in
different hosts without changing graph truth.

### 6.3 Density Policy

- labels on/off
- edge suppression or simplification
- aggregation hints
- occupancy badges
- archived-item visibility
- emphasis rules for active, selected, or focused targets

### 6.4 Interaction Profile

- passive preview only
- focus/routing target
- reveal/select affordance
- transfer-capable target where host geometry is spacious enough

Mutating affordances may be disabled entirely for preview-only hosts.

### 6.5 Host Options

- size class
- chrome level
- counts and badges
- animation policy
- host feature flags
- accessibility constraints

---

## 7. Non-Goals

- inventing a new persisted graph-region model
- replacing Overview Plane as the canonical graph-view editor
- forcing all compact graph surfaces into one visual layout
- requiring toolbar-scale hosts to support precision gestures that their
  geometry cannot support reliably

---

## 8. Current State

What exists today:

- a concrete Navigator overview swatch implementation
- width gating for sidebar swatch visibility
- list-first Atlas behavior with optional swatch projection
- reducer-parity routing for supported Atlas actions

What does not exist yet:

- a reusable `SwatchSpec` type or equivalent shared contract in runtime code
- a shared layout-profile vocabulary across multiple swatch hosts
- a common density-policy carrier for non-Navigator compact graph surfaces

---

## 9. Extraction Roadmap

### S0 - Contract Lock (docs)

Goals:

- define what `SwatchSpec` owns and does not own
- define reusable contract dimensions
- define extraction triggers so this does not become premature abstraction

Done gates:

- [ ] active graph docs link to this plan as the home for generalized swatch work
- [ ] ownership and non-goal boundaries are explicit

### S1 - Runtime Shape Sketch

Goals:

- identify the minimum runtime carrier needed for reuse
- avoid overfitting the contract to the current Navigator host

Possible shape:

- `SwatchSource`
- `SwatchLayoutProfile`
- `SwatchDensityPolicy`
- `SwatchInteractionProfile`
- `SwatchHostOptions`

Done gates:

- [ ] one runtime sketch exists in docs or code comments without forcing immediate broad refactor
- [ ] current Navigator swatch can be described in that vocabulary without distortion

### S2 - Second Host Adoption

Goals:

- prove the contract against a non-Navigator host
- validate that the abstraction removes duplication instead of renaming it

Candidate second hosts:

- selected-node neighborhood preview
- graphlet summary card
- history/session compact preview

Done gates:

- [ ] at least one second host reuses the contract
- [ ] shared density/layout policy is observably reused rather than copied

### S3 - Shared Rendering Policy

Goals:

- unify compact graph rendering knobs that should stay consistent across hosts
- keep host-specific chrome outside the shared contract

Done gates:

- [ ] shared policy covers labels, aggregation, archived visibility, and focus emphasis
- [ ] host-specific badges/chrome remain host-owned

---

## 10. Extraction Guardrails

Extract only when at least one of these becomes true:

1. A second non-Navigator compact graph surface needs the same rendering rules.
2. The current Navigator swatch accumulates host-conditional branching that is
   clearly describing a latent shared contract.
3. Design review needs a stable vocabulary for comparing multiple compact graph
   projections.

Do not extract merely because "swatch" sounds architectural. The abstraction
must pay for itself in reuse or consistency.

---

## 11. Open Questions

1. Which compact layout profiles deserve first-class status in the initial
   runtime contract?
2. Should density policy be entirely declarative, or may hosts still inject
   local rendering overrides?
3. When a swatch can route selection or focus, what is the minimum shared
   accessibility contract across hosts?
4. Should transfer-capable swatches share one gesture model, or should gesture
   semantics remain host-specific with only reducer parity shared?

---

## 12. Migration Note

This plan extracts and preserves the generalized swatch work that was left
deferred in the archived hybrid overview/atlas plan.

Interpretation rule:

- Atlas shipping is complete enough to archive.
- Generalized `SwatchSpec` extraction is still active, but only as a follow-on
  once multiple hosts need the same contract.
