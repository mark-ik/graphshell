# Internal Address Scheme Implementation Plan

**Date**: 2026-03-03
**Status**: Active / Canonical planning slice
**Purpose**: Define how internal system and content addresses are issued, resolved, and integrated so address-as-identity becomes a runtime-enforced contract instead of scattered doc intent.

**Canonical references**:

- `../../../TERMINOLOGY.md`
- `../workbench/graph_first_frame_semantics_spec.md`
- `../aspect_control/2026-02-20_settings_architecture_plan.md`
- `../viewer/clipping_and_dom_extraction_spec.md`
- `../2026-03-03_spec_conflict_resolution_register.md`

---

## Progress Note (2026-03-03)

The current runtime slice has already landed the following:

- typed internal address parsing/formatting with canonical `verso://` emission and legacy `graphshell://` parse compatibility,
- workbench-authority routing for `verso://settings/...`, `verso://frame/...`, `verso://tool/...`, and `verso://view/...`,
- initial content-domain scaffolds for `notes://`, `graph://`, and `node://`,
- in-memory durable note records plus `notes://<NoteId>` resolution and `verso://view/note/<NoteId>` routing.

Still intentionally unresolved:

- durable `graph://<GraphId>` authority and `verso://view/graph/<GraphId>` resolution,
- a real note pane/editor surface that consumes the queued note-open state,
- clip authority definition and `verso://clip/<id>` semantics.

---

## 1. Canonical Address Families

The system-owned internal namespace is now:

```text
verso://settings/<section>
verso://frame/<FrameId>
verso://tool/<name>
verso://tool/<name>/<n>
verso://view/<legacy-GraphViewId>            # compatibility path
verso://view/graph/<GraphId>                 # target shape; parser scaffold only
verso://view/node/<NodeId>
verso://view/note/<NoteId>
verso://clip/<id>                            # unresolved until clip authority is defined
```

Legacy compatibility:

- `graphshell://...` remains parseable as a compatibility alias.
- New canonical formatting should emit `verso://...`.

Durable content/domain namespaces are now split out:

```text
notes://<NoteId>
graph://<GraphId>     # parser scaffold; no durable graph authority yet
node://<NodeId>       # parser scaffold; runtime view routing can resolve current node IDs
```

Rules:

- `verso://` is reserved for system/workbench authority routes.
- Content-domain schemes identify durable records and should not encode pane/layout handles.
- Address parsing and formatting must be centralized; callers must not hand-roll string variants.

---

## 2. Issuance Rules

### 2.1 General

An internal address is issued when Graphshell creates a graph-ownable internal surface identity.

Issuance must be:

- deterministic,
- reversible (parseable back to typed identity),
- stable for the lifetime of that surface identity.

### 2.2 Family-specific rules

- `view` (legacy path): issued only for compatibility with the older single-segment graph-pane routing form.
- `view/<kind>/<id>`: issued when the workbench wants to route a durable content identity onto a pane surface.
- `tool`: issued when a tool pane instance is created; instance suffix is added only for concurrent duplicates.
- `frame`: issued when frame identity is created and retained across handle open/close.
- `settings`: issued when a settings route is opened as a first-class pane surface.
- `clip`: deferred; no canonical issuance rule should be treated as final until clip authority is defined.
- `notes://`: issued when a durable note record is created.
- `graph://` / `node://`: reserved for durable record identity; canonical issuance rules remain to be finalized.

---

## 3. Canonical Graph-Citizenship Query

Graph citizenship is determined only by address resolution:

1. Does the pane have a canonical address?
2. Does that address resolve to a live non-tombstone node?

If both are true, the pane is graph-backed.

If either is false, the pane is not graph-backed.

No parallel membership table is permitted.

---

## 4. Write Path Contract

Internal addresses must enter the graph through one canonical write path.

Required behavior:

- Address issuance does not itself imply graph mutation.
- The reducer/apply path that creates or enrolls a graph-backed pane performs the node create/reuse decision.
- Duplicate handling follows the address-as-identity policy for that surface family.
- Destructive delete removes the live resolution of the address by tombstoning or removing the node.

This is especially critical for `verso://frame/<FrameId>` because frame identity, workbench handle state, and graph presence must remain aligned.

---

## 5. Integration Boundaries

### 5.1 Opening flows

- Ephemeral pane opens must not auto-write `verso://` addresses.
- Promotion to graph-backed state may issue and write an address if the pane becomes `Tile`.
- Internal surfaces that are graph-owned at creation time may issue their address immediately, but still must use the canonical write path for graph enrollment.

### 5.2 Traversal/history

- `verso://` internal routes are excluded from ordinary web traversal capture unless a spec explicitly defines otherwise.
- Promotion of an ephemeral pane into graph-backed state may use `NavigationTrigger::PanePromotion` when history semantics require it.
- Address writes must happen before any traversal append that depends on the destination node.

### 5.3 Storage/persistence

- Stored frame/tool/view identities must be able to reconstruct their canonical address exactly.
- Persistence restore must not mint a different address for an existing identity.
- Address parser/formatter changes require migration review because they affect graph identity.

Content-domain records (`notes://`, later `graph://` and `node://`) must be restorable without being rekeyed through the `verso://` workbench namespace. Workbench routing may consume those IDs, but it must not become the durable identity authority for them.

---

## 6. Diagnostics Requirements

Diagnostics should expose:

- parsed address family,
- typed identity payload,
- whether the address is system/workbench (`verso://`) or domain/content (`notes://`, `graph://`, `node://`),
- graph-backed resolution result,
- whether the last action was issue, reuse, failed-parse, or failed-resolve.

This is required so address-as-identity bugs are visible before they become data-shape corruption.

---

## 7. Validation Gates

This plan closes only when:

1. All active `verso://` families round-trip parse/format, and legacy `graphshell://` aliases remain parse-compatible until explicit removal.
2. `verso://frame/<FrameId>` is stable across handle close/reopen.
3. Restore does not reissue a new address for an existing internal identity.
4. Internal addresses do not bypass the canonical graph write path.
5. Traversal paths that depend on graph identity wait until address resolution is live.
6. Domain-address records (`notes://`, and later `graph://` / `node://`) resolve through durable record authorities rather than through ad hoc workbench-only IDs.

Suggested checks:

- Parse/format tests for each address family.
- Frame close/reopen scenario proving same address before and after reopen.
- Settings route open proving pane surface creation and graph enrollment use the same canonical address.
- Note route open proving `notes://<NoteId>` queues the durable note-open path.
- View-route tests proving `verso://view/note/<NoteId>` and `verso://view/node/<NodeId>` route durable content IDs onto panes.
- Clip delete proving `verso://clip/<id>` no longer resolves after deletion, once clip authority exists.

---

## 8. Immediate Implementation Slices

1. Keep the typed internal address parser/formatter (`VersoAddress`, currently aliased through `GraphshellAddress`) as the canonical system routing module.
2. Replace ad hoc string construction in internal-surface code paths with the typed module and canonical `verso://` emission.
3. Finish durable domain record authorities for `notes://`, `graph://`, and `node://` instead of treating `verso://view/...` as a substitute for content identity.
4. Audit all graph-backed internal panes for canonical write-path compliance.
5. Add diagnostics events around address issue/resolve/reuse.
6. Define clip authority before treating `verso://clip/<id>` as canonical.
7. Update the affected specs to reference this plan as the address authority.
