# VERSO AS PEER

**Purpose**: Specification for Verso — Graphshell's web capability mod and bilateral peer agent. Verso represents the user on the open web and in relationship-scoped peer networking.

**Document Type**: Behavior and role specification
**Status**: Registry integration Phase 2–3 complete (Servo + ViewerRegistry); Verse Tier 1 (iroh sync) in Phase 5
**See**: [GRAPHSHELL_AS_BROWSER.md](GRAPHSHELL_AS_BROWSER.md) for the user-facing browser model; [VERSE_AS_NETWORK.md](../../verse_docs/technical_architecture/VERSE_AS_NETWORK.md) for the network protocol Verso participates in; [COMMS_AS_APPLETS.md](../../graphshell_docs/implementation_strategy/social/COMMS_AS_APPLETS.md) for optional hosted communication surfaces

---

## What Verso Is

Verso is a **native mod** — compiled into the Graphshell binary, registered at startup via `inventory::submit!`, not sandboxed. It is the single "Browser Capability" mod. It does two things:

1. **Web Peer**: brings Servo (the browser engine) and the `wry` OS webview fallback into the registry as `viewer:webview` and `viewer:wry`. These are how Graphshell renders the web.
2. **Bilateral Peer Agent**: manages Graphshell's identity, pairing, Device Sync, and co-op participation over iroh. Verso is the local agent that holds the user's keys, establishes named-peer connections, and exchanges `SyncUnit` deltas with trusted peers.

These two roles are unified in Verso because they share the same trust infrastructure: the keypair that signs Verso's bilateral sync payloads is the same identity that signs the mod's capabilities. A user who enables Verso gets both web access and peer-to-peer collaboration from a single, coherent module.

Architectural boundary:

- **Graphshell** remains the host/renderer and must remain useful with no networking at all.
- **Verso** owns bilateral peer behavior and optional co-op/session capability.
- **Verse** is the separate community-scale network layer that Verso can connect the user to, but does not collapse into the shell or into Verso's bilateral semantics.
- [**Comms**](../../graphshell_docs/implementation_strategy/social/COMMS_AS_APPLETS.md) and other social/network surfaces are optional hosted applets, not core Graphshell semantic domains.

---

## Verso as Web Peer

### What Verso Registers

On startup (via `inventory::submit!`), Verso registers the following with the registry layer:

**ViewerRegistry entries:**

| Viewer ID | Backend | Rendering mode | Usable in |
| --------- | ------- | -------------- | --------- |
| `viewer:webview` | Servo (libservo) | Texture (GPU surface) | Graph canvas + workbench tiles |
| `viewer:wry` | wry (OS webview) | Overlay (native window) | Workbench tiles only |

`viewer:webview` is the canonical default that all nodes use unless the user has set a preference. Switching to `viewer:wry` makes new webviews use the native OS webview. Per-node and per-frame overrides use specific IDs.

**ProtocolRegistry entries:**

| Scheme | Handler | Notes |
| ------ | ------- | ----- |
| `http`, `https` | Servo net layer | Default; already active |
| `file` | FileResolver | Filesystem read; FilePermissionGuard enforced |
| `about` | Built-in | `about:blank`, `about:newtab` |
| `gemini` | GeminiResolver | Optional; `--features gemini` |

**ActionRegistry entries** (via Verso):

- `navigation.back`, `navigation.forward`, `navigation.reload`, `navigation.stop`
- `webview.open_url`, `webview.create_tab`, `webview.close_tab`
- `verse.pair_device`, `verse.sync_now`, `verse.share_workspace`, `verse.forget_device`
- `gemini.start_server`, `gemini.stop_server`, `gemini.serve_node`
- `gopher.start_server`, `gopher.stop_server`, `gopher.serve_node`
- `finger.start_server`, `finger.stop_server`, `finger.publish_profile`

### The Viewer Trait Contract

All Verso-registered viewers implement:

```rust
pub trait Viewer {
    fn render_embedded(&mut self, ui: &mut egui::Ui, node: &Node) -> bool;
    fn sync_overlay(&mut self, rect: egui::Rect, visible: bool);
    fn is_overlay_mode(&self) -> bool { false }
}
```

`ServoViewer` renders embedded (returns true), `sync_overlay` is a no-op.
`WryViewer` returns false from `render_embedded` (renders thumbnail fallback in graph canvas), and calls `wry::WebView::set_bounds()` + `set_visible()` from `sync_overlay`.

See [2026-02-23_wry_integration_strategy.md](../implementation_strategy/2026-02-23_wry_integration_strategy.md) for the full Wry integration plan.

### Verso vs Other Viewers

Non-web renderers (`PdfViewer`, `ImageViewer`, `PlaintextViewer`, `AudioViewer`, `DirectoryViewer`) are **not** part of Verso. They are registered by the core seed floor of the registry (Phase 2), not by a mod. This separation is intentional:

- Verso = capability that requires network access and external software (Servo, OS webview).
- Core viewers = pure Rust renderers with no external dependencies; always available regardless of whether Verso is enabled.

A Graphshell instance without Verso is a visual outliner/file manager — all core viewers work, no web access. Verso adds the web.

Small-protocol posture:

- Servo remains the general browser engine and fallback renderer.
- Native small-protocol support is justified only when protocol-native trust, navigation, publishing, or graph semantics give the user something Servo fallback would flatten away.
- `SimpleDocument` is a useful bridge/export substrate for text-first document lanes, but it is not a mandatory universal core for discovery or messaging protocols.

See [`../research/2026-03-28_smolnet_follow_on_audit.md`](../research/2026-03-28_smolnet_follow_on_audit.md) and [`../research/2026-03-28_smolnet_dependency_health_audit.md`](../research/2026-03-28_smolnet_dependency_health_audit.md) for the current admission bar and follow-on protocol posture.

### Verso as Storage Runtime Host

If Graphshell adopts a future WHATWG-style `ClientStorageManager`, Verso is the
correct runtime host for it on the browser side.

Placement rules:

- `ClientStorageManager` lives with Verso/browser runtime services in
    `AppServices`, alongside `EmbedderCore`.
- It is not owned by `GraphWorkspace` and is not part of Graphshell app-state
    durability (`GraphStore`).
- Servo-facing storage clients obtain bottle or shelf access through the
    manager's bridge API; they do not become the authority for bucket metadata,
    persistence mode, or site-data clearing.
- Graphshell may also host a thin `StorageInteropCoordinator` above the browser
    storage authority for backend-switch policy, Wry compatibility handling, and
    explicit compound actions, but that layer must not become a rival storage
    hierarchy.

This keeps browser-origin storage policy runtime-owned and aligned with the
same authority model used elsewhere in Graphshell: reducer-owned graph truth,
workbench-owned layout truth, and runtime-owned browser services.

See
`../implementation_strategy/subsystem_storage/2026-03-11_graphstore_vs_client_storage_manager_note.md`
and
`../implementation_strategy/subsystem_storage/2026-03-11_client_storage_manager_implementation_plan.md`
for the storage-specific boundary and phased execution plan.

---

## Verso as Bilateral Peer Agent

### Identity

Verso holds the user's bilateral peer identity: an Ed25519 keypair stored in the OS keychain (`keyring` crate). This keypair derives:

- The iroh `NodeId` (via Ed25519 public key → iroh identity).
- The signing key for `SyncUnit` payloads sent to peers.
- The capability attestation key used in `ModManifest`.

The keypair is generated on first Verso initialization and never leaves the keychain. Pairing is done by exchanging public keys (QR code, invite link, or local mDNS discovery) — the private key never moves.

### Sync Architecture

Verso manages a `SyncWorker` — a tokio task that:

1. **Accepts** incoming iroh connections from trusted peers.
2. **Computes deltas**: compares local version vectors with peer-reported vectors to determine which `SyncUnit`s to send.
3. **Applies** received `SyncUnit`s by emitting `GraphIntent` events into the main reducer pipeline. Verso never mutates `GraphWorkspace` directly — it enqueues intents.
4. **Persists** the trust store (peer list, last-seen version vectors, frame access grants) in the frame bundle (redb).

**SyncUnit** is the wire format: a rkyv-serialized, zstd-compressed batch of WAL log entries since the last acknowledged version vector for a given peer. See [verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md](../../verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md) for the full wire format and protocol.

### What Verso Syncs

Verso syncs the **semantic graph** (fjall WAL entries: nodes, edges, tags, metadata). It does not sync:

- Layout state (tile tree, node positions) — these are device-local spatial preferences.
- Renderer runtime state (active webviews, scroll positions) — ephemeral.
- Frame tile-selector semantics overlay — device-local organizational state.

This boundary keeps sync semantically meaningful: peers share *what nodes and edges exist and what they mean*, not *how each device arranges them on screen*.

### Pairing Model

Verso exposes three pairing paths:

1. **QR code**: display a QR encoding the local `NodeId` + invite token; peer scans → mutual trust established.
2. **Invite link**: `verse://pair/{NodeId}/{token}` — shareable URL; clicks open Graphshell and trigger pairing.
3. **mDNS discovery**: on the local network, Verso announces presence and accepts pairing from recognized peers without manual exchange.

Pairing emits `GraphIntent::VersePairDevice { peer_id, public_key, display_name }` which the reducer stores in the trust store.

### Frame Sharing

A frame can be shared with specific peers by granting them access:

- `GraphIntent::VerseShareFrame { frame_name, peer_ids, access_level }`.
- `access_level`: `ReadOnly` (receive sync, cannot send) or `ReadWrite` (bidirectional).
- Access grants are stored in the frame manifest (redb) and enforced by the `SyncWorker` accept loop.

### Bilateral Storage Visibility

Verso reports per-peer storage usage without enforcing limits. This is the
bilateral microcosm of the decentralized storage bank — the simplest case
where n=2 and trust substitutes for bonds and reputation.

```rust
struct PeerStorageReport {
    peer_id: Did,
    bytes_i_hold_for_peer: u64,
    bytes_peer_holds_for_me: u64,
    held_blob_cids: Vec<Cid>,
    last_verified_at_ms: u64,
}
```

Each peer sees how much of their data the other peer is holding, how much of
the other peer's data they are holding, and the imbalance. No enforcement —
peers negotiate informally. Trust handles free-riding at n=2.

The bilateral storage report is intentionally compatible with the community-
scale storage bank structures: a `PeerStorageReport` is a degenerate storage
bank view with n=2 and no credit intermediary. If both peers later join a
community, their bilateral hosting can promote to community-level hosting
without re-placing the data.

See
[2026-03-28_decentralized_storage_bank_spec.md](../../verse_docs/implementation_strategy/2026-03-28_decentralized_storage_bank_spec.md)
§5 for the full bilateral storage budgeting model and its relationship to the
community-scale storage bank.

---

### Relationship to Verse

Verso is the **entry point** into optional networking, but it is not the entirety of the network model.

- Verso owns named-peer, bilateral, iroh-backed behavior.
- Verse owns community-scale, participant-governed, longer-lived shared network behavior.
- A bilateral session can promote into a community or room-backed context, but that promotion is a boundary crossing, not proof that the two layers are the same domain.

This distinction keeps Graphshell local-first and prevents co-op or sync from implicitly redefining the shell as a mandatory social platform.

---

## Verso's ModManifest

```rust
// Registered via inventory::submit! in mods/native/verso/mod.rs
ModManifest {
    id: "verso",
    provides: &[
        "viewer:webview",
        "viewer:wry",           // only if compiled with --features wry
        "protocol:http",
        "protocol:https",
        "protocol:file",
        "protocol:about",
        "protocol:gemini",      // only if compiled with --features gemini
        "identity:p2p",
        "protocol:verse",
        "action:verse.pair_device",
        "action:verse.sync_now",
        "action:verse.share_workspace",
        "action:verse.forget_device",
        "action:gemini.start_server",
        "action:gemini.stop_server",
        "action:gemini.serve_node",
    ],
    requires: &[
        "network",              // capability: network access is needed
    ],
    capabilities: &["network"],
}
```

If Verso is not loaded (or if the `network` capability is denied), `viewer:webview`, `viewer:wry`, `protocol:http/https`, and all Verse actions are simply not registered. Graphshell degrades gracefully to the core viewer set with no web access.

---

## Verso as Gemini Capsule Server

Verso includes a Gemini capsule server (`mods/native/verso/gemini/server.rs`) that serves Graphshell content over the Gemini protocol (TCP port 1965, TLS).

### What it serves

Any content Graphshell can parse into a `SimpleDocument` can be served as `text/gemini` via the reverse transform. Current routes:

| Path | Content |
| --- | --- |
| `/` | Index page — list of all served nodes as Gemini links |
| `/node/{uuid}` | Node content as `text/gemini` |

### Access control

Nodes are registered with an `ArchivePrivacyClass`:

- `LocalPrivate` — never registered; not served
- `OwnDevicesOnly` — served on loopback only (planned)
- `TrustedPeers` — requires Gemini client certificate (planned)
- `PublicPortable` — open, no authentication required

### TLS

A self-signed certificate is generated at startup via `rcgen`. The certificate is ephemeral; TOFU clients re-pin on restart. Persistent cert storage is a follow-on improvement.

### GraphIntent wiring

| Intent | Effect |
| --- | --- |
| `StartGeminiCapsuleServer { port }` | Start the server on the given port (default 1965) |
| `StopGeminiCapsuleServer` | Stop the running server |
| `ServeNodeAsGemini { node_id, title, privacy_class, gemini_content }` | Register a node for serving |
| `UnserveNodeFromGemini { node_id }` | Remove a node from the server |

### Implementation

- `mods/native/verso/gemini/simple_document.rs` — `SimpleDocument` + `SimpleBlock` types; `text/gemini` ↔ `SimpleDocument` serialization
- `mods/native/verso/gemini/server.rs` — `GeminiCapsuleServer`, `CapsuleRegistry`, `GeminiServerHandle`
- Registry integration: `shell/desktop/runtime/registries/mod.rs` — static `GEMINI_REGISTRY` + `GEMINI_SERVER_HANDLE`

See [2026-03-28_gemini_capsule_server_plan.md](../implementation_strategy/2026-03-28_gemini_capsule_server_plan.md) for the implementation plan and status of all three protocol servers.

### Gopher capsule server

`mods/native/verso/gopher/server.rs` — plain TCP (port 70, no TLS). Serves `SimpleDocument` content as Gophermap format. Routes: `/` (root menu), `/node/{uuid}`. Registered via `ServeNodeAsGopher` intent.

### Finger server

`mods/native/verso/finger/server.rs` — plain TCP (port 79, no TLS). Serves named profiles as plain text. Query `""` or any registered `query_name` returns the profile content. Registered via `PublishFingerProfile` intent.

This should be treated as a **legacy compatibility lane**, not the preferred modern public profile transport. It is acceptable for interoperating with old/plaintext profile tooling or for receiving/importing legacy Finger contact info, but modern public discovery should prefer HTTPS-hosted structured mechanisms such as WebFinger.

---

## Verso's Relationship to Graph Authority Domains

Verso touches all three authority domains, but only through the correct boundaries:

| Domain | How Verso interacts |
| ------ | ------------------- |
| **Semantic graph** (`GraphWorkspace`) | Only via `GraphIntent` enqueued from the `SyncWorker` accept loop or from Servo delegate callbacks |
| **Spatial layout** (`Tree<TileKind>`) | Only via `GraphIntent::PromoteNodeToActive` / `DemoteNode*`; never direct tree mutation |
| **Runtime instances** (`AppServices`) | Directly: Verso owns the `EmbedderCore` (Servo) and the `SyncWorker` handle; these live in `AppServices` |

The `GraphSemanticEvent` seam is the exclusive channel from Servo callbacks to graph state. No Servo callback in Verso reaches `GraphWorkspace` directly — every callback terminates in either a `GraphSemanticEvent` enqueue or a `UserInterfaceCommand`. See [2026-02-22_registry_layer_plan.md](../implementation_strategy/2026-02-22_registry_layer_plan.md) Phase 6, Step 6.2 for the contract test enforcing this.

---

## Related Documentation

- [GRAPHSHELL_AS_BROWSER.md](GRAPHSHELL_AS_BROWSER.md) — user-facing browser behavior model
- [VERSE_AS_NETWORK.md](../../verse_docs/technical_architecture/VERSE_AS_NETWORK.md) — the network Verso participates in
- [2026-02-22_registry_layer_plan.md](../implementation_strategy/2026-02-22_registry_layer_plan.md) — registry architecture; Verso's place in the mod system
- [2026-02-23_wry_integration_strategy.md](../implementation_strategy/2026-02-23_wry_integration_strategy.md) — Wry OS webview overlay integration (7-step implementation plan)
- [../implementation_strategy/subsystem_storage/2026-03-11_graphstore_vs_client_storage_manager_note.md](../implementation_strategy/subsystem_storage/2026-03-11_graphstore_vs_client_storage_manager_note.md) — GraphStore vs future `ClientStorageManager` runtime/storage boundary
- [../implementation_strategy/subsystem_storage/2026-03-11_client_storage_manager_implementation_plan.md](../implementation_strategy/subsystem_storage/2026-03-11_client_storage_manager_implementation_plan.md) — phased plan for a Servo-compatible `ClientStorageManager`
- [../../verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md](../../verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md) — Verse Tier 1 sync: iroh transport, identity, pairing, delta sync, SyncWorker
- [../../verse_docs/implementation_strategy/2026-03-28_decentralized_storage_bank_spec.md](../../verse_docs/implementation_strategy/2026-03-28_decentralized_storage_bank_spec.md) — decentralized storage bank: bilateral storage visibility, credit mechanics, placement, durability
