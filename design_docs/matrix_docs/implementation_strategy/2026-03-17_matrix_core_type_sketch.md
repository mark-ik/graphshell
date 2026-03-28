<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# MatrixCore Type Sketch

**Doc role:** Rust-facing type sketch for `MatrixCoreRegistry` and its normalized event model
**Status:** Draft / implementation-oriented planning note
**Kind:** Register implementation sketch
**Related docs:**
- [matrix_core_registry_spec.md](matrix_core_registry_spec.md) (`MatrixCore` capability boundary)
- [../2026-03-17_matrix_core_adoption_plan.md](../2026-03-17_matrix_core_adoption_plan.md) (phase sequencing and done gates)
- [../2026-03-17_matrix_event_schema.md](../2026-03-17_matrix_event_schema.md) (Graphshell-owned event families)
- [../2026-03-17_multi_identity_binding_rules.md](../2026-03-17_multi_identity_binding_rules.md) (identity-binding rules)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (routing and worker boundary rules)

**Interpretation note**:

- this document is a type sketch, not a claim that these exact names must ship unchanged
- opaque IDs, handles, and enums matter more than exact field spelling
- Matrix SDK internals should stay behind `MatrixCoreRegistry`; the rest of the app should depend on Graphshell-owned types

---

## 1. Purpose

This sketch exists to make the Matrix lane implementation-shaped.

It answers:

- what the registry roughly owns
- what a supervised Matrix worker sends back
- what the normalized event enum looks like
- how room events become projection updates or bounded intent proposals

---

## 2. Core Opaque Types

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MatrixSessionId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MatrixRoomId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MatrixEventId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MatrixUserId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MatrixProfileId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MatrixSubscriptionId(pub String);
```

Rule:

- wrap SDK-native identifiers early so the rest of Graphshell depends on Graphshell-owned types

---

## 3. Registry Shape

```rust
pub struct MatrixCoreRegistry {
    sessions: HashMap<MatrixSessionId, MatrixSessionRecord>,
    room_projections: HashMap<MatrixRoomId, MatrixRoomProjection>,
    subscription_owners: HashMap<MatrixSubscriptionId, CallerId>,
    linked_identity_index: IdentityBindingIndexHandle,
    command_tx: tokio::sync::mpsc::Sender<MatrixWorkerCommand>,
    diagnostics: DiagnosticsWriteHandle,
    signal_bus: SignalBusHandle,
}
```

Intent of each field:

- `sessions`: active authenticated Matrix sessions known to the host
- `room_projections`: Graphshell-owned room metadata and timeline projection cache
- `subscription_owners`: caller-scoped ownership and quota enforcement
- `linked_identity_index`: read/write seam for Matrix ID <-> `npub` / `NodeId` bindings
- `command_tx`: explicit worker boundary into `ControlPanel`-supervised Matrix work
- `diagnostics`: explicit failure and policy visibility
- `signal_bus`: optional non-mutating fanout for room/projection observers

Non-goal:

- no raw Matrix SDK client object escapes this registry as public API

---

## 4. Session and Projection Records

```rust
pub struct MatrixSessionRecord {
    pub session_id: MatrixSessionId,
    pub profile_id: MatrixProfileId,
    pub user_id: MatrixUserId,
    pub homeserver: String,
    pub status: MatrixSessionStatus,
    pub joined_rooms: BTreeSet<MatrixRoomId>,
}

pub enum MatrixSessionStatus {
    Opening,
    Ready,
    Degraded { reason: String },
    Closed,
}

pub struct MatrixRoomProjection {
    pub room_id: MatrixRoomId,
    pub descriptor: Option<GraphshellSpaceDescriptorProjection>,
    pub membership: MatrixMembershipSnapshot,
    pub timeline: Vec<NormalizedMatrixEvent>,
    pub affordances: BTreeSet<String>,
}
```

Notes:

- timeline projection stays Graphshell-owned and typed; it is not the raw SDK timeline object
- `descriptor` comes from `graphshell.room.space_descriptor` or equivalent host-owned projection rules

---

## 5. Public API Sketch

```rust
impl MatrixCoreRegistry {
    pub fn session_open(
        &mut self,
        profile_id: MatrixProfileId,
        config: MatrixHomeserverConfig,
    ) -> Result<MatrixSessionId, MatrixCoreError>;

    pub fn session_close(
        &mut self,
        session_id: &MatrixSessionId,
    ) -> Result<(), MatrixCoreError>;

    pub fn room_subscribe(
        &mut self,
        caller_id: CallerId,
        room_id: MatrixRoomId,
        filter: MatrixRoomFilter,
    ) -> Result<MatrixSubscriptionId, MatrixCoreError>;

    pub fn room_unsubscribe(
        &mut self,
        caller_id: &CallerId,
        subscription_id: &MatrixSubscriptionId,
    ) -> Result<(), MatrixCoreError>;

    pub fn room_send(
        &mut self,
        caller_id: CallerId,
        room_id: MatrixRoomId,
        event: GraphshellOutgoingMatrixEvent,
    ) -> Result<MatrixSendReceipt, MatrixCoreError>;

    pub fn room_membership(
        &self,
        room_id: &MatrixRoomId,
    ) -> Result<&MatrixMembershipSnapshot, MatrixCoreError>;

    pub fn room_projection(
        &self,
        room_id: &MatrixRoomId,
    ) -> Option<&MatrixRoomProjection>;
}
```

Design rule:

- external callers deal in Graphshell event and projection types, not Matrix SDK event classes

---

## 6. Worker Boundary

All async Matrix I/O stays outside the registry core behind commands and worker outputs.

```rust
pub enum MatrixWorkerCommand {
    OpenSession {
        profile_id: MatrixProfileId,
        config: MatrixHomeserverConfig,
    },
    CloseSession {
        session_id: MatrixSessionId,
    },
    SubscribeRoom {
        caller_id: CallerId,
        room_id: MatrixRoomId,
        filter: MatrixRoomFilter,
        subscription_id: MatrixSubscriptionId,
    },
    UnsubscribeRoom {
        subscription_id: MatrixSubscriptionId,
    },
    SendRoomEvent {
        caller_id: CallerId,
        room_id: MatrixRoomId,
        event: GraphshellOutgoingMatrixEvent,
    },
}
```

Worker outputs:

```rust
pub enum MatrixWorkerOutput {
    SessionOpened {
        session: MatrixSessionRecord,
    },
    SessionDegraded {
        session_id: MatrixSessionId,
        reason: String,
    },
    SessionClosed {
        session_id: MatrixSessionId,
    },
    RoomEventObserved {
        room_id: MatrixRoomId,
        event: NormalizedMatrixEvent,
    },
    RoomMembershipUpdated {
        room_id: MatrixRoomId,
        membership: MatrixMembershipSnapshot,
    },
    CommandFailed {
        operation: MatrixOperationKind,
        reason: String,
    },
}
```

Rule:

- worker outputs still do not mutate graph/workbench state directly; they are consumed by host-owned projection and routing code

---

## 7. Normalized Event Model

Raw Matrix events should normalize into one Graphshell-owned enum:

```rust
pub enum NormalizedMatrixEvent {
    Native(NativeMatrixEvent),
    Graphshell(GraphshellRoomEvent),
}
```

Native events:

```rust
pub enum NativeMatrixEvent {
    RoomMessage(RoomMessageProjection),
    Membership(MatrixMembershipEventProjection),
    Topic(RoomTopicProjection),
    RoomName(RoomNameProjection),
    RoomAvatar(RoomAvatarProjection),
}
```

Graphshell namespaced events:

```rust
pub enum GraphshellRoomEvent {
    LinkNode(LinkNodeEvent),
    OpenWorkspaceRef(OpenWorkspaceRefEvent),
    AttachReference(AttachReferenceEvent),
    PublishSelection(PublishSelectionEvent),
    IdentityLink(IdentityLinkEvent),
    SpaceDescriptor(SpaceDescriptorEvent),
}
```

Rule:

- `NativeMatrixEvent` is observe-only by default
- `GraphshellRoomEvent` is still not automatically intent-capable; it must pass per-variant routing rules from the event schema

---

## 8. Outgoing Event Model

Outgoing Graphshell-authored room events should also be typed:

```rust
pub enum GraphshellOutgoingMatrixEvent {
    Graphshell(GraphshellRoomEvent),
    PlainTextMessage {
        body: String,
    },
}
```

Reason:

- the sending side should reuse the same Graphshell event payload types rather than constructing ad hoc JSON

---

## 9. Projection and Routing Result

After normalization and validation, Matrix events should map into one of a few explicit outcomes:

```rust
pub enum MatrixEventDisposition {
    ObserveOnly,
    ProjectionUpdate(MatrixProjectionUpdate),
    GraphIntentProposal(MatrixGraphIntentProposal),
    WorkbenchIntentProposal(MatrixWorkbenchIntentProposal),
    Rejected { reason: String },
}
```

Projection updates:

```rust
pub enum MatrixProjectionUpdate {
    AppendTimelineEvent {
        room_id: MatrixRoomId,
        event: NormalizedMatrixEvent,
    },
    UpdateMembership {
        room_id: MatrixRoomId,
        membership: MatrixMembershipSnapshot,
    },
    UpdateDescriptor {
        room_id: MatrixRoomId,
        descriptor: GraphshellSpaceDescriptorProjection,
    },
    UpdateLinkedIdentity {
        room_id: MatrixRoomId,
        link: MatrixIdentityLinkProjection,
    },
}
```

Intent proposals:

```rust
pub struct MatrixGraphIntentProposal {
    pub room_id: MatrixRoomId,
    pub source_event_id: MatrixEventId,
    pub intent: GraphIntent,
}

pub struct MatrixWorkbenchIntentProposal {
    pub room_id: MatrixRoomId,
    pub source_event_id: MatrixEventId,
    pub intent: WorkbenchIntent,
}
```

Current-carrier note:

- `GraphIntent` / `WorkbenchIntent` are the active bridge carriers today
- future `AppCommand` / planner layers may wrap these proposals

---

## 10. Membership and Identity Types

```rust
pub struct MatrixMembershipSnapshot {
    pub room_id: MatrixRoomId,
    pub joined: BTreeMap<MatrixUserId, MatrixMemberRecord>,
    pub invited: BTreeMap<MatrixUserId, MatrixMemberRecord>,
    pub banned: BTreeMap<MatrixUserId, MatrixMemberRecord>,
}

pub struct MatrixMemberRecord {
    pub user_id: MatrixUserId,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub power_level: Option<i64>,
    pub linked_identity: Option<MatrixLinkedIdentitySnapshot>,
}

pub struct MatrixLinkedIdentitySnapshot {
    pub matrix_id: MatrixUserId,
    pub linked_npub: Option<String>,
    pub linked_node_id: Option<String>,
    pub verification: IdentityBindingVerificationMode,
}
```

Rule:

- membership display may use linked identities
- moderation or trust decisions may not be inferred solely from the existence of a link

---

## 11. Error and Config Types

```rust
pub struct MatrixHomeserverConfig {
    pub homeserver_url: String,
    pub sliding_sync_enabled: bool,
    pub persist_session: bool,
}

pub struct MatrixRoomFilter {
    pub include_timeline: bool,
    pub include_membership: bool,
    pub include_graphshell_events_only: bool,
}

pub struct MatrixSendReceipt {
    pub room_id: MatrixRoomId,
    pub event_id: Option<MatrixEventId>,
    pub accepted: bool,
}

pub enum MatrixOperationKind {
    OpenSession,
    CloseSession,
    SubscribeRoom,
    UnsubscribeRoom,
    SendRoomEvent,
}

pub enum MatrixCoreError {
    CapabilityDenied,
    InvalidConfig(String),
    SessionUnavailable,
    MembershipDenied,
    ValidationFailed(String),
    WorkerUnavailable,
}
```

---

## 12. Suggested First Code Slice

If implementation starts, the thinnest honest slice is:

1. define opaque Matrix ID/session/room wrapper types
2. define `MatrixCoreRegistry`
3. define `MatrixWorkerCommand` / `MatrixWorkerOutput`
4. define `NormalizedMatrixEvent`
5. stub `MatrixEventDisposition` mapping with rejection by default

That yields a compile-visible boundary before any real Matrix SDK integration lands.
