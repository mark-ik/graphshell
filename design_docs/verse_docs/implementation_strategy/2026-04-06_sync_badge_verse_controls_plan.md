# Sync Badge → Verse Controls Expansion Plan

**Date**: 2026-04-06
**Status**: Design — Pre-Implementation (Verse-blocked)
**Purpose**: Specify the interaction contract for the ambient sync status badge
in the graph-scoped Navigator host and its expansion into live Verse peer
presence, trust controls, and explicit sync actions.

**Extracted from**: `../subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md`
(Slice 4 open item, §12 acceptance criterion "Sync badge expands to Verse
controls on click") — that plan is now archived; this document is the
forward-tracking authority for this feature.

**Related**:
- `../technical_architecture/` — Verse subsystem architecture
- `../../verse_docs/technical_architecture/VERSE_AS_NETWORK.md`
- `../../verse_docs/technical_architecture/VERSE_AS_PEER.md`
- `../subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md` (archived)

**Blocking condition**: Implementation requires a stable Verse peer presence and
sync API. This plan should not drive shell-side code until that API is defined.
The badge ambient state (dot rendered in graph-scoped host) is already landed;
only the expansion interaction is open.

---

## 1. Current State

The graph-scoped Navigator host renders an ambient sync status indicator derived
from `GraphBrowserApp` sync state. It is a read-only dot/chip — no interactive
expansion is implemented. This is the correct baseline: the ambient badge is
present and correctly scoped to the graph host.

---

## 2. Target Interaction Contract

When the user clicks the sync badge:

1. An expansion panel opens anchored to the badge, showing:
   - **Peer presence list**: connected Verse peers with trust tier and last-seen
     timestamp
   - **Sync action**: `SyncNow` button — triggers an explicit sync cycle
   - **Trust controls**: per-peer trust tier toggle (trusted / untrusted /
     blocked), routing through the Verse trust authority
   - **Connection status**: current Verse node connectivity state (connected /
     degraded / offline)

2. Clicking outside the expansion panel dismisses it without side effects.

3. The badge dot reflects aggregate sync state at all times regardless of
   whether the panel is open:

   | State | Badge appearance |
   |-------|-----------------|
   | Synced / no peers | Neutral dot |
   | Peers connected, sync active | Active/animated dot |
   | Sync degraded or offline | Warning dot |
   | Verse disabled / not configured | Badge hidden or grayed |

---

## 3. Authority Boundaries

- **Verse subsystem** owns: peer presence truth, trust tier state, sync cycle
  dispatch, connectivity state.
- **Shell / Navigator host** owns: badge rendering, expansion panel layout,
  dispatch of `GraphIntent` or `WorkbenchIntent` variants that the Verse apply
  layer handles.
- The badge must not duplicate or replace any Verse-owned settings surface. It
  is an ambient status affordance with quick-action reach, not a full Verse
  configuration UI.

---

## 4. Required Verse API Surface

Before shell-side implementation can proceed, the Verse subsystem must expose:

1. A queryable peer presence snapshot (peer id, trust tier, last-seen, sync
   state) readable each frame from `GraphBrowserApp` or a Verse runtime handle.
2. A `SyncNow` dispatch path — either a `GraphIntent` variant or a direct
   Verse runtime call.
3. A per-peer trust mutation path — either `GraphIntent::SetVersePeerTrust` or
   equivalent, handled in the Verse apply layer.
4. A connectivity state field readable from `GraphBrowserApp`.

These are the four inputs the shell badge expansion needs. Until they exist,
the badge remains read-only ambient state.

---

## 5. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Sync badge visible in graph-scoped host | Already landed — ambient dot present |
| Badge reflects aggregate sync state | Verify: offline → warning dot; active → animated dot |
| Click opens expansion panel with peer list | Test: click badge → panel appears with at least `SyncNow` and connectivity status |
| `SyncNow` dispatches and triggers a sync cycle | Test: click `SyncNow` → Verse sync cycle starts; badge state updates |
| Trust tier toggle routes through Verse apply layer | Test: toggle trust for a peer → Verse trust state updated; badge reflects change |
| Panel dismisses on outside click | Test: click outside → panel closes; no side effects |
| Badge hidden when Verse is disabled/unconfigured | Test: Verse not configured → badge absent or grayed |
| Expansion does not duplicate Verse settings surface | Review: panel contains only presence/status/quick-actions, not full config |
