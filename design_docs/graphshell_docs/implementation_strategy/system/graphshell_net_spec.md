<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# graphshell-net Spec

**Date**: 2026-04-30
**Status**: Canonical / Active
**Scope**: Unifying spec for Graphshell-side network resource handling:
**downloads, uploads, prefetch, DNS prefetch, service workers, push
notifications, and outbound HTTP requests originated by Graphshell
itself** (e.g., URL completion providers, agent-pane LLM requests, sync
worker traffic). Defines the `graphshell-net` subsystem boundary, the
`NetIntent` family, the relationship to Servo's networking and Wry's
networking, the per-graph and per-persona scope coupling, and the
permission contract through the
[settings + permissions spine](../aspect_control/settings_and_permissions_spine_spec.md).

**Related**:

- [`../subsystem_security/SUBSYSTEM_SECURITY.md`](../subsystem_security/SUBSYSTEM_SECURITY.md) — security subsystem (transport trust, mixed-content, origin permissions; consumes net signals)
- [`../subsystem_storage/SUBSYSTEM_STORAGE.md`](../subsystem_storage/SUBSYSTEM_STORAGE.md) — download persistence, cache backing
- [`../aspect_control/settings_and_permissions_spine_spec.md`](../aspect_control/settings_and_permissions_spine_spec.md) — net access permissions follow the same five-scope hierarchy
- [`../shell/iced_browser_amenities_spec.md` §5](../shell/iced_browser_amenities_spec.md) — Downloads amenity (consumes graphshell-net downloads)
- [`../shell/iced_agent_pane_spec.md`](../shell/iced_agent_pane_spec.md) — agent pane LLM traffic flows through graphshell-net
- [`../../TERMINOLOGY.md`](../../TERMINOLOGY.md) — `Intent`, `Idempotence + Replay Contract`
- [SUBSYSTEM_HISTORY.md](../subsystem_history/SUBSYSTEM_HISTORY.md) — net events surface in Activity Log

---

## 1. Intent

Network handling in Graphshell prior to 2026-04-30 was scattered:

- **Web content** went through Servo or Wry (each handles its own
  network stack).
- **Downloads** were sketched in `iced_browser_amenities_spec.md §5`
  but no unifying subsystem owned the resource lifecycle.
- **Provider requests** (URL completions, history-by-URL) used
  `HostRequestMailbox` under `ControlPanel` supervision but had no
  shared network policy.
- **Agent LLM traffic** was implicit in `AgentRegistry` agents.
- **Sync traffic** (Verso bilateral, Verse community) had its own
  transport (iroh / libp2p).

`graphshell-net` is the unifying subsystem boundary for **outbound
network requests originated by Graphshell** (or its mods) that don't
belong to a viewer engine. Servo's and Wry's per-page networking
remain inside their respective engines; Graphshell-originated traffic
flows through `graphshell-net`.

**Out of scope**:

- Servo's per-page HTTP, TLS, cookie jar, HTTP cache (Servo owns).
- Wry's per-webview networking (Wry owns; webview2/wkwebview-backed).
- Verso bilateral sync iroh transport (lives in `crates/verso`).
- Verse community libp2p transport (lives in Verse mod, Tier 2).

**In scope**:

- **Downloads**: file-saved-to-disk operations, regardless of which
  surface initiated.
- **Uploads**: outbound payloads for sync, agent requests, etc.
- **Prefetch / DNS prefetch / preconnect**: speculative network warming
  Graphshell initiates (e.g., for likely-next-tile addresses).
- **Push notifications**: server-pushed events Graphshell handles for
  PWA-like content.
- **Provider HTTP**: URL completion providers, search engine queries
  (per the [search providers spec](../shell/search_providers_and_fuzzy_spec.md)),
  history/bookmark imports.
- **Agent traffic**: outbound LLM API requests from `AgentRegistry`
  agents (per the [agent pane spec](../shell/iced_agent_pane_spec.md)).
- **Mod-originated traffic**: a mod that needs HTTP (e.g., a Nostr-
  relay mod) routes through `graphshell-net` rather than opening its
  own client.

---

## 2. Architecture

`graphshell-net` is a **subsystem** per
[DOC_POLICY.md §11](../../DOC_POLICY.md), with a canonical authority
doc (this spec is the authority — there is no separate
`SUBSYSTEM_NET.md`; this spec serves both roles). Code lives in
`crates/graphshell-net/`.

### 2.1 Boundary

```text
                       ┌───────────────────────────────┐
                       │       graphshell-net          │
                       │                               │
   downloads_intent ───┤ → DownloadManager             │
   provider_request  ──┤ → ProviderClient              ├─→ HTTP outbound
   agent_request    ───┤ → AgentClient                 │   (via reqwest /
   prefetch_intent  ───┤ → PrefetchScheduler           │    hyper / etc.)
   push_subscription ──┤ → PushReceiver                │
                       │                               │
                       │   per-request: scope path,    │
                       │   permission check, transport │
                       │   trust verification          │
                       └───────────────────────────────┘
                                       │
                                       │ NetEvent stream
                                       ▼
                  Activity Log / SUBSYSTEM_HISTORY
                  Settings/Permissions Spine
                  Tool panes (Downloads, Diagnostics)
```

The subsystem is one process / one task pool; supervised by
`ControlPanel` per the existing register architecture.

### 2.2 What it is not

`graphshell-net` is **not** a generic HTTP client library. It is a
**policy layer** wrapping outbound HTTP with:

- per-request scope-path tracking (which persona / graph / view
  initiated)
- permission gating (per the settings + permissions spine)
- transport trust verification (TLS state surfaces to
  `subsystem_security`)
- shared connection pool (one set of pooled connections for the whole
  app, not per-call)
- centralized rate limiting and backpressure
- audit logging (every outbound request lands in Activity Log via
  SUBSYSTEM_HISTORY)
- cancellation by `RequestId`

Internally it can use `reqwest` (or `hyper` directly); that's an
implementation detail, not a contract.

### 2.3 Why centralize

Each of the seven request kinds (downloads, uploads, prefetch, DNS
prefetch, push, provider, agent) wants the same policy primitives:
permission check, scope tracking, audit, cancel, rate-limit. Without
a shared layer, those primitives get re-implemented (or skipped) per
caller. Centralization buys consistency and one audit surface.

---

## 3. Request Kinds

### 3.1 Downloads

Save a network resource to disk.

```rust
pub struct DownloadRequest {
    pub url: Url,
    pub destination: DownloadDestination,
    pub initiator: Initiator,                  // user / agent / mod / sync
    pub scope: ScopePath,                      // for permission resolution
    pub trust_requirements: TrustRequirements, // require TLS, etc.
}

pub enum DownloadDestination {
    Default,                                   // user's default directory
    Path(PathBuf),
    GraphNode(NodeKey),                        // materialize as graph node content
}
```

Downloads always materialize in **two** places: the filesystem
(or the graph store) AND the Activity Log + downloads tool pane.
Per the [iced_browser_amenities_spec §5](../shell/iced_browser_amenities_spec.md),
completed downloads become graph nodes by default.

### 3.2 Uploads

Outbound payloads for non-sync, non-agent traffic. Used by mods
that publish content (Nostr, Verse), import-export tooling, etc.

```rust
pub struct UploadRequest {
    pub url: Url,
    pub method: HttpMethod,                    // POST / PUT / PATCH
    pub payload: Bytes,
    pub initiator: Initiator,
    pub scope: ScopePath,
    pub trust_requirements: TrustRequirements,
}
```

### 3.3 Prefetch / DNS prefetch / preconnect

Speculative warming of network resources. Three intensities:

- **DNS prefetch**: resolve hostname; do not open connection.
- **Preconnect**: open TCP+TLS but do not request.
- **Prefetch**: GET the resource into a short-lived cache.

Initiated by Graphshell heuristics (e.g., hover on a node likely to
activate; recently-recurring tile addresses) or explicitly by mods.
Subject to permission scope (`net_speculative_warming`).

### 3.4 Push notifications

Server-pushed events Graphshell receives for PWA-style content.
Subject to per-origin permission grants per the
[settings + permissions spine §5](../aspect_control/settings_and_permissions_spine_spec.md).

```rust
pub struct PushSubscription {
    pub origin: Origin,
    pub endpoint: Url,
    pub scope: ScopePath,
    pub topics: Vec<PushTopic>,
}
```

Servo handles per-page push for live web content; `graphshell-net`
handles Graphshell-level push (e.g., a Verse mod pushing community
event notifications).

### 3.5 Provider HTTP

Used by URL completion providers, search providers, history-import
parsers. Subject to provider-specific rate limits and per-persona
provider allowlist.

```rust
pub struct ProviderRequest {
    pub provider_id: ProviderId,               // history / bookmark / search
    pub url: Url,
    pub method: HttpMethod,
    pub scope: ScopePath,                      // typically [persona, default]
    pub timeout: Duration,
}
```

Returns through `HostRequestMailbox<ProviderResult>` per the existing
landed pattern.

### 3.6 Agent traffic

Outbound LLM/inference API requests from `AgentRegistry` agents.

```rust
pub struct AgentRequest {
    pub agent_id: AgentId,
    pub conversation_id: ConversationId,
    pub url: Url,                              // LLM endpoint
    pub method: HttpMethod,
    pub payload: Bytes,                        // the LLM request body
    pub scope: ScopePath,                      // typically [agent, persona, default]
    pub trust_requirements: TrustRequirements::TLS_REQUIRED,
}
```

Agent traffic surfaces in the Activity Log with the agent's full
provenance per [agent pane spec §4.2](../shell/iced_agent_pane_spec.md);
the request payload (which includes the conversation context) is
audit-logged so the user can verify what the agent sent to its
provider.

### 3.7 Mod traffic

Generic outbound HTTP for mod-originated requests:

```rust
pub struct ModRequest {
    pub mod_id: ModId,
    pub url: Url,
    pub method: HttpMethod,
    pub payload: Option<Bytes>,
    pub scope: ScopePath,                      // typically [mod, persona, default]
    pub trust_requirements: TrustRequirements,
}
```

Mods get their own scope key for permission grants
(`net.mod.<mod_id>`); a user can permit / deny network access per mod
per persona per graph etc.

---

## 4. Permission Contract

Every request kind has a permission gate. Per the
[settings + permissions spine §5](../aspect_control/settings_and_permissions_spine_spec.md):

| Permission key | Default | Description |
|---|---|---|
| `net.downloads` | persona-prompt | per-origin or always-allow / always-deny |
| `net.uploads` | persona-prompt | per-mod and per-origin |
| `net.prefetch` | persona-allow | speculative warming; can be denied entirely |
| `net.push` | per-origin-prompt | push notification subscriptions |
| `net.providers.<provider_id>` | persona-allow | per-provider allowlist; default-allow for canonical providers, default-deny for new mods |
| `net.agent.<agent_id>` | per-agent-grant | required for any agent outbound traffic |
| `net.mod.<mod_id>` | persona-prompt | per-mod allowlist |

Permission scope path follows the spine (default → persona → graph →
view/tile → pane). Narrowing-only constraint applies:
graph-scope `net.providers.search = deny` overrides
persona-scope `net.providers.search = allow_per_query`, but graph
scope cannot widen to `allow_always` if persona is more restrictive.

Permission denial is **explicit**: a denied request returns
`NetError::PermissionDenied { scope, key }` to the caller and emits
an Activity Log entry. There is no silent failure.

---

## 5. NetIntent and NetEvent

`graphshell-net` integrates with the canonical Intent contract:

```rust
pub enum NetIntent {
    // Initiate
    DownloadStart(DownloadRequest),
    UploadStart(UploadRequest),
    PrefetchStart(PrefetchRequest),
    PushSubscribe(PushSubscription),
    ProviderRequest(ProviderRequest),
    AgentRequest(AgentRequest),
    ModRequest(ModRequest),

    // Cancel
    Cancel(RequestId),
    CancelAllForScope(ScopePath),               // e.g., on persona switch

    // Permission management
    GrantPermission { scope: ScopePath, key: PermissionKey, value: PermissionValue },
    RevokePermission { scope: ScopePath, key: PermissionKey },
}

pub enum NetEvent {
    RequestQueued { request_id: RequestId },
    RequestStarted { request_id: RequestId },
    RequestProgress { request_id: RequestId, bytes_done: u64, bytes_total: Option<u64> },
    RequestCompleted { request_id: RequestId, response: NetResponse },
    RequestFailed { request_id: RequestId, error: NetError },
    RequestCancelled { request_id: RequestId },

    PushReceived { subscription_id: SubscriptionId, payload: PushPayload },

    PermissionPromptRequired {
        request_id: RequestId,
        scope: ScopePath,
        key: PermissionKey,
    },
    PermissionGranted { scope: ScopePath, key: PermissionKey },
    PermissionDenied { scope: ScopePath, key: PermissionKey },
}
```

`NetIntent`s satisfy the
[idempotence + replay contract](../../TERMINOLOGY.md): a duplicate
`DownloadStart` for an already-running request returns the existing
`RequestId`, not a new download. Cancellation is idempotent.

`NetEvent`s flow through a Subscription consumed by:

- Downloads tool pane (in-progress state)
- Activity Log (every event)
- Permission prompt UI (`PermissionPromptRequired`)
- Caller-facing response (per-request via `HostRequestMailbox`)

---

## 6. Persistence

| Resource | Persistence layer |
|---|---|
| Download in-progress state | in-memory; lost on crash (resumable downloads = future work) |
| Completed downloads | graph node + filesystem (per [amenities §5](../shell/iced_browser_amenities_spec.md)) |
| Push subscription registrations | per-persona settings tree (the spine) |
| Provider rate-limit / backoff state | in-memory; reset on app restart |
| Permission grants | per-spine; persistent at persona+ scope (the spine) |
| Audit log entries | SUBSYSTEM_HISTORY append-only WAL |

Resumable downloads, persistent prefetch cache, persistent
push-subscription resync are tracked as open items (§9).

---

## 7. Coherence Guarantees

Per the
[tool-pane / Downloads coherence guarantees](../shell/2026-04-28_iced_jump_ship_plan.md):

> Downloads tool pane is observer-only; downloads materialize as
> graph nodes via standard GraphReducerIntent path; never bypasses
> the uphill rule.

`graphshell-net` extends this to all network traffic:

- **No silent network access**. Every outbound request is
  permission-checked; denied requests fail with explicit error.
- **No caller-controlled rate limits**. Rate limits are subsystem-
  enforced; mods cannot bypass.
- **Audit completeness**. Every request lands in the Activity Log
  before its response returns to the caller.
- **Cancellation is durable**. Cancelling a request stops its
  outbound traffic immediately and cleans up filesystem partials.
- **TLS verification is not bypassable from caller code**. Trust
  requirements (`TrustRequirements`) are enforced inside the
  subsystem; a caller cannot opt out.

---

## 8. Relationship to Servo and Wry

Servo and Wry each have their own network stacks for the web content
they render. `graphshell-net` does **not** intercept those:

- Servo pages make HTTP through Servo's `net` crate; that traffic
  doesn't flow through `graphshell-net`. (Subsystem_security observes
  Servo's transport trust state through the existing channel.)
- Wry webviews make HTTP through the underlying platform webview
  (WebView2 / WKWebView / WebKitGTK).

Where Graphshell needs to know about web-content traffic (e.g., for
Activity Log inclusion), it observes via existing Servo/Wry hooks
(navigation events, resource-load events) — it does not replicate
the transport layer.

This separation matches the
[2026-04-30 renderer policy decision](../shell/2026-04-28_iced_jump_ship_plan.md):
"don't rely on Servo's WebRender for arbitrary scenes; use what we
have for fundamentals." Servo's network stack stays inside Servo;
Graphshell-originated traffic uses `graphshell-net`.

For the smolnet / Linebender component-crate path the user described
(2026-04-30, #14), Graphshell-originated content fetching for
middlenet-style rendering (Gemini, RSS, Markdown, plain HTML) lives
in `graphshell-net` — middlenet's network needs go through this
subsystem rather than middlenet rolling its own client.

---

## 9. Open Items

- **Resumable downloads**: in-progress download state isn't currently
  durable; HTTP Range support and partial-state persistence is a
  follow-up.
- **Persistent prefetch cache**: prefetched resources currently live
  only as long as the app session; a small persisted cache for
  warm-restart performance is a follow-up.
- **Push subscription resync**: re-establishing push subscriptions on
  app startup against an authoritative server registry. Tracked under
  the per-mod push spec.
- **Mod-defined permission keys**: the spec lists canonical permission
  keys; mods can define their own (`net.mod.<mod_id>.<feature>`) but
  the registration mechanism for new keys (settings UI surfacing)
  needs design.
- **Per-persona network identity** (e.g., user-agent string,
  fingerprinting protections): the persona scope is the natural home
  but specific policies are out of scope here.
- **Network diagnostics tool pane**: a dedicated `verso://tool/net`
  pane showing live request flow, rate-limit state, permission
  prompts, errors. Useful for debugging; not strictly required for
  bring-up.
- **Bandwidth + cost metering**: per-persona / per-graph bandwidth
  counters for cost-aware operation. Future work.

---

## 10. Bottom Line

`graphshell-net` is the unifying subsystem boundary for outbound
network traffic Graphshell originates. Seven request kinds
(downloads / uploads / prefetch / push / providers / agent / mod)
flow through one policy layer that handles permission gating,
scope tracking, transport trust, audit logging, and cancellation.
Servo and Wry retain their own network stacks for the web content
they render; this subsystem covers everything else.

Permission grants follow the five-scope settings + permissions spine.
Every request lands in the Activity Log with full provenance. The
agent pane and downloads tool pane are the primary user-facing
consumers; the diagnostics pane is the developer-facing audit
surface. Mod-originated traffic gets its own scope key for
permission management.

This closes the network-resource-handling gap that the iced jump-ship
plan §4.6 left implicit and gives every consumer (agent pane,
downloads, providers, mods, sync, push) one shared shape.
