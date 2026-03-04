# Standards Alignment Report

**Date**: 2026-03-04
**Status**: Active / Canonical
**Purpose**: Identify applicable external standards for each Graphshell domain, resolve contradictions between them, and establish the harmonious adoption set that structures how Graphshell components are specified, implemented, and validated.

**Policy statement**: Graphshell components must be specifiable, implementable, and validatable against a coherent set of external standards. Each standard is either **formally adopted** (the codebase implements to it, specs cite it, tests can validate against it) or **referenced as prior art** (informs design vocabulary and mental models; no conformance obligation). This distinction must be maintained in all subsystem and register specs.

**Related docs**:
- [system_architecture_spec.md](../implementation_strategy/system/system_architecture_spec.md)
- [SUBSYSTEM_STORAGE.md](../implementation_strategy/subsystem_storage/SUBSYSTEM_STORAGE.md)
- [SUBSYSTEM_SECURITY.md](../implementation_strategy/subsystem_security/SUBSYSTEM_SECURITY.md)
- [SUBSYSTEM_DIAGNOSTICS.md](../implementation_strategy/subsystem_diagnostics/SUBSYSTEM_DIAGNOSTICS.md)
- [VERSE_AS_NETWORK.md](../../verse_docs/technical_architecture/VERSE_AS_NETWORK.md)
- [verseblob_content_addressing_spec.md](../../verse_docs/implementation_strategy/verseblob_content_addressing_spec.md)
- [subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md](../implementation_strategy/subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md)


---

## 1. Structuring Policy

> **Harmonious standards adoption is the structuring policy for Graphshell development.** Every subsystem, aspect, and registry that names a behavior must cite the standard(s) that govern that behavior if such a standard exists. Conformance to a reputable standard is the primary validation target — not adherence to an internal spec alone. Internal specs translate standards into Graphshell-specific contracts.

Rules:

1. **Cite, don't invent**: If an external standard defines a behavior Graphshell implements, cite it. Do not re-derive the behavior from first principles when a standard exists.
2. **Formally adopt or explicitly not**: Every standard in this report is either adopted (with obligations) or referenced (without obligations). No implicit middle ground.
3. **Contradictions are resolved here**: When two standards conflict in the same domain, the resolution is recorded here and binding on all child specs.
4. **Conformance validates implementation**: The accepted criteria for a subsystem's done-definition must include demonstrable conformance to its adopted standards, not just internal contract tests.
5. **Standards evolve**: When a standard is revised, this report is updated and the impact on adoption obligations assessed.

---

## 2. Domain Map

| Domain | Adopted standards | Referenced as prior art |
|--------|-------------------|------------------------|
| URI schemes (internal) | RFC 3986 | RFC 7595 |
| Node identity | RFC 4122 UUID v4 | POSIX inode semantics |
| WAL / operation sequencing | RFC 4122 UUID v7 (operation tokens only) | OAIS ISO 14721 |
| Persistence / storage paths | XDG Base Dir (via `directories` crate) | — |
| Accessibility | WCAG 2.2 Level AA | EN 301 549, WAI-ARIA 1.3 APG |
| Diagnostics / observability | OpenTelemetry Semantic Conventions | RFC 5424 Syslog |
| Registry layer | OSGi R8 Service Registry (conceptual) | — |
| Physics layout | Fruchterman-Reingold 1991 | — |
| UX information architecture | Shneiderman's VISM | — |
| Tier 1 P2P transport (Verso / bilateral) | libp2p specs (via iroh) | — |
| Tier 2 P2P transport (Verse community) | libp2p specs (GossipSub) | — |
| Verse peer identity | W3C DID Core 1.0 (`did:key`) | — |
| Verse knowledge objects | W3C VC Data Model 2.0 | ActivityPub |
| Verse content addressing | IPFS CIDv1 (via `cid` crate) | — |
| Verse concurrent sync | CRDT algorithms (Automerge/Yrs) | RFC 6902 JSON Patch |
| Transport encryption | Noise Protocol (via iroh QUIC) | — |
| At-rest encryption | FIPS 197 AES-256, NIST SP 800-38D (GCM) | — |
| WASM sandboxing | WASI Preview 1 (via extism) | — |

---

## 3. Formal Adoption Details

### 3.1 URI Schemes — RFC 3986

**Adopted**: yes.

RFC 3986 (Uniform Resource Identifier: Generic Syntax) governs all internal URI schemes:
`verso://`, `notes://`, `graph://`, `node://`, and legacy `graphshell://`.

Requirements:
- All internal URIs must be syntactically valid per RFC 3986 §3 (scheme, authority, path, query, fragment).
- `verso://` is an application-private scheme. No IANA registration is required or planned (RFC 7595 is therefore reference-only).
- Scheme parsing in `parser.rs` must produce results that pass RFC 3986 structural validation.
- `verso://view/<kind>/<id>` path segmentation follows RFC 3986 §3.3 hierarchical path rules.

Note: RFC 7595 (scheme registration guidelines) is **not adopted** — it only becomes relevant if `verso://` is ever submitted for IANA registration, which is not planned given its internal-only semantics.

### 3.2 Node Identity — RFC 4122 UUID v4

**Adopted**: yes.

Each graph node has a stable UUID v4 identity (`NodeId`). UUID v4 is random, universally unique, and has no ordering implication. This property is deliberate: node creation order is not encoded in node identity. URL is mutable metadata, not identity (see POSIX inode analogy in prior-art §5.1).

Requirements:
- `NodeId` is generated with `uuid::Uuid::new_v4()`. No other UUID version may be used for node identity.
- `NodeId` must be stable across sessions: serialize to bytes, persist, restore, and the same UUID reappears.
- Node identity is never derived from URL, title, or position.

### 3.3 WAL Operation Sequencing — RFC 4122 UUID v7

**Adopted**: yes, for WAL/undo-stack operation tokens only.

UUID v7 is time-ordered (timestamp-prefixed). This property makes it suitable for WAL journal entries and undo-stack operation identifiers, where chronological ordering without a counter is useful.

Critical policy: UUID version is **never** used to infer node creation order or distinguish node identity from operation identity. The two namespaces are:

| Namespace | UUID version | Purpose |
|-----------|-------------|---------|
| `NodeId` | v4 | Stable node identity — no ordering semantics |
| Operation / WAL entry token | v7 | Time-ordered journal entry identifier |

Mixing these namespaces is a correctness error. Serialize/deserialize code must not treat a WAL entry's v7 UUID as a `NodeId` or vice versa.

### 3.4 Storage Paths — XDG Base Directory Specification

**Adopted**: naming conventions and semantics only, resolved per-platform via the `directories` crate.

XDG defines `$XDG_DATA_HOME`, `$XDG_CONFIG_HOME`, `$XDG_CACHE_HOME`, `$XDG_STATE_HOME`. These semantics are adopted as the canonical vocabulary for storage path categories regardless of platform:

| XDG name | Windows equivalent | macOS equivalent | Graphshell use |
|----------|--------------------|-----------------|----------------|
| `XDG_DATA_HOME` | `%APPDATA%\Graphshell` | `~/Library/Application Support/Graphshell` | fjall WAL, redb snapshots |
| `XDG_CONFIG_HOME` | `%APPDATA%\Graphshell\config` | `~/Library/Application Support/Graphshell/config` | `AppPreferences`, user registries |
| `XDG_CACHE_HOME` | `%LOCALAPPDATA%\Graphshell\cache` | `~/Library/Caches/Graphshell` | Thumbnails, favicons, temporary blobs |
| `XDG_STATE_HOME` | `%LOCALAPPDATA%\Graphshell\state` | `~/Library/Application Support/Graphshell/state` | Session state, tile layout |

Requirements:
- Path resolution uses the `directories` crate (`ProjectDirs::from("", "", "Graphshell")`), not hard-coded paths.
- No path may be hard-coded relative to the binary or CWD.
- XDG naming is used in documentation and diagnostics channel names even on non-Linux platforms.

### 3.5 Accessibility — WCAG 2.2 Level AA

**Adopted**: yes.

WCAG 2.2 Level AA is the single accessibility conformance target. EN 301 549 conformance follows as a consequence for any EU distribution (see §4.1 — EN 301 549 is not a separate conformance obligation).

Adopted standard is WCAG 2.2 (not 2.1). Key criteria added in 2.2 that are relevant to Graphshell:
- **2.4.11 Focus Not Obscured (Minimum)** — focus indicator must not be fully hidden by sticky content (relevant to omnibar / radial menu floating over canvas)
- **2.4.12 Focus Not Obscured (Enhanced)** — AA: focus not fully obscured (tighter); relevant to tile-tree pane chrome
- **2.5.3 Label in Name** — visible label text must be contained in the accessible name (relevant to all button/action labels in command palette, radial menu, omnibar)
- **2.5.7 Dragging Movements** — alternatives must exist for all drag operations (relevant to graph node repositioning and lasso)
- **2.5.8 Target Size (Minimum)** — 24×24 CSS px minimum (relevant to node interaction hit targets)
- **3.2.6 Consistent Help** — help affordance in consistent location (relevant to workbar `F1` help)

Implementation path: `accesskit` crate for platform accessibility tree (AT-SPI2 on Linux, UIA on Windows, AX on macOS). ARIA APG widget patterns inform the semantic labels assigned to `accesskit::NodeBuilder` instances (see prior art §5.2).

The seven surface classes requiring Level AA evidence (per AG0):
1. Graph Pane
2. Node Pane
3. Tool Pane
4. Radial Menu
5. Command Palette
6. Omnibar
7. Settings

### 3.6 Diagnostics Naming — OpenTelemetry Semantic Conventions

**Adopted**: yes.

OpenTelemetry Semantic Conventions are adopted for diagnostic channel naming, severity levels, and attribute naming conventions. The existing `ChannelSeverity::Error / Warn / Info` enum is already aligned with OTel's `SeverityText` values.

Requirements:
- Channel names follow OTel attribute naming: `namespace.sub.operation` (e.g., `persistence.journal.entry_written`).
- Severity levels map to OTel `SeverityNumber` ranges: Error (17–20), Warn (13–16), Info (9–12). Do not use RFC 5424 integer severity codes in channel descriptors.
- Attribute keys in diagnostic event payloads follow OTel naming conventions where applicable (`url.full`, `error.type`, `peer.id`, etc.).
- RFC 5424 is reference-only and only relevant if a syslog export sink is ever added.

### 3.7 Registry Layer — OSGi R8 Service Registry (Conceptual)

**Adopted**: conceptual model and vocabulary. Not a wire-format or API conformance target.

OSGi R8 Core §3 (Service Layer) defines: service registration with capability declarations, service lookup by interface/filter, service lifecycle (registered → in-use → unregistered), and versioned bundles. Graphshell's registry layer implements these concepts in Rust:

| OSGi concept | Graphshell equivalent |
|-------------|----------------------|
| Service | Registry entry (viewer, protocol, action, theme, ...) |
| Bundle | Mod (`ModManifest`) |
| Capability | Declared `provides` / `requires` in `ModManifest` |
| Service Registry | `RegistryRuntime` |
| `namespace:name` filter | Registry key convention (`namespace:name`) |
| Service lifecycle | Mod activation / deactivation via `ModLoader` |

Requirements:
- Registry keys follow `namespace:name` format at all times.
- New registry types define their capability interface (what a provider must declare) explicitly.
- Registry entries declare their compatibility version so mods can express minimum requirements.

### 3.8 Physics Layout — Fruchterman-Reingold 1991

**Adopted**: algorithm authority. Cite in code and docs; no external conformance artifact.

Source: Fruchterman, T.M.J. & Reingold, E.M. (1991). "Graph drawing by force-directed placement." *Software: Practice and Experience*, 21(11), 1129–1164.

All physics tuning parameters (repulsion, attraction, damping, temperature decay) must be documented against their corresponding variables in the F-R formulation. Deviations from the algorithm (e.g., the egui_graphs `FruchtermanReingoldState` implementation) must be noted.

### 3.9 UX Architecture — Shneiderman's Visual Information Seeking Mantra

**Referenced**: yes. Not a conformance target — a design heuristic with literature weight.

"Overview first, zoom and filter, then details on demand." (Shneiderman, 1996)

This maps directly to the Graphshell tile-tree + canvas + detail-pane model:
- **Overview first**: the graph canvas with LOD rendering is the overview
- **Zoom and filter**: graph camera + lasso zones + scope model is the filter layer
- **Details on demand**: Node Pane / Tool Pane expansion is the detail layer

Cite in canvas design docs and LOD rendering decisions.

### 3.10 Tier 1 P2P Transport — libp2p specs via iroh

**Adopted**: the libp2p protocol specs as implemented by iroh. iroh is the implementation vehicle; the libp2p specs are the normative authority for protocol behavior.

Relevant specs:
- **QUIC transport** ([libp2p QUIC spec](https://github.com/libp2p/specs/tree/master/quic)) — primary transport for iroh bilateral sync
- **Noise Protocol** ([libp2p Noise spec](https://github.com/libp2p/specs/tree/master/noise)) — transport-layer encryption and peer authentication
- **Identify protocol** ([libp2p identify](https://github.com/libp2p/specs/tree/master/identify)) — peer capability announcement
- **PeerID** ([libp2p peer IDs](https://github.com/libp2p/specs/blob/master/peer-ids/peer-ids.md)) — Ed25519 public key as peer identity

Tier 2 additionally uses:
- **GossipSub 1.1** ([libp2p gossipsub](https://github.com/libp2p/specs/tree/master/pubsub/gossipsub)) — community-layer pubsub for blob announcements and community metadata

### 3.11 Verse Peer Identity — W3C DID Core 1.0

**Adopted**: yes, for Verse peer identity representation.

Each Graphshell peer's Ed25519 keypair is representable as a `did:key` DID per W3C DID Core 1.0. This provides a standard URI form for peer identity usable in Verifiable Credentials without requiring a server or blockchain.

Format: `did:key:z6Mk...` (base58btc-multibase-encoded Ed25519 public key)

Requirements:
- Peer identity exported to Verse wire formats uses `did:key` DID representation.
- iroh `NodeId` (the raw Ed25519 public key hash) is the internal representation; `did:key` is the Verse-layer identity URI.
- `did:key` resolution is deterministic and offline (no network lookup required).

Note: this does **not** require a DID resolver or blockchain. `did:key` is a self-describing format resolved locally.

### 3.12 Verse Knowledge Objects — W3C VC Data Model 2.0

**Adopted**: yes, for Verse Tier 2 knowledge object envelopes.

W3C Verifiable Credentials Data Model 2.0 provides a standard envelope for cryptographically attested knowledge objects. This replaces ActivityPub as the Verse knowledge vocabulary (see §4.2 for the ActivityPub conflict resolution).

Mapping:
- A Verse report or graph snapshot is a `VerifiableCredential` with the authoring peer as `issuer` (expressed as a `did:key` DID).
- The `credentialSubject` is the content being attested (a graph slice, an engram, a search index segment).
- The `proof` field carries the Ed25519 signature over the credential, verifiable against the issuer's `did:key`.
- VerseBlob CID appears as the credential's `id` field.

This composes with iroh: a `VerseBlob` can carry a Verifiable Credential payload with its CID as the canonical identifier.

### 3.13 Verse Content Addressing — IPFS CIDv1

**Adopted**: yes.

`VerseBlob` uses CIDv1 with base32 canonical text representation (already specified in `verseblob_content_addressing_spec.md §2`). This is confirmed as the adopted standard.

Requirements:
- All content-addressed blobs use CIDv1 (not CIDv0 / base58btc bare multihash).
- Codec field follows the IPFS codec table (dag-cbor for structured data, raw for opaque bytes).
- CID computation uses BLAKE3 as the hash function (iroh-native; faster than SHA2-256 for this use case).

### 3.14 Verse Concurrent Sync — CRDT Algorithms

**Adopted**: CRDT semantics as the concurrency model for Verse graph sync. Specific algorithm implementation TBD at design time for Tier 2.

RFC 6902 JSON Patch is **not adopted** for sync diffs: it has no merge semantics and requires stable structural paths that contradict the NodeIndex instability in petgraph (see §4.3).

CRDT requirements:
- Graph node sets use OR-Set (Observed-Remove Set) semantics: adds and removes are tracked as operations, and concurrent adds from different peers merge additively.
- Edge sets use the same OR-Set model.
- Node metadata (title, URL, lifecycle) uses Last-Write-Wins per field, with timestamp from UUID v7 operation tokens.
- The CRDT operation log must be compatible with the existing fjall WAL model (append-only, UUID v7 keyed entries).

### 3.15 Transport Encryption — Noise Protocol

**Adopted**: Noise Protocol Framework (via iroh QUIC). Already implemented in `SyncWorker` (§3.1 in `SUBSYSTEM_SECURITY.md`).

All Verse connections use Noise handshake (XX pattern) over QUIC. No plaintext transport path exists.

### 3.16 At-Rest Encryption — FIPS 197 AES-256 + NIST SP 800-38D (GCM)

**Adopted**: AES-256-GCM as specified in FIPS 197 and NIST SP 800-38D. Already implemented in `GraphStore` (§3.5 in `SUBSYSTEM_STORAGE.md`).

Requirements (as already specified in storage subsystem §3.4/3.5):
- Key size: 256 bits (AES-256).
- Mode: GCM (NIST SP 800-38D). Provides both confidentiality and integrity.
- Nonce: 12 bytes, generated with `OsRng` per encryption call. Never reused.
- Authentication tag: 16 bytes. Verification failure produces explicit error.

### 3.17 WASM Sandboxing — WASI Preview 1

**Adopted**: WASI Preview 1 capability model (via extism). Mods must declare capabilities; undeclared capabilities are denied.

---

## 4. Conflict Resolutions

### 4.1 EN 301 549 vs. WCAG 2.2

**Resolution**: WCAG 2.2 Level AA is the single normative accessibility target. EN 301 549 conformance follows as a consequence for any EU distribution; no separate checklist is maintained.

Rationale: EN 301 549 currently references WCAG 2.1 AA in its harmonized standard. WCAG 2.2 AA is a strict superset of WCAG 2.1 AA plus additional criteria (§3.5 above). Targeting WCAG 2.2 AA satisfies EN 301 549's WCAG-derived requirements and adds future-proofing.

### 4.2 ActivityPub vs. W3C VC Data Model + DID Core

**Resolution**: W3C VC Data Model 2.0 + W3C DID Core 1.0 replace ActivityPub for Verse knowledge object representation.

Rationale: ActivityPub requires `https://` actor URIs and HTTP inbox/outbox endpoints, which imply a DNS-backed server. Verse peers are identified by Ed25519 keypairs, not DNS hostnames. The W3C VC + `did:key` combination provides:
- Cryptographic attestation (which ActivityPub lacks)
- Self-describing peer identity without a server (`did:key`)
- No network lookup required for identity resolution

ActivityPub **interoperability** remains a long-horizon consideration (Verse content surfaced to ActivityPub feeds via a bridge server is architecturally possible), but ActivityPub is not the backbone of Verse and is not adopted as an internal standard.

AT Protocol (`atproto`) is similarly referenced as prior art for federated browsing identity patterns, but its PDS (Personal Data Server) model conflicts with the serverless Verso peer model.

### 4.3 RFC 6902 JSON Patch vs. CRDT algorithms

**Resolution**: CRDT algorithms (§3.14) are adopted for Verse sync. RFC 6902 is not adopted.

Rationale: RFC 6902 JSON Patch has no merge semantics and requires stable structural JSON paths. `petgraph::NodeIndex` is explicitly not stable across serialize/deserialize cycles (confirmed in `ARCHITECTURAL_OVERVIEW.md`). UUID-keyed paths (`/nodes/{uuid}/title`) would technically work at the exchange layer, but RFC 6902 still cannot handle concurrent edits from multiple peers — which Verse Tier 2 requires. CRDTs provide deterministic convergence without central coordination.

### 4.4 UUID v4 vs. UUID v7 (namespace conflict)

**Resolution**: Split by namespace (§3.2 and §3.3). v4 for node identity; v7 for WAL/operation tokens only. Code that conflates the two namespaces is a correctness error.

### 4.5 rkyv (zero-copy binary) vs. schema-evolution requirements

**Resolution**: rkyv is adopted for local same-version persistence (WAL journal entries, snapshots). It is **not** adopted for Verse wire format or cross-version schema evolution.

Verse wire format uses `VerseBlob` with dag-cbor codec (IPFS CIDv1 §3.13), which supports schema evolution via CBOR's self-describing field encoding. This cleanly separates:
- **Local persistence** (rkyv): zero-copy performance, same binary, same struct layout
- **Verse wire format** (dag-cbor / VerseBlob): schema-versioned, cross-peer compatible

A schema migration path for the local rkyv format will be needed at v1.0 (first public release with real users). Until then, DOC_POLICY.md §3 (no legacy friction) applies.

### 4.6 RFC 5424 Syslog vs. OpenTelemetry Semantic Conventions

**Resolution**: OpenTelemetry Semantic Conventions adopted for all diagnostics. RFC 5424 is reference-only and only relevant if a syslog export sink is added.

Rationale: RFC 5424 uses integer severity 0–7 with a facility code. OTel uses string-valued severity with a 1–24 numeric range. The existing `ChannelSeverity::Error / Warn / Info` enum maps directly to OTel `SeverityText` and does not map cleanly to RFC 5424. OTel is the more appropriate target for a structured observability pipeline and potential future export.

### 4.7 iroh (Tier 1) vs. libp2p community swarm (Tier 2)

**Non-conflict**: iroh is a Rust implementation of a subset of libp2p specs. It is the Tier 1 implementation vehicle. Tier 2 uses the same libp2p protocol specs (GossipSub 1.1, Kademlia DHT) through a broader libp2p implementation. The standards are the same; the implementation libraries may differ between tiers.

There is no conflict between using iroh for Tier 1 bilateral sync and libp2p GossipSub for Tier 2 community swarms — they speak compatible protocols.

### 4.8 POSIX filesystem POSIX compliance — clarification

**Clarification (not a conflict)**: The file tree subsystem (graph nodes that represent filesystem paths, the planned filesystem ingest mapping) does not need to be "POSIX-compliant" in the sense of implementing the POSIX filesystem API. The relevant POSIX concept is the **inode identity model** (inode number is stable; filename/path is mutable metadata pointing to it), which is adopted as prior art in §5.1 to explain why `NodeId` is decoupled from URL/path.

If Graphshell exposes a FUSE-based filesystem mount (not currently planned), POSIX compliance in the API sense would become relevant at that point.

---

## 5. Prior Art References (No Conformance Obligation)

### 5.1 POSIX Inode Semantics

Stable identity (inode number) decoupled from mutable name/path. Directly analogous to `NodeId` (UUID v4, stable) decoupled from `url` (mutable metadata). Cite in node identity and lifecycle documentation.

### 5.2 WAI-ARIA 1.3 + ARIA Authoring Practices Guide

ARIA APG widget patterns (Combobox, Listbox, Menu, Dialog, Toolbar, Tree) inform the semantic labels assigned to `accesskit::NodeBuilder` instances for each Graphshell widget. No independent conformance target — all ARIA requirements flow through WCAG 2.2 AA SC 4.1.2 (Name, Role, Value).

Widget role mapping:
- Omnibar → `combobox` (with `listbox` popup for search results)
- Command palette → `dialog` containing `listbox`
- Radial menu → `menu` with `menuitem` sectors
- Graph pane → `application` region with `tree` or `grid` for node navigation
- Workbar → `toolbar`
- Settings pane → `dialog` or named region

### 5.3 OAIS (ISO 14721) — Open Archival Information System

SIP/AIP/DIP vocabulary informs graph export and import design. Graphshell's export path (JSON export, interactive HTML export) maps conceptually to a DIP (Dissemination Information Package). Cite in export design docs.

Not adopted as a formal conformance target — the OAIS standard is designed for institutional archives, and its fixity-audit and format-migration obligations are disproportionate for a prototype browser.

### 5.4 ActivityPub / AT Protocol

Reference for federated identity and content patterns. Both protocols illustrate trade-offs between server-mediated federation (ActivityPub) and personal data servers (AT Protocol). Neither is adopted; both inform the Verse interoperability design space. See §4.2.

### 5.5 RFC 6902 JSON Patch

Reference for understanding operational diff formats. Not adopted (§4.3). If Verse requires a patch-based exchange format for specific narrow use cases (e.g., metadata-only updates where structural stability holds), JSON Patch semantics may be reconsidered for that scope.

### 5.6 GoF Command Pattern

The existing `GraphReducerIntent` / `apply_reducer_intents()` architecture already instantiates the Command Pattern. `UndoRedoSnapshot` instantiates the Memento Pattern. Cite as documentation vocabulary in architecture docs; no adoption obligation.

### 5.7 Shneiderman's Visual Information Seeking Mantra

See §3.9. Cite in canvas design docs and LOD rendering decisions.

### 5.8 Filecoin FIP Mechanics

Relevant to FLora staking design in Verse Tier 2. Filecoin FVM and FIP-0045 (actor upgradability) are the relevant specs for on-chain staking and storage deal mechanics. The Filecoin economic layer is separate from the iroh/libp2p transport layer and requires a Filecoin light client integration decision that is explicitly out of Verse Tier 1 scope.

### 5.9 RFC 7595 URI Scheme Registration

Relevant only if `verso://` is ever submitted for IANA registration. Not planned. Do not adopt.

### 5.10 EN 301 549

See §4.1. Consequences flow from WCAG 2.2 AA; no separate conformance checklist.

---

## 6. What Needs Evaluation (Not Yet Decided)

### 6.1 FlatBuffers / Cap'n Proto for schema-versioned local persistence

rkyv is zero-copy and fast, but opaque across versions and binary layouts. If the local WAL schema evolves significantly before v1.0 (when real-user migration requirements exist), a schema-versioned binary format (FlatBuffers or Cap'n Proto) at the WAL boundary would allow safe migration without full replay. Evaluate before designing the v1.0 schema migration path.

Decision gate: Does the WAL schema need to evolve before the first public release? If yes, evaluate. If no, rkyv + snapshot-and-rebuild suffices until then.

### 6.2 Specific CRDT library for Verse Tier 2

`automerge-rs` and `yrs` are the two leading Rust CRDT implementations. The choice affects the wire format for Verse sync operations. Evaluate at Tier 2 design time (currently long-horizon, Q3 2026+).

---

## 7. Summary: Adoption Register

| Standard | Adopted? | Scope | Cites in |
|----------|----------|-------|---------|
| RFC 3986 | Yes | URI syntax for all internal schemes | `parser.rs`, SUBSYSTEM_STORAGE.md, TERMINOLOGY.md |
| RFC 4122 UUID v4 | Yes | Node identity | `graph/mod.rs`, node lifecycle spec |
| RFC 4122 UUID v7 | Yes (operation tokens only) | WAL entry sequencing | `services/persistence/mod.rs`, undo stack design |
| XDG Base Dir | Yes (via `directories`) | Storage path semantics | SUBSYSTEM_STORAGE.md, prefs.rs |
| WCAG 2.2 Level AA | Yes | All 7 surface classes | SUBSYSTEM_ACCESSIBILITY.md, all design docs |
| OTel Semantic Conventions | Yes | Diagnostics channel naming + severity | SUBSYSTEM_DIAGNOSTICS.md, all `*.diagnostics.*` channels |
| OSGi R8 (conceptual) | Yes | Registry vocabulary + lifecycle | system_architecture_spec.md, register_layer_spec.md |
| Fruchterman-Reingold 1991 | Yes (algorithm authority) | Physics engine | `registries/atomic/physics_profile.rs`, layout docs |
| libp2p specs (via iroh) | Yes | Tier 1 P2P transport | SUBSYSTEM_SECURITY.md, VERSE_AS_NETWORK.md |
| libp2p GossipSub 1.1 | Yes | Tier 2 community layer | VERSE_AS_NETWORK.md, verseblob spec |
| W3C DID Core 1.0 | Yes | Verse peer identity | SUBSYSTEM_SECURITY.md, verseblob spec |
| W3C VC Data Model 2.0 | Yes | Verse knowledge objects | verseblob spec, engram spec |
| IPFS CIDv1 | Yes | Verse content addressing | verseblob_content_addressing_spec.md |
| CRDT algorithms | Yes (semantics) | Verse concurrent sync | VERSE_AS_NETWORK.md |
| Noise Protocol | Yes | Transport encryption | SUBSYSTEM_SECURITY.md |
| FIPS 197 / NIST SP 800-38D | Yes | At-rest encryption (AES-256-GCM) | SUBSYSTEM_STORAGE.md, SUBSYSTEM_SECURITY.md |
| WASI Preview 1 | Yes | WASM mod sandboxing | SUBSYSTEM_MODS.md, mod_loader.rs |
| Shneiderman VISM | Referenced | UX design heuristic | canvas design docs |
| POSIX inode semantics | Referenced | Node identity analogy | node lifecycle docs |
| WAI-ARIA 1.3 APG | Referenced | accesskit role vocabulary | SUBSYSTEM_ACCESSIBILITY.md |
| OAIS ISO 14721 | Referenced | Export design vocabulary | export spec |
| ActivityPub | Referenced | Interop design space | VERSE_AS_NETWORK.md |
| AT Protocol | Referenced | Federated identity patterns | VERSE_AS_NETWORK.md |
| RFC 6902 JSON Patch | Referenced (not adopted) | Diff format prior art | — |
| RFC 5424 Syslog | Referenced (not adopted) | Only if syslog sink added | — |
| EN 301 549 | Referenced (not adopted) | WCAG 2.2 AA consequence | — |
| GoF Command Pattern | Referenced | Architecture vocabulary | ARCHITECTURAL_OVERVIEW.md |
| Filecoin FIPs | Referenced | FLora staking (Tier 2) | VERSE_AS_NETWORK.md |
| FlatBuffers / Cap'n Proto | Under evaluation | Schema-versioned WAL | — |
| CRDT library choice | Under evaluation | Verse Tier 2 sync | — |
| RFC 7595 | Not adopted | — | — |
