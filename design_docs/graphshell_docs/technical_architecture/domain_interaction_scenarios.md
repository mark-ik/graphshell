# Domain Interaction Scenarios

**Date**: 2026-03-25
**Status**: Architecture reference
**Scope**: End-to-end scenarios showing how Shell, Graph, Navigator, Workbench, and Viewer cooperate without collapsing ownership.

**Related**:

- `unified_view_model.md` — canonical five-domain model
- `graphlet_model.md` — graphlet semantics used across flows
- `../implementation_strategy/domain_interaction_acceptance_matrix.md` — compact acceptance matrix for PR review and scenario evidence
- `../implementation_strategy/shell/SHELL.md` — Shell host/orchestration boundary
- `../implementation_strategy/shell/shell_overview_surface_spec.md` — Shell overview UI
- `../implementation_strategy/graph/GRAPH.md` — Graph domain spec; the canvas is one of its primary surfaces
- `../implementation_strategy/navigator/NAVIGATOR.md` — Navigator projection/navigation domain
- `../implementation_strategy/workbench/WORKBENCH.md` — Workbench arrangement/activation domain

---

## 1. Purpose

The five-domain model is easiest to understand when expressed as concrete user flows.

This doc provides those flows.

It does not replace the domain specs. It shows how they collaborate while preserving ownership.

---

## 2. Scenario Format

Each scenario answers four questions:

1. what the user is trying to do,
2. which surface they act on,
3. which domain owns each state change,
4. what the user should see as a result.

---

## 3. `DI01` Scenario A: Graph-First Local Exploration

### `DI01` User goal

Inspect the neighborhood around one node and expand into likely next targets.

### `DI01` Starting surface

Graph canvas.

### `DI01` Flow

1. The user selects a node on the graph canvas.
2. Graph owns the selection write and updates graph truth for the active target.
3. Navigator derives an ego graphlet from that anchor.
4. Graph updates visible highlighting to show the active anchor and nearby related nodes.
5. Navigator exposes frontier candidates and local context in its host.
6. Shell overview summarizes the active graphlet as the current work context.
7. The user chooses one frontier candidate.
8. Navigator owns the graphlet transition and resolves the next local world.
9. Graph updates the visible graph presentation for that new graphlet.

### `DI01` Ownership breakdown

| Concern | Owner |
|---|---|
| selected node truth | Graph |
| active graphlet derivation | Navigator |
| visible graph highlight/layout | Graph |
| cross-domain summary | Shell |

### `DI01` UI result

- Graph canvas shows the active local world.
- Navigator shows the same local world in structured form.
- Shell overview shows `Ego graphlet`, anchors, and frontier count.
- Workbench may be absent with no contradiction.

---

## 4. `DI02` Scenario B: Corridor Transition From Navigator

### `DI02` User goal

Understand the relation path between two targets already visible in context.

### `DI02` Starting surface

Navigator host.

### `DI02` Flow

1. The user selects two relevant nodes through Navigator rows or graph selection.
2. Graph owns the selected target set.
3. Navigator resolves a corridor graphlet between those anchors.
4. Graph receives the corridor result and presents path emphasis on the graph surface.
5. Navigator reprojects its rows around the corridor ordering.
6. Shell overview updates to show `Corridor graphlet` and path summary.

### `DI02` Ownership breakdown

| Concern | Owner |
|---|---|
| selected anchors | Graph |
| path/corridor derivation | Navigator using Graph algorithms |
| path rendering on graph | Graph |
| summary of current mode | Shell |

### `DI02` UI result

- Navigator behaves like a path explainer, not a second graph store.
- Graph behaves like a path renderer and inspection surface, not a hierarchy browser.
- Shell exposes the resulting context without redefining the path.

---

## 5. `DI03` Scenario C: Open A Linked Arrangement Around A Graphlet

### `DI03` User goal

Stage a set of related content panes for comparative work while keeping them tied to a graphlet.

### `DI03` Starting surface

Graph canvas, Navigator, or Shell overview suggestion.

### `DI03` Flow

1. The user invokes `open in workbench` for the active graphlet.
2. Shell routes the command to the owning domains rather than creating structure itself.
3. Navigator supplies the active graphlet identity and member set.
4. Workbench creates or focuses a linked arrangement around that graphlet.
5. Viewer realizes the member nodes in their chosen backends inside the resulting panes.
6. Shell overview updates with frame, pane count, and linked/detached status.

### `DI03` Ownership breakdown

| Concern | Owner |
|---|---|
| graphlet identity | Navigator |
| frame/tile/pane structure | Workbench |
| viewer backend realization | Viewer |
| command routing and summary | Shell |

### `DI03` UI result

- Workbench becomes active because the user asked for arrangement.
- The arrangement is visibly linked to a graphlet but does not own graphlet truth.
- Shell overview can show `linked arrangement` without becoming the arrangement authority.

---

## 6. `DI04` Scenario D: Viewer Fallback Inside A Workbench-Heavy Session

### `DI04` User goal

Continue work even when one pane cannot render with the preferred backend.

### `DI04` Starting surface

Workbench.

### `DI04` Flow

1. The user focuses a pane whose preferred viewer backend degrades or fails.
2. Viewer owns backend selection, fallback, and degraded-state reason.
3. Workbench preserves pane placement and focus context.
4. Shell overview surfaces the degraded state in the Viewer / Content card and Runtime / Attention card.
5. The user invokes `Render With` or diagnostic detail from the overview or pane controls.
6. Viewer resolves an alternate realization path.
7. Workbench keeps the same arrangement unless the user explicitly restructures it.

### `DI04` Ownership breakdown

| Concern | Owner |
|---|---|
| backend failure/fallback | Viewer |
| pane placement/focus context | Workbench |
| ambient attention surfacing | Shell |

### `DI04` UI result

- The failure is legible without implying that Workbench or Shell owns rendering policy.
- The user can recover from the overview or the pane-local controls.

---

## 7. `DI05` Scenario E: Shell Overview As Cross-Domain Reorientation

### `DI05` User goal

Recover orientation after context drift or interruption.

### `DI05` Starting surface

Shell overview.

### `DI05` Flow

1. The user opens the Shell overview.
2. Shell composes summaries from Graph, Navigator, Workbench, and Viewer.
3. The overview shows:
   - active graphlet,
   - active frame/pane,
   - current viewer backend,
   - runtime warnings or background work.
4. The user chooses one summary target, such as the active graphlet or focused pane.
5. Shell routes to the owning domain:
   - graphlet summary -> Navigator / Graph,
   - frame or pane summary -> Workbench,
   - viewer fallback -> Viewer diagnostics.

### `DI05` Ownership breakdown

| Concern | Owner |
|---|---|
| summary composition | Shell |
| graph context facts | Graph + Navigator |
| arrangement facts | Workbench |
| viewer facts | Viewer |

### `DI05` UI result

- The overview acts as a control tower, not as a universal data model.
- It helps the user recover orientation across domains quickly.

---

## 8. `DI06` Scenario F: Runtime / Trust Interruption Without Context Loss

### `DI06` User goal

Respond to a warning without losing graph/workbench context.

### `DI06` Starting surface

Any surface.

### `DI06` Flow

1. A runtime or trust warning becomes relevant to the current content.
2. Shell surfaces the attention signal in overview or ambient host chrome.
3. Viewer and supporting runtime subsystems provide the reason/details.
4. The user opens the warning detail route.
5. Shell preserves the current graphlet and Workbench return context while routing to the appropriate detail surface.
6. After resolution or dismissal, focus returns to the prior graph/workbench context anchor.

### `DI06` Ownership breakdown

| Concern | Owner |
|---|---|
| attention surfacing and return path | Shell |
| content-specific warning reason | Viewer or runtime subsystem |
| underlying graph/arrangement context | unchanged Graph / Navigator / Workbench authorities |

### `DI06` UI result

- Interruption handling is host-owned.
- Domain truth remains stable while the interruption is handled.

---

## 9. Core Pattern

Across all scenarios, the repeated pattern is:

1. Shell hosts and routes.
2. Graph defines truth.
3. Navigator derives local navigable worlds from that truth.
4. Workbench stages work when arrangement is requested.
5. Viewer realizes content.

This is the operational meaning of the five-domain model.

---

## 10. Acceptance Criteria

These scenarios are doing their job when:

1. a reader can identify which domain owns each step,
2. Shell is clearly host/orchestrator rather than universal owner,
3. graphlets appear as shared cross-domain objects without becoming Workbench-owned,
4. Workbench can be present or absent depending on the scenario,
5. Viewer fallback and runtime attention fit into the same model without distortion.
