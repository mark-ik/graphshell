# Small Protocol Capsule Servers Plan (Gemini / Gopher / Finger)

**Date**: 2026-03-28
**Status**: In progress
**Location**: `mods/native/verso/gemini/`, `mods/native/verso/gopher/`, `mods/native/verso/finger/`

---

## Plan

### Phase 1: SimpleDocument → text/gemini serializer

- Implement `simple_document_to_gemini(doc: &SimpleDocument) -> String`
- Reverse of §5 mapping in `2026-03-08_simple_document_engine_target_spec.md`
- Lives in the content adaptation pipeline alongside existing producers

### Phase 2: GeminiCapsuleServer (TCP + TLS)

- `rcgen` + `tokio-rustls` deps added to Cargo.toml
- TLS cert derived from Verso's existing Ed25519 keypair via `rcgen`
- TCP listener on configurable port (default 1965)
- Request format: single URL line + CRLF per Gemini protocol
- Runs as a tokio task managed by `SyncWorker` lifecycle

### Phase 3: Content router

Routes incoming `gemini://` requests:

| Path pattern | Content |
| --- | --- |
| `/` | Graph index — list of served nodes as gemini links |
| `/node/{uuid}` | Node metadata + content as `text/gemini` |
| `/graph` | Full graph as navigable capsule (nodes = pages, edges = links) |
| `/export/{capsule_cid}` | SessionCapsule download (gated by `ArchivePrivacyClass`) |

### Phase 4: Access control

- Public routes: open, no client cert required
- `TrustedPeers` routes: require Gemini client certificate; verify against UCAN delegation list
- `LocalPrivate` nodes: not served
- `OwnDevicesOnly`: served only on loopback

### Phase 5: Verso registration + GraphIntent wiring

New `GraphIntent` variants:

```rust
StartGeminiCapsuleServer { port: Option<u16> },
StopGeminiCapsuleServer,
ServeNodeAsGemini { node_id: Uuid, privacy_class: ArchivePrivacyClass },
UnserveNodeFromGemini { node_id: Uuid },
```

New Verso `ModManifest` entries:

- `"action:gemini.start_server"`
- `"action:gemini.stop_server"`
- `"action:gemini.serve_node"`

---

## Findings

- `rustls 0.23` already in Cargo.toml; `tokio-rustls` and `rcgen` are not — both needed
- No existing Gemini client implementation in verso yet (only a placeholder in `ProtocolRegistry`)
- `GeminiResolver` (client) and `GeminiCapsuleServer` (server) are independent; server does not depend on client being implemented first
- Ed25519 keypair already managed by Verso identity layer (`keyring` crate); cert derivation via `rcgen::KeyPair::from_der` with the existing key material is the right path
- `SimpleDocument` → `text/gemini` is a clean reversible transform; all block types have a 1:1 line-prefix representation

---

## Progress

### Session 2026-03-28

- Plan doc created
- `rcgen` + `tokio-rustls` added to Cargo.toml
- `mods/native/verso/gemini/simple_document.rs` — `SimpleDocument`, `SimpleBlock`, `from_gemini()`, `to_gemini()`; 5 unit tests pass
- `mods/native/verso/gemini/server.rs` — `GeminiCapsuleServer`, `CapsuleRegistry`, `GeminiServerHandle`, TLS setup, content router (`/` index, `/node/{uuid}`)
- `GraphIntent` variants added: `StartGeminiCapsuleServer`, `StopGeminiCapsuleServer`, `ServeNodeAsGemini`, `UnserveNodeFromGemini`
- Wired in `handle_runtime_lifecycle_intent` via `start/stop/register/unregister_gemini_*` functions in `shell/desktop/runtime/registries/mod.rs`
- Verso `ModManifest` updated: `action:gemini.start_server`, `action:gemini.stop_server`, `action:gemini.serve_node`
- `VERSO_AS_PEER.md` updated with Gemini capsule server section
- `cargo check` clean; all 5 `simple_document` tests pass

- `mods/native/verso/gopher/server.rs` — `GopherCapsuleServer`, `GopherRegistry`, `GopherServerHandle`; plain TCP; Gophermap format; `/` root menu + `/node/{uuid}` routes
- `mods/native/verso/finger/server.rs` — `FingerServer`, `FingerRegistry`, `FingerServerHandle`; plain TCP; named profile queries; default profile fallback; legacy compatibility lane rather than preferred modern publication target
- `SimpleDocument` → Gophermap (`to_gophermap()`) and plain text (`to_finger_text()`) serializers added to `simple_document.rs`; 4 new tests (9 total pass)
- `GraphIntent` variants added: `StartGopherCapsuleServer`, `StopGopherCapsuleServer`, `ServeNodeAsGopher`, `UnserveNodeFromGopher`, `StartFingerServer`, `StopFingerServer`, `PublishFingerProfile`, `UnpublishFingerProfile`
- All wired in `registries/mod.rs` via static `GOPHER_REGISTRY`/`FINGER_REGISTRY` + server handles
- Verso `ModManifest` updated with 6 new `action:gopher.*` / `action:finger.*` entries
- `VERSO_AS_PEER.md` updated with Gopher and Finger sections

**Follow-on improvements (not in this slice):**

- Persistent TLS cert for Gemini (currently ephemeral — TOFU clients re-pin on restart)
- Client certificate enforcement for `TrustedPeers` routes (Gemini)
- Loopback-only binding for `OwnDevicesOnly` nodes (all three protocols)
- `/graph` route for Gemini and Gopher (full graph as navigable capsule)
- `/export/{capsule_cid}` route (SessionCapsule download)
- Profile sharing UI: `CapsuleProfile` publication mapping derived from the social profile surface, optionally associated with a `GraphshellProfile`; `ServeProfileOnAllProtocols` intent
- WebFinger replacement lane: if modern public profile discovery is prioritized, add an HTTPS-hosted WebFinger document rather than expanding Finger further
