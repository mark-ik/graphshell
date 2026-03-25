# Shell Overview Surface Spec

**Date**: 2026-03-25
**Status**: Design / Active
**Scope**: The Shell-level overview surface that summarizes graph truth, Workbench truth, and shell/runtime truth without collapsing them into one abstraction.

**Related**:

- `SHELL.md` — Shell host and orchestration boundary
- `../../technical_architecture/unified_view_model.md` — five-domain model and shell overview role
- `../../technical_architecture/graphlet_model.md` — graphlet semantics used by the overview
- `../navigator/NAVIGATOR.md` — Navigator projection domain
- `../workbench/WORKBENCH.md` — Workbench arrangement authority
- `../canvas/CANVAS.md` — Graph domain at the canvas surface

---

## 1. Purpose

Graphshell needs a surface that answers:

- what graph context am I in,
- what is open and foregrounded,
- what is the application doing right now,
- what needs my attention,
- where should I go next?

This is a Shell concern because it spans multiple domains at once.

The overview must summarize them without pretending they are one thing.

---

## 2. Three Truths The Overview Must Preserve

The Shell overview must present three parallel truths.

| Truth family | Meaning | Authority |
|---|---|---|
| **Graph truth** | what exists and how it is related | Graph |
| **Workbench truth** | what is open, arranged, and foregrounded | Workbench |
| **Shell/runtime truth** | what the application is doing and what needs attention | Shell plus supporting runtime/subsystems |

The overview must show correspondences among those truths, not flatten them.

---

## 3. Default Information Architecture

The default Shell overview should be built from six modules.

### 3.1 Active Context strip

Always-visible summary of:

- active graph/view target,
- active graphlet name or kind,
- current scope token,
- current focus authority.

### 3.2 Graph Context card

Summarizes:

- primary and secondary targets,
- active graphlet members/count,
- current graphlet kind,
- nearby frontier candidates,
- component or path summary when relevant.

### 3.3 Workbench Context card

Summarizes:

- active frame,
- active tile group / tab group,
- focused pane,
- open pane count,
- linked or detached graphlet binding state when relevant.

### 3.4 Viewer / Content card

Summarizes:

- current viewer backend,
- fallback/degraded state,
- content kind,
- per-content attention flags such as blocked permissions or unsupported type.

### 3.5 Runtime / Attention card

Summarizes:

- diagnostics warnings,
- background jobs,
- sync/presence state,
- trust/security state,
- current focus-return anchor if a modal or overlay is active.

### 3.6 Suggested Next Actions card

Provides action-oriented suggestions such as:

- open frontier,
- show path,
- reveal in graph,
- reopen linked graphlet,
- resolve fallback viewer,
- inspect diagnostics.

---

## 4. Default Layout Sketch

One plausible desktop layout:

```text
┌──────────────────────────────────────────────────────────────────────┐
│ Active Context: Graph A · Component graphlet · Frame 2 · Pane 5     │
├──────────────────────────────┬───────────────────────────────────────┤
│ Graph Context                │ Workbench Context                     │
│ - active graphlet            │ - frame / group / pane               │
│ - primary/secondary targets  │ - linked vs detached                 │
│ - frontier / path summary    │ - open pane count                    │
├──────────────────────────────┼───────────────────────────────────────┤
│ Viewer / Content             │ Runtime / Attention                  │
│ - backend + content kind     │ - diagnostics / sync / trust         │
│ - fallback state             │ - background jobs                    │
├──────────────────────────────────────────────────────────────────────┤
│ Suggested Next Actions                                            │
└──────────────────────────────────────────────────────────────────────┘
```

This may be rendered as:

- a transient overlay in graph-first mode,
- a pinned Workbench tool pane,
- a compact mode in a Shell host region.

---

## 5. Domain Collaboration Model

### 5.1 Shell

Shell owns:

- overview surface composition,
- module ordering,
- command routing from the overview,
- attention prioritization,
- app-level visibility policy.

### 5.2 Graph

Graph supplies:

- active targets,
- graphlet facts,
- path/component/loop/facet summaries,
- graph-side diagnostics relevant to current context.

### 5.3 Navigator

Navigator supplies:

- active graphlet identity,
- breadcrumb/context strings,
- scoped search context,
- frontier candidates,
- graphlet transition suggestions.

### 5.4 Workbench

Workbench supplies:

- active frame/group/pane summary,
- linked vs detached arrangement state,
- foregrounding and staging status,
- open-pane correspondence information.

### 5.5 Viewer

Viewer supplies:

- viewer backend,
- content kind,
- fallback/degraded reason,
- content-local state worth surfacing to the host.

---

## 6. UI Role Of Each Domain

The overview should make the role of each domain legible.

| Domain | UI signature in the overview |
|---|---|
| **Shell** | commands, ambient status, attention, orchestration |
| **Graph** | what exists, where focus is, how targets are related |
| **Navigator** | what local world the user is traversing |
| **Workbench** | what is open and staged for work |
| **Viewer** | how the current thing is being realized |

This is the shortest honest explanation of how the domains work together in UI.

---

## 7. Interaction Rules

The overview is not just passive status.

Minimum interactions:

- clicking a graph target summary reveals it in graph,
- clicking a graphlet summary opens or focuses the matching Navigator graphlet view,
- clicking a frame/pane summary foregrounds it in Workbench,
- clicking a viewer backend or fallback state opens viewer diagnostics or `Render With`,
- clicking a runtime warning routes to the owning diagnostics or control surface,
- action suggestions dispatch to the owning domain rather than mutating state directly in Shell.

---

## 8. Modes

The overview should support at least three modes.

### 8.1 Compact

One-line or one-row summary for persistent shell chrome.

### 8.2 Standard

Two-column or card-based summary for regular use.

### 8.3 Diagnostic

Expanded mode with more subsystem/runtime visibility and fewer productivity shortcuts.

---

## 9. Graphlet Integration

The overview should treat the active graphlet as the primary unit of current graph context.

That means it should show:

- graphlet kind,
- anchor(s),
- member count,
- current layout/presentation mode if relevant,
- linked Workbench binding state if one exists,
- next-best graphlet transitions when helpful.

This is the most concrete way the overview makes Navigator and Graph collaborate in the UI.

---

## 10. Example Flows

### 10.1 Graph-first investigation

1. User is on the canvas.
2. Overview shows `Ego graphlet · 14 nodes · 3 frontier candidates`.
3. User clicks a frontier candidate.
4. Shell routes to Navigator for graphlet transition.
5. Graph and Navigator update; Workbench remains absent.

### 10.2 Workbench-heavy comparison

1. User has 6 panes open across 2 frames.
2. Overview shows `Frame B · detached arrangement · viewer:fallback in 1 pane`.
3. User clicks the fallback warning.
4. Shell routes to Workbench + Viewer diagnostics.

### 10.3 Trust / runtime interruption

1. Focused node enters degraded trust state.
2. Overview shows warning in Runtime / Attention card.
3. User clicks it.
4. Shell routes to the owning diagnostics or trust surface while preserving graph/workbench context.

---

## 11. Acceptance Criteria

The overview surface is coherent when:

1. it clearly separates graph, Workbench, and shell/runtime truth,
2. it names the active graphlet and current work context,
3. it exposes domain-specific attention without lying about ownership,
4. its commands always route to the owning domain,
5. it works both with and without the Workbench being active,
6. it makes the five domains more legible to the user rather than less.
