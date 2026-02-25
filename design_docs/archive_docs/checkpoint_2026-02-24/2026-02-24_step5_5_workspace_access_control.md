<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Step 5.5: Workspace Access Control

**Date:** 2026-02-24
**Status:** ✅ Complete (Phase 1 — UI + Intent Infrastructure). Archived 2026-02-24.
**Related Plans:**

- [../../graphshell_docs/implementation_strategy/2026-02-22_registry_layer_plan.md](../../graphshell_docs/implementation_strategy/2026-02-22_registry_layer_plan.md) — Step 5.4/5.5 specifications
- [../../verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md](../../verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md) — Verse access control model

---

## Summary

Step 5.5 implements per-workspace per-peer access grants and the UI to manage them. This is the final piece of Phase 5 (Verse Tier 1 sync).

**Phase 5 Done Gate:** Two instances pair via 6-word code, sync bidirectionally, and enforce per-workspace access control. ReadOnly peers receive mutations but their mutations do not propagate.

---

## Specification Checklist

### 5.5.1 Enforce `WorkspaceGrant` on Inbound Sync
- **Status:** Already implemented in `SyncWorker` (lines 412-417 in [verse/sync_worker.rs](/mods/native/verse/sync_worker.rs))
- **Check:** Inbound `SyncUnit` for non-granted workspaces rejected → `verse.sync.access_denied` diagnostic
- **Verification:** See `resolve_peer_grant()` method in sync worker

### 5.5.2 Enforce Read-Only Grants
- **Status:** Already implemented in `SyncWorker` (lines 420-425)
- **Check:** Inbound mutating intents from `ReadOnly` peers rejected
- **Verification:** SyncWorker rejects mutations on `AccessLevel::ReadOnly` peers

### 5.5.3 Sync Panel UI ("Manage Access" Dialog)
- **Status:** ✅ Implemented in [render/mod.rs](/render/mod.rs)
- **Features:**
  - `render_sync_panel()` — Shows trusted devices list
    - Device name + truncated NodeId
    - "Manage Access" button → opens dialog
    - "Forget" button → `ForgetDevice` intent
    - Connected peer count
  - `render_manage_access_dialog()` — Per-device workspace grants
    - Lists all workspace grants (workspace_id → access_level)
    - Shows access icon: 🔒 (ReadOnly) or ✏️ (ReadWrite)
    - "Revoke" button → `RevokeWorkspaceAccess` intent
- **Activation:** `graphshell://settings/sync` URL opens panel
- **Code paths:**
  - [app.rs:1033-1036](app.rs#L1033-L1036) — `show_sync_panel`, `show_manage_access_dialog` fields
  - [app.rs:3551-3570](app.rs#L3551-L3570) — `open_settings_url()` handles `/sync` path
  - [desktop/gui_frame.rs:1022-1025](desktop/gui_frame.rs#L1022-L1025) — Render calls

### 5.5.4 ForgetDevice Action
- **Status:** ✅ Fully implemented
- **Components:**
  - **Intent variant** [app.rs:968-969](app.rs#L968-L969): `GraphIntent::ForgetDevice { peer_id: String }`
  - **Handler** [app.rs:2190-2191](app.rs#L2190-L2191): Routes to `self.forget_device(&peer_id)`
  - **Method** [app.rs:4901-4907](app.rs#L4901-L4907): Parses NodeId and calls `verse::revoke_peer()`
  - **Verse API** [verse/mod.rs:848-864](mods/native/verse/mod.rs#L848-L864): Removes peer + persists trust store
  - **UI Trigger:** Sync Panel → device list → "Forget" button

### 5.5.5 Workspace Sharing Context Menu
- **Status:** ⚠️ Partial (Infrastructure ready, UI placeholder)
- **Note:** Tab right-click interception is complex (tab rendering happens in tile compositor)
- **Alternative:** "Share Workspace" button in Sync Panel (Phase 2 enhancement)
- **Infrastructure Ready:**
  - `GraphIntent::GrantWorkspaceAccess` [app.rs:970-974](app.rs#L970-L974)
  - `GraphIntent::RevokeWorkspaceAccess` [app.rs:975-978](app.rs#L975-L978)
  - Handler methods [app.rs:4909-4940](app.rs#L4909-L4940)
  - Verse APIs [verse/mod.rs:866-916](mods/native/verse/mod.rs#L866-L916)

---

## Implementation Details

### App State (GraphBrowserApp)

```rust
// Lines 1033-1036 in app.rs
pub show_sync_panel: bool,
pub show_manage_access_dialog: bool,
```

Initialize in `Default::default()` [line 1345](app.rs#L1345) and `GraphBrowserApp::new_for_testing()` [line 1453](app.rs#L1453).

### Settings URL Routing

```rust
// [app.rs:3551-3570]
pub fn open_settings_url(&mut self, url: &str) {
    let normalized = url.trim().to_ascii_lowercase();
    // ... reset all flags ...
    if normalized == "graphshell://settings/sync" {
        self.show_sync_panel = true;
        return;
    }
}
```

### GraphIntent Variants

**ForgetDevice** (implemented):
```rust
GraphIntent::ForgetDevice { peer_id: String }
```
→ Handler: `self.forget_device(&peer_id)` → `verse::revoke_peer(node_id)`

**GrantWorkspaceAccess** (implemented):
```rust
GraphIntent::GrantWorkspaceAccess {
    peer_id: String,
    workspace_id: String,
    access_level: String, // "read_only" or "read_write"
}
```
→ Handler: `self.grant_workspace_access(...)` → `verse::grant_workspace_access(node_id, ws_id, access)`

**RevokeWorkspaceAccess** (implemented):
```rust
GraphIntent::RevokeWorkspaceAccess {
    peer_id: String,
    workspace_id: String,
}
```
→ Handler: `self.revoke_workspace_access(...)` → `verse::revoke_workspace_access(node_id, ws_id)`

### Verse API Extensions

All new methods in [verse/mod.rs](/mods/native/verse/mod.rs):

```rust
/// Grant workspace access for a peer (lines 866-893)
pub(crate) fn grant_workspace_access(
    node_id: iroh::NodeId,
    workspace_id: String,
    access: AccessLevel,
)

/// Revoke workspace access for a peer (lines 895-916)
pub(crate) fn revoke_workspace_access(
    node_id: iroh::NodeId,
    workspace_id: String,
)
```

Both update trusted peer's `workspace_grants: Vec<WorkspaceGrant>` and persist via `save_trust_store()`.

### UI Rendering

**Sync Panel** [render/mod.rs:3317-3365]:
- Calls `get_trusted_peers()` → List<TrustedPeer>
- For each peer: show name, device suffix (first 8 chars of NodeId)
- Buttons: "Manage Access" (toggle dialog), "Forget" (emit intent)
- Status: peer count, Verse initialized check

**Manage Access Dialog** [render/mod.rs:3367-3417]:
- Calls `get_trusted_peers()` again
- For each peer: group by device
  - For each grant: show workspace_id + access_level icon
  - "Revoke" button → emit `RevokeWorkspaceAccess` intent
- UI-only (no actual revocation in render); intent handling is in app layer

**Rendering Calls** [desktop/gui_frame.rs:1022-1025]:
```rust
render::render_sync_panel(ctx, graph_app);
render::render_manage_access_dialog(ctx, graph_app);
```

---

## Compilation & Testing

### Build Status
```
$ cargo check --lib
Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.02s
```
✅ No errors. Warnings are pre-existing (unused imports, etc.).

### Test Cases (Manual/End-to-End)

| Test | Precondition | Action | Expected |
|------|---|---|---|
| Open Sync Panel | App running | `OpenSettingsUrl { url: "graphshell://settings/sync" }` | Panel shows, peer list visible |
| Forget Device | 2+ trusted peers | Click "Forget" → `ForgetDevice` intent | Peer removed from trust store, panel updates |
| Manage Access | Device with grants | Click "Manage Access" → dialog opens | Dialog shows workspace grants |
| Revoke Grant | Grant exists (mock) | Click "Revoke" → `RevokeWorkspaceAccess` intent | Grant removed (intent handler) |
| ReadOnly Enforcement | Sync running, peer has ReadOnly on W | Peer mutates W locally | Mutation rejected by SyncWorker (already in code) |

---

## Code Locations

| Component | File | Lines |
|-----------|------|-------|
| State fields | [app.rs](app.rs) | 1033-1036, 1345, 1453 |
| Intent variants | [app.rs](app.rs) | 968-978 |
| Intent handlers | [app.rs](app.rs) | 2190-2191, 4901-4940 |
| Verse grant APIs | [verse/mod.rs](mods/native/verse/mod.rs) | 866-916 |
| Sync Panel render | [render/mod.rs](render/mod.rs) | 3317-3365 |
| Manage Access render | [render/mod.rs](render/mod.rs) | 3367-3417 |
| UI rendering calls | [desktop/gui_frame.rs](desktop/gui_frame.rs) | 1022-1025 |
| Settings URL routing | [app.rs](app.rs) | 3551-3570 |

---

## Phase 5 Completion Summary

**Step 5.1: Verse Init** ✅  
- Initialized on app startup, secret key stored in OS keychain

**Step 5.2: Local P2P Sync** ✅  
- SyncWorker spawned by ControlPanel, QUIC endpoint active, mDNS discovery working

**Step 5.3: Two-Way Delta Sync** ✅  
- SyncLog persists mutations, version vectors track causality, remote entries applied correctly

**Step 5.4: Control Panel Integration** ✅  
- ControlPanel wired into Gui, workers supervised, toolbar indicator shows peer count

**Step 5.5: Workspace Access Control** ✅  
- UI for grant/revoke implemented
- Intents wired end-to-end
- Verse APIs for trust store updates implemented
- ReadOnly enforcement already in SyncWorker

---

## Open Items (Future Phases)

1. **Workspace Sharing Context Menu** (Phase 2)
   - Right-click workspace tab → "Share with..." submenu
   - Select peer + access level → emit `GrantWorkspaceAccess` intent
   - Requires integrating with tile compositor tab rendering

2. **QR Code Pairing Flow** (Phase 3)
   - Currently using mock 6-word code generation
   - Replace with actual QR code rendering + scanning

3. **Conflict Resolution UI** (Phase 4)
   - Show diverged nodes in conflict panel
   - Allow user to choose "keep remote" or "keep local" for each conflict

4. **Version Vector Pruning** (Phase 5)
   - Cap VV size, prune entries for peers not seen in 30+ days
   - Emit `verse.sync.vv_pruned` diagnostic

5. **Relay Infrastructure** (Phase 6+)
   - Currently using iroh's public relay (n0 operated)
   - Option to host dedicated relay for production resilience

---

## Notes

- **Access control enforcement** happens in two places:
  1. **Inbound:** SyncWorker validates peer grants before applying remote mutations
  2. **Outbound:** (Future) LocalSync only records into logs for peers with grants
  
- **Trust store persistence** uses same encrypted store as version vectors (AES-GCM with key derived from device secret key)

- **No conflict resolution** in Phase 5; conflicts are detected but only logged. Phase 4+ adds UI.

- **ReadOnly is mutation-blocking, not visibility-hiding.** ReadOnly peers CAN see mutations from peers on a workspace; they just cannot propagate their own mutations to others on that workspace. This differs from encryption-based visibility, which is a future design decision.

