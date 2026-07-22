# Graphshell

Graphshell is a remote, local-first projection host. Applications retain their
own truth and authority; Graphshell stores only curation state, disclosed
scenes, and presentation preferences.

## G0 boundary

The initial workspace is intentionally portable:

- `graphshell-protocol` carries versioned score, scene, status, and intent
  messages over an unspecified carrier.
- `graphshell-client` keeps endpoint-scoped scene snapshots and curation-local
  state.
- `graphshell-endpoint` defines injected projection and intent traits for
  applications to implement beside their own truth.
- `graphshell` is a facade only. A Genet/Cambium application waits for the
  loopback representation proof.

These crates may depend on Scenograph contracts and serde. They must not depend
on Mere, Merecat, Isometry, Genet, Cambium, NetRender, a network runtime, or an
application model. Product adapters depend on `graphshell-endpoint` in the
other direction.

The donor repository must be renamed and its live citations repaired before
this local G0 workspace is published under the Graphshell name.
