# VERSO AS PEER

**Purpose**: Specification for Verso — Graphshell's web capability mod and Verse peer agent. Verso is the bridge between the user's graph and two things: the open web (as a browser engine peer), and the Verse network (as a sync and sharing peer).

**Document Type**: Behavior and role specification
**Status**: Registry integration Phase 2–3 complete (Servo + ViewerRegistry); Verse Tier 1 (iroh sync) in Phase 5
**See**: [GRAPHSHELL_AS_BROWSER.md](GRAPHSHELL_AS_BROWSER.md) for the user-facing browser model; [VERSE_AS_NETWORK.md](../../verse_docs/technical_architecture/VERSE_AS_NETWORK.md) for the network protocol Verso participates in

---

## What Verso Is

Verso is a **native mod** — compiled into the Graphshell binary, registered at startup via `inventory::submit!`, not sandboxed. It is the single "Browser Capability" mod. It does two things:

1. **Web Peer**: brings Servo (the browser engine) and the `wry` OS webview fallback into the registry as `viewer:webview` and `viewer:wry`. These are how Graphshell renders the web.
2. **Verse Peer**: manages Graphshell's identity, pairing, and sync participation on the Verse network. Verso is the local agent that holds the user's keys, establishes iroh connections, and exchanges `SyncUnit` deltas with trusted peers.

These two roles are unified in Verso because they share the same trust infrastructure: the keypair that signs Verso's Verse sync payloads is the same identity that signs the mod's capabilities. A user who enables Verso gets both web access and Verse participation from a single, coherent module.

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

---

## Verso as Verse Peer

### Identity

Verso holds the user's Verse identity: an Ed25519 keypair stored in the OS keychain (`keyring` crate). This keypair derives:

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
    ],
    requires: &[
        "network",              // capability: network access is needed
    ],
    capabilities: &["network"],
}
```

If Verso is not loaded (or if the `network` capability is denied), `viewer:webview`, `viewer:wry`, `protocol:http/https`, and all Verse actions are simply not registered. Graphshell degrades gracefully to the core viewer set with no web access.

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
- [../../verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md](../../verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md) — Verse Tier 1 sync: iroh transport, identity, pairing, delta sync, SyncWorker
