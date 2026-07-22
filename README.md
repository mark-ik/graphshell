# Graphshell

Graphshell is a remote, local-first projection host. Applications retain their
own truth and authority; Graphshell stores only curation state, disclosed
scenes, and presentation preferences.

## Current boundary

The workspace is intentionally portable:

- `graphshell-protocol` carries versioned score, scene, status, and intent
  messages over an unspecified carrier.
- `graphshell-client` keeps endpoint-scoped scene snapshots and curation-local
  state.
- `graphshell-endpoint` defines injected projection and intent traits for
  applications to implement beside their own truth.
- `graphshell` is the presentation host. Its first native semantic view is the
  deterministic G1 loopback receipt; Genet/Cambium composition remains later
  application work.

The portable crates may depend on Scenograph contracts, serialization, and
content-addressing primitives. They must not depend on Mere, Merecat, Isometry,
Genet, Cambium, NetRender, a network runtime, or an application model. Product
adapters depend on `graphshell-endpoint` in the other direction.

## G1 loopback proof

G1 keeps presentation outside `sceno::Scene`. A snapshot carries a Graphshell
sidecar manifest that binds scene instances to ordered, versioned resource
offers. Resource bytes are fetched separately, verified by content hash, and
cached within the disclosing session.

The deterministic fixture proves two capability profiles over one scene:

- rich: portable card plus content-addressed image;
- compact: native glyph plus a labeled image placeholder;
- both: the same advertised actions in the accessibility projection.

Run the proof wall:

```powershell
$env:CARGO_TARGET_DIR = 'target-proof'
cargo test --workspace
cargo check --workspace --target wasm32-unknown-unknown
cargo run -p graphshell --bin g1_receipt -- docs/receipts/g1_loopback.html
```

The committed [G1 receipt](docs/receipts/g1_loopback.html) is compared
byte-for-byte with fresh output by the test suite.

The donor repository must be renamed and its live citations repaired before
this local G0 workspace is published under the Graphshell name.
