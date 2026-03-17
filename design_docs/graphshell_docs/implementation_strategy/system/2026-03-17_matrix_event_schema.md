<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Graphshell Matrix Event Schema

**Date**: 2026-03-17
**Status**: Draft / architectural contract
**Scope**: Defines the initial Graphshell-owned Matrix room event namespace,
payload shapes, and the allowed projection or intent-routing behavior for each event family.

**Related docs**:

- [`2026-03-17_matrix_core_adoption_plan.md`](2026-03-17_matrix_core_adoption_plan.md) - adoption phases and event allowlist
- [`register/matrix_core_registry_spec.md`](register/matrix_core_registry_spec.md) - `MatrixCore` provider boundary
- [`2026-02-21_lifecycle_intent_model.md`](2026-02-21_lifecycle_intent_model.md) - current reducer-facing carrier rules
- [`register/SYSTEM_REGISTER.md`](register/SYSTEM_REGISTER.md) - routing decision table (`GraphIntent` vs `WorkbenchIntent` vs signal)
- [`2026-03-17_multi_identity_binding_rules.md`](2026-03-17_multi_identity_binding_rules.md) - identity-linking semantics for Matrix IDs, `npub`, and `NodeId`

---

## 1. Purpose

Matrix room events are not allowed to mutate Graphshell state generically.

This schema defines the small set of **Graphshell-owned event types** that may:

- project durable shared-space metadata into Graphshell surfaces
- reference graph objects and workspace targets in a stable way
- propose bounded reducer/workbench actions through host-owned mapping code

Anything outside this schema is:

- observe-only, or
- unsupported and explicitly rejected with diagnostics

---

## 2. Namespace Rule

All Graphshell-owned Matrix event types use the `graphshell.room.*` namespace.

Initial reserved event types:

- `graphshell.room.link_node`
- `graphshell.room.open_workspace_ref`
- `graphshell.room.attach_reference`
- `graphshell.room.publish_selection`
- `graphshell.room.identity_link`
- `graphshell.room.space_descriptor`

Rules:

1. Only Graphshell-owned namespaced events may be treated as intent-capable.
2. Native Matrix event types (`m.room.message`, membership, topic, etc.) remain observe-only
   unless a separate schema explicitly upgrades them.
3. Experimental event types must live under `graphshell.room.experimental.*` and remain
   non-default until promoted into this document.

---

## 3. Envelope Shape

All Graphshell-owned Matrix events should normalize to a shared conceptual envelope:

```rust
pub struct GraphshellRoomEventEnvelope {
    pub event_type: String,
    pub room_id: String,
    pub sender_matrix_id: String,
    pub event_id: String,
    pub origin_server_ts_ms: u64,
    pub content: serde_json::Value,
}
```

Normalization rule:

- `MatrixCore` converts raw SDK events into this envelope first.
- Host-owned validation then deserializes `content` into a typed Graphshell event payload.
- Only successfully validated payloads may continue to projection or intent mapping.

---

## 4. Event Families

### 4.1 `graphshell.room.link_node`

Purpose:

- Link an existing graph node or durable node address into a room as a shared reference.

Payload:

```rust
pub struct LinkNodeEvent {
    pub workspace_ref: Option<String>,
    pub node_ref: GraphNodeRef,
    pub presentation: Option<NodePresentationHint>,
    pub authored_reason: Option<String>,
}
```

Where:

```rust
pub enum GraphNodeRef {
    NodeKey { node_key: String },
    NodeAddress { address: String },
    ExternalUrl { url: String },
}
```

Allowed behavior:

- project the shared reference into room UI
- if validated and user/policy allows, propose a bounded graph-side "attach or reveal this node"
  action

Forbidden behavior:

- direct node creation without host-side validation
- arbitrary metadata overwrite on the referenced node

Preferred routing:

- current bridge carrier: reducer proposal or `AppCommand`-equivalent wrapper
- if only a workbench reveal/open is required, route through workbench-owned open/reveal path

### 4.2 `graphshell.room.open_workspace_ref`

Purpose:

- Reference a workspace, graph view, or other durable Graphshell address that another room
  participant can open.

Payload:

```rust
pub struct OpenWorkspaceRefEvent {
    pub target_uri: String,
    pub label: Option<String>,
    pub open_mode: WorkspaceOpenMode,
}

pub enum WorkspaceOpenMode {
    FocusExisting,
    OpenInNewPane,
    OpenInNewTile,
}
```

Allowed behavior:

- show a room-visible shared target
- propose a bounded open/reveal action when the local user accepts it or policy allows

Preferred routing:

- workbench-open carrier (`WorkbenchIntent`, `AppCommand`, or current bridge equivalent)

Forbidden behavior:

- implicit navigation without explicit local acceptance, unless the room surface explicitly
  invokes the action

### 4.3 `graphshell.room.attach_reference`

Purpose:

- Attach a structured reference or annotation to a graph object from a room discussion.

Payload:

```rust
pub struct AttachReferenceEvent {
    pub target: GraphReferenceTarget,
    pub reference_kind: RoomReferenceKind,
    pub source_uri: Option<String>,
    pub title: Option<String>,
    pub note_markdown: Option<String>,
}
```

Where:

```rust
pub enum GraphReferenceTarget {
    NodeKey { node_key: String },
    NodeAddress { address: String },
    SelectionToken { selection_token: String },
}

pub enum RoomReferenceKind {
    Citation,
    DiscussionAnchor,
    TaskLink,
    RelatedWork,
}
```

Allowed behavior:

- create a room-scoped projection for discussion/reference panels
- propose a bounded graph annotation/reference intent if validation succeeds

Forbidden behavior:

- arbitrary edge creation to unknown targets
- background mutation against an unloaded or unverifiable target without explicit host handling

Preferred routing:

- reducer-owned graph relation/annotation carrier

### 4.4 `graphshell.room.publish_selection`

Purpose:

- Publish a user-selected set of graph objects into the room in a Graphshell-aware format.

Payload:

```rust
pub struct PublishSelectionEvent {
    pub selection_items: Vec<PublishedSelectionItem>,
    pub selection_label: Option<String>,
    pub snapshot_mode: SelectionSnapshotMode,
}

pub struct PublishedSelectionItem {
    pub object_ref: GraphObjectRef,
    pub title: Option<String>,
    pub canonical_uri: Option<String>,
}

pub enum SelectionSnapshotMode {
    LiveReference,
    SnapshotReference,
}
```

Allowed behavior:

- room projection of the shared selection
- optional import/propose flow into a local graph surface

Forbidden behavior:

- silent bulk import into graph truth
- treating room-published selections as canonical graph ownership transfer

Preferred routing:

- projection first; user-initiated import second

### 4.5 `graphshell.room.identity_link`

Purpose:

- Carry an identity-link claim or room-scoped identity presentation record.

Payload:

```rust
pub struct IdentityLinkEvent {
    pub linked_matrix_id: String,
    pub linked_user_identity: Option<String>,
    pub linked_node_id: Option<String>,
    pub verification_mode: IdentityLinkVerificationMode,
}

pub enum IdentityLinkVerificationMode {
    SignedAssertion,
    SessionVerified,
    UserConfirmed,
}
```

Allowed behavior:

- enrich room/member presentation
- feed the binding authority for explicit verification or user-review flows

Forbidden behavior:

- automatic permission transfer
- automatic trust promotion for Device Sync or room moderation

Preferred routing:

- projection state and identity-binding authority only

### 4.6 `graphshell.room.space_descriptor`

Purpose:

- Define high-level metadata for a Graphshell-aware Matrix room space.

Payload:

```rust
pub struct SpaceDescriptorEvent {
    pub descriptor_version: u32,
    pub room_kind: GraphshellRoomKind,
    pub linked_workspace_uri: Option<String>,
    pub default_open_mode: Option<WorkspaceOpenMode>,
    pub affordances: Vec<String>,
}

pub enum GraphshellRoomKind {
    SharedWorkspace,
    DiscussionRoom,
    ReviewRoom,
    BroadcastRoom,
}
```

Allowed behavior:

- configure room surface presentation
- drive room affordances and empty-state messaging

Forbidden behavior:

- automatic mutation of graph/workbench state by descriptor alone

Preferred routing:

- projection/configuration only

---

## 5. Validation Rules

Every Graphshell-owned Matrix event must pass all applicable checks before projection or routing:

1. `event_type` is one of the allowlisted Graphshell event types
2. payload matches the typed schema exactly
3. referenced objects/addresses pass local validation rules
4. any identity references obey the multi-identity binding rules
5. event size stays within a documented upper bound for Graphshell room payloads

Rejection rule:

- invalid or unsupported events emit a `mod:matrix:event_rejected`-style diagnostic and stop there

---

## 6. Carrier Mapping Rules

The schema does not authorize a universal "event executes command" pipeline.

Instead:

- `graphshell.room.space_descriptor` -> projection only
- `graphshell.room.identity_link` -> identity-binding projection only
- `graphshell.room.open_workspace_ref` -> workbench-open proposal only
- `graphshell.room.link_node` -> projection plus bounded graph/workbench proposal
- `graphshell.room.attach_reference` -> projection plus bounded graph-intent proposal
- `graphshell.room.publish_selection` -> projection plus explicit import/publish affordance

Current architecture note:

- the active bridge carrier may still be `GraphIntent` / workbench bridge forms
- this doc should not be read as freezing the long-term top-level carrier
- the authority split remains the stable truth: graph mutation stays reducer-owned and tile/open
  behavior stays workbench-owned

---

## 7. Diagnostics

Suggested diagnostics channels:

- `mod:matrix:event_rejected` - malformed or unsupported Graphshell room event
- `mod:matrix:event_projected` - valid event projected into room/graphshell state
- `mod:matrix:intent_proposal_rejected` - allowlisted event failed validation for routing
- `mod:matrix:identity_link_unverified` - room identity-link event lacks strong verification

Severity guidance:

- rejected/unsupported payloads: `Warn`
- projection successes: `Info`
- security or policy violations: `Error`

---

## 8. Example Flow

For `graphshell.room.open_workspace_ref`:

> When a room participant posts a `graphshell.room.open_workspace_ref` event in a Matrix-backed
> room, `MatrixCore` normalizes and validates the payload, which causes a room-shared workspace
> reference projection owned by the Matrix projection layer, resulting in an "Open in Graphshell"
> affordance on room surfaces and, on user invocation, a workbench-owned open action.

This is the canonical sentence form for the event family.

---

## 9. Planning Implications

Before any Matrix implementation work claims graph-affecting behavior, it should:

1. point to this schema for event typing
2. name the exact allowed event family
3. state whether the route ends in projection, reducer proposal, or workbench proposal
4. include rejection diagnostics and non-goal statements

This keeps the Matrix lane additive and controlled rather than becoming a generic remote-control path.
