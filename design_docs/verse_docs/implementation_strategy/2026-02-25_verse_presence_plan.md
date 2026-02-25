# Verse Presence UX — Implementation Plan

**Date**: 2026-02-25
**Status**: Deferred (blocked on Phase 5 done-gate closure)
**Blocked by**: Phase 5.4 delta sync harness + Phase 5.5 access control harness
**Phase**: Post-Phase-5 (Phase 6+)
**Context**: Specifies the minimum presence layer for collaborative sessions — ghost cursors, remote selection highlights, and follow mode. Presence is a sync UX layer built on top of stable Verse semantics, not a substitute for them.

---

## 1. Overview

Verse Tier 1 (Phase 5) delivers **data sync**: intents flow between trusted peers, workspaces converge, and conflicts are resolved. Phase 5 does not deliver **presence**: there is no representation of remote users in the local graph view during a live session.

This plan introduces **Presence** as the first post-Phase-5 Verse feature. Presence turns Verse from a data sync layer into a shared work environment by giving each peer a visible avatar, a cursor position, and a selection highlight in the graph pane.

**Adoption trigger**: This work must not begin until both of the following done gates are met:
- Phase 5.4 done gate: `verse_delta_sync_basic` harness passes (create/rename sync between two instances).
- Phase 5.5 done gate: `verse_access_control` harness passes (per-workspace, per-peer grant enforcement).

**Rationale**: Presence requires stable identity (Phase 5.2), live peer connections (Phase 5.3), and reliable workspace membership (Phase 5.5) to be meaningful. Implementing presence on top of an unstable sync layer would mask correctness regressions and conflate presence noise with sync bugs.

---

## 2. Minimum Presence Feature Set

The minimum viable presence layer consists of three rendering cues and one interaction mode. Each cue maps directly to a presence event defined in §3.

### 2.1 Remote Cursors (Ghost Cursors)

**Rendering**: A labeled pointer overlay in the graph pane showing where each connected peer's graph-space pointer is hovering.

- **Label**: Peer display name (from `TrustedPeer::display_name`).
- **Color**: A per-peer accent color derived deterministically from the peer's `NodeId` (HSV hue, fixed saturation/value to remain legible on dark and light graph backgrounds).
- **Fade-out**: If no cursor event is received from a peer for 3 seconds, the ghost cursor fades to 20% opacity. At 10 seconds of inactivity it is hidden entirely.
- **Coordinate space**: Graph-space (canvas coordinates), not screen-space, so the cursor follows graph pan and zoom correctly.

### 2.2 Remote Selection Highlights

**Rendering**: A colored secondary border around nodes and edges that a peer currently has selected.

- **Border style**: A 2px dashed outer ring in the peer's accent color, offset 3px from the node's own selection border. This distinguishes remote selection from local selection without ambiguity.
- **Multiple peers**: Stacked concentric rings, one per peer with a selection on that node.
- **De-selection**: Border is removed when the peer deselects or disconnects.

### 2.3 Peer Avatar Strip

**Rendering**: A compact horizontal strip of peer avatar bubbles in the graph pane's top-right corner, visible when at least one peer is connected and presence is active.

- Each bubble shows the peer's initials (first two characters of `display_name`) on the peer's accent color background.
- A green dot indicator on the bubble shows the peer is actively connected.
- Clicking a bubble enters Follow Mode for that peer (§2.4) or opens a tooltip with peer name and last activity timestamp.

### 2.4 Follow Mode

**Behavior**: When the user activates Follow Mode for a peer, the local camera tracks that peer's viewport — pan and zoom — in real time.

- **Activation**: Click a peer avatar bubble in the avatar strip.
- **Indicator**: The active avatar bubble gains a "following" ring border; the graph pane toolbar shows "Following: Marks-iPhone [✕]".
- **Exit**: Clicking the ✕ in the toolbar indicator, clicking the same avatar bubble again, or performing a local pan/zoom gesture exits follow mode immediately.
- **Conflict with local input**: Any pan/zoom input from the local user immediately exits follow mode. Follow mode is passive — it never overrides user intent.
- **Read-only peers**: Follow mode works regardless of whether the peer has read or write access to the workspace.

---

## 3. Minimum Presence Events

Presence requires a new lightweight event channel alongside the existing `SyncUnit` data-sync stream. Presence events are **not** recorded in the workspace intent log — they are ephemeral and must not affect workspace state.

### 3.1 Presence Event Wire Format

```rust
/// Ephemeral presence event — not persisted, not applied to workspace state.
/// Sent over the same iroh QUIC stream as SyncUnit, on a separate logical channel.
enum PresenceEvent {
    /// Peer's graph-space cursor position.
    /// Sent at most every 50ms (20 Hz cap) to limit bandwidth.
    CursorMoved {
        peer_id: NodeId,
        workspace: String,
        graph_x: f32,
        graph_y: f32,
        timestamp_ms: u64,
    },
    /// Peer's current selection changed.
    SelectionChanged {
        peer_id: NodeId,
        workspace: String,
        selected_node_ids: Vec<NodeId>,
        selected_edge_ids: Vec<EdgeId>,
    },
    /// Peer's camera viewport changed (for follow mode).
    ViewportChanged {
        peer_id: NodeId,
        workspace: String,
        center_x: f32,
        center_y: f32,
        zoom: f32,
    },
    /// Peer explicitly disconnected from presence (clean exit).
    PresenceLeft {
        peer_id: NodeId,
        workspace: String,
    },
}
```

### 3.2 Transmission Policy

| Event | Rate cap | Trigger |
| --- | --- | --- |
| `CursorMoved` | 20 Hz (50ms minimum interval) | Pointer moved in graph pane |
| `SelectionChanged` | On change | Selection set changes |
| `ViewportChanged` | 10 Hz (100ms minimum interval) | Camera pan or zoom in graph pane |
| `PresenceLeft` | On disconnect / tab close | User closes workspace or session ends |

- `CursorMoved` and `ViewportChanged` are rate-limited client-side before transmission to avoid flooding the iroh stream.
- Presence events are transmitted only to peers with an active session on the same workspace. They are not queued when a peer is offline — they are dropped.
- Presence events do not affect `VersionVector` or `SyncLog` — they carry no causal history.

### 3.3 Presence Channel Lifecycle

Presence events are multiplexed on a separate QUIC stream within the same iroh connection used for data sync. The `SyncWorker` opens a `presence` substream on connection establishment (after authentication in Step 5.3). If the peer's Graphshell version does not support presence, the substream open is a no-op (capability negotiation via stream ID convention).

---

## 4. Diagnostics Channels

All presence diagnostics follow the `verse.presence.*` namespace, consistent with `verse.sync.*` naming in §8.4 of the Tier 1 plan.

| Channel | Emitted When |
| --- | --- |
| `verse.presence.peer_joined` | Peer sends first presence event for a workspace |
| `verse.presence.peer_left` | `PresenceLeft` received or connection dropped mid-session |
| `verse.presence.cursor_received` | `CursorMoved` received from peer (debug/verbose only) |
| `verse.presence.viewport_received` | `ViewportChanged` received from peer (debug/verbose only) |
| `verse.presence.follow_mode_entered` | User activates Follow Mode for a peer |
| `verse.presence.follow_mode_exited` | Follow Mode exited (user gesture or manual exit) |
| `verse.presence.event_dropped` | Presence event dropped (peer offline, rate-limit, or workspace mismatch) |

`cursor_received` and `viewport_received` are verbose-level diagnostics (not shown in default diagnostics pane) to avoid flooding the diagnostics stream at 20 Hz.

---

## 5. Privacy Constraints

Presence is an opt-in feature gated by workspace access and explicit session participation.

### 5.1 Access Control Gate

Presence events are only accepted from peers who have an active `WorkspaceGrant` for the relevant workspace (§5.5 of the Tier 1 plan). A peer with no grant for workspace W cannot receive or send presence events for W. The `SyncWorker` rejects presence events for non-granted workspaces with a `verse.presence.event_dropped` diagnostic.

### 5.2 Presence Opt-Out

The Sync Panel includes a per-workspace presence toggle:

```
☑ Share my presence in this workspace
  (cursor position, selection, viewport)
```

When unchecked, the local instance does not emit any `PresenceEvent` for that workspace. It still receives and renders remote peers' presence (so the user can see others without being seen). A full mutual opt-out requires both sides to disable presence.

### 5.3 Presence vs. Sync Independence

Presence events are never written to the workspace intent log. A peer who observes your presence cannot reconstruct your browsing history, undo/redo queue, or workspace state beyond what is already sync-visible. Presence communicates only: current cursor position (ephemeral), current selection (ephemeral, mirrored in sync state anyway), and current viewport (ephemeral, local-only state).

### 5.4 No Presence Without Active Session

Presence is only active while a peer has the workspace open and the iroh connection is live. Historical or reconstructed presence (e.g., "last seen at this node") is explicitly out of scope for the minimum presence feature and should be treated as a separate analytics/history feature if ever considered.

---

## 6. Phased Scope

### Phase P-1 (Minimum Viable Presence)

- `PresenceEvent` wire format and transmission on QUIC presence substream
- `CursorMoved` receive path and ghost cursor rendering
- `SelectionChanged` receive path and remote selection border rendering
- Peer avatar strip with connection status
- `verse.presence.*` diagnostics channels
- Per-workspace presence opt-out toggle in Sync Panel

### Phase P-2 (Follow Mode)

- `ViewportChanged` receive and transmit
- Follow Mode activation via avatar bubble
- Follow Mode toolbar indicator and exit gesture
- `verse.presence.follow_mode_entered` / `verse.presence.follow_mode_exited` diagnostics

### Phase P-3 (Presence Polish — deferred)

- Presence-aware node tooltip: "Also viewing: Marks-iPhone" when a peer's cursor is over a node
- Presence history: "last active N minutes ago" label on avatar bubble (derived from last received event timestamp — no new event channel required)
- Cursor trail / path visualization (exploratory — evaluate after P-1/P-2 land)

---

## 7. Implementation Notes

### 7.1 Rendering Integration

Ghost cursors and remote selection borders are rendered as overlays in `render/mod.rs` (graph pane draw path), after all node/edge geometry is drawn but before the local selection highlight. This ensures local selection always renders on top of remote selection, preserving clear ownership semantics.

Presence overlay data (cursor positions, peer selections, peer viewports) is held in a `PresenceState` struct owned by the graph view and updated on receipt of `PresenceEvent`s from the `SyncWorker`. The `SyncWorker` sends presence events to the render thread via the existing `GraphIntent` channel using a new `GraphIntent::ApplyPresenceEvent` variant that does not touch the workspace graph or intent log.

### 7.2 Color Assignment

Peer accent colors are derived deterministically:

```rust
fn peer_accent_color(node_id: &NodeId) -> egui::Color32 {
    // Use first 4 bytes of NodeId as hue seed
    let hue_seed = u32::from_le_bytes(node_id.as_bytes()[0..4].try_into().unwrap());
    let hue = (hue_seed % 360) as f32 / 360.0;
    // Fixed saturation and value for legibility on graph backgrounds
    egui::Color32::from(egui::ecolor::Hsva::new(hue, 0.8, 0.9, 1.0))
}
```

This avoids requiring a color negotiation protocol between peers and ensures both sides render the same color for a given peer.

### 7.3 Presence State and `GraphIntent`

```rust
/// Non-persisted presence update delivered to the render/view layer only.
/// Does not enter the workspace intent log or SyncLog.
GraphIntent::ApplyPresenceEvent {
    event: PresenceEvent,
}
```

The reducer handles `ApplyPresenceEvent` by updating `AppState::presence` (a per-workspace `HashMap<NodeId, PeerPresenceState>`) without touching the workspace graph.

```rust
struct PeerPresenceState {
    display_name: String,
    accent_color: egui::Color32,
    cursor_graph_pos: Option<egui::Pos2>,
    selected_node_ids: HashSet<NodeId>,
    selected_edge_ids: HashSet<EdgeId>,
    viewport: Option<CameraViewport>,
    last_event_at: std::time::Instant,
}
```

---

## 8. Open Questions

1. **Presence substream multiplexing**: Should presence use a dedicated QUIC stream or be interleaved with `SyncUnit` frames using a message-type discriminant? (Recommendation: separate stream with a well-known stream ID so presence backpressure does not stall data sync.)

2. **Cursor rate limiting enforcement**: Should rate limiting be enforced sender-side only (trust-based) or receiver-side with drop policy? (Recommendation: sender-side by default; receiver-side drop if `cursor_received` rate exceeds 25 Hz from a single peer as a DOS guard.)

3. **Presence and workspace read-only access**: Should `ReadOnly` peers be allowed to emit `CursorMoved` and `SelectionChanged`? (Recommendation: yes — presence is independent of mutation access; a `ReadOnly` collaborator should still be visible.)

4. **Avatar color collision**: Two peers with similar `NodeId` prefix bytes could receive the same hue. Accept with documentation, or add a minimum angular distance check across active peers? (Recommendation: defer until observed in practice.)

---

## 9. Source References

- `design_docs/graphshell_docs/research/2026-02-18_graph_ux_research_report.md` §15.2 (original ghost cursors concept)
- `design_docs/archive_docs/checkpoint_2026-02-24/GRAPHSHELL_P2P_COLLABORATION.md` (P2P collaboration vision source)
- `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md` §2 item 3 (forgotten concept adoption trigger)
- `design_docs/verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md` §5–§8 (Phase 5 sync foundation this plan depends on)
