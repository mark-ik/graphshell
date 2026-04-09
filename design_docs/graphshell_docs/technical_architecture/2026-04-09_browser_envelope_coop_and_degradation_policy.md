<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Browser-Envelope Co-op and Degradation Policy

**Date**: 2026-04-09
**Status**: Architectural policy baseline
**Purpose**: Define the current Graphshell policy for co-op capability across
desktop, extension, browser-tab/PWA, and mobile envelopes, and make host
degradation explicit instead of leaving it to implied fallback behavior.

**Related docs**:

- [`2026-03-29_portable_web_core_host_envelopes.md`](2026-03-29_portable_web_core_host_envelopes.md)
- [`2026-03-30_protocol_modularity_and_host_capability_model.md`](2026-03-30_protocol_modularity_and_host_capability_model.md)
- [`2026-03-29_middlenet_engine_spec.md`](2026-03-29_middlenet_engine_spec.md)
- [`2026-04-09_graphshell_verse_uri_scheme.md`](2026-04-09_graphshell_verse_uri_scheme.md)
- [`../implementation_strategy/system/coop_session_spec.md`](../implementation_strategy/system/coop_session_spec.md)
- [`../research/2026-03-30_middlenet_vision_synthesis.md`](../research/2026-03-30_middlenet_vision_synthesis.md)

---

## 1. Why This Doc Exists

Several existing notes already establish:

- one core, many hosts,
- host-aware degradation as a canonical rule,
- co-op as a Verso bilateral/session concern rather than a Graphshell truth
  model,
- browser envelopes as strategically important future hosts.

What was still missing was one explicit policy answering:

- what co-op means per host envelope,
- which co-op behaviors are real today,
- which are future-compatible design directions,
- when a host should hide, degrade, proxy, or refuse a co-op action.

This note fills that gap.

---

## 2. Core Position

Graphshell must preserve **one co-op product model** across hosts, but it must
not pretend every host can realize the same transport or authority powers.

Therefore:

- co-op semantics are singular,
- host realization is conditional,
- degradation must be explicit,
- unavailable capability must not be surfaced as if it merely failed at
  runtime.

The user-facing rule is simple:

- desktop may expose the richest co-op surface,
- weaker hosts may expose partial or read-only co-op affordances,
- browser-host co-op must not be advertised as parity-complete until a real
  browser-safe transport path exists.

---

## 3. Current Baseline (2026-04-09)

Current reality:

- Graphshell is currently a native desktop-first application.
- The canonical co-op session specification lives under the Verso bilateral
  layer.
- Browser-extension, browser-tab/PWA, and mobile host envelopes remain target
  architecture rather than shipped product surfaces.
- Browser envelopes do not yet have a completed transport realization for full
  bilateral co-op that matches native assumptions.

This means Graphshell should currently behave as if:

- **desktop** is the only host profile permitted to promise full co-op,
- **browser envelopes** are not yet allowed to imply full hosted co-op parity,
- **mobile** remains host-specific and should inherit only explicitly validated
  co-op powers rather than desktop assumptions.

---

## 4. Host Policy by Envelope

### 4.1 Desktop

Desktop may expose the full co-op surface when the native runtime and Verso
layer can satisfy it.

Desktop-allowed assumptions:

- richer bilateral transport,
- stronger local storage integration,
- richer session ownership/hosting flows,
- explicit invitation/join/approval affordances,
- local graph/workbench integration around the shared co-op surface.

Desktop is the reference envelope for co-op today.

### 4.2 Extension

Extension hosts may eventually offer meaningful co-op affordances, but they are
transport- and lifecycle-constrained.

Policy today:

- do not imply full desktop-parity co-op,
- do not require raw socket or native-QUIC assumptions,
- only expose co-op surfaces that can be satisfied through explicit
  browser-safe transport or host delegation.

Future-compatible options:

- WebRTC data-channel realization,
- explicit companion/native-bridge delegation,
- limited session participation or invite handling.

Until one of those exists, extension co-op should degrade explicitly.

### 4.3 BrowserTab / PWA

Browser-tab and PWA envelopes are even more constrained than extensions.

Policy today:

- do not advertise full bilateral co-op,
- do not promise raw-network behavior the browser cannot provide,
- prefer explicit non-support over silent partial failure.

Permitted future directions:

- browser-safe join/observe flows,
- explicit WebRTC-based participation,
- proxied or delegated participation where the user can see that a host bridge
  is in the loop.

### 4.4 Mobile

Mobile should not be grouped with browser-tab/PWA hosts, but it also should not
inherit desktop powers by default.

Policy today:

- mobile may expose only the co-op features whose runtime, transport, and
  lifecycle constraints are explicitly validated for that platform,
- backgrounding, storage, and permission limits must be treated as part of the
  capability model rather than as incidental implementation details.

---

## 5. Degradation Rules

Graceful degradation for co-op means:

1. **Unavailable means unavailable**.
   The action should be hidden or presented as unsupported for that host, not
   surfaced as though it merely failed by accident.
2. **Read-only and delegated modes must be named honestly**.
   If the host can observe, import, or proxy but not truly host or join in the
   desktop sense, the UI should say so.
3. **No silent transport substitution**.
   The host must not secretly swap one transport/power model for another while
   presenting identical semantics.
4. **Capability limits belong in routing and diagnostics**.
   Users should be able to tell why a co-op surface is absent, reduced, or
   delegated.

---

## 6. The WebRTC Question

WebRTC is the most plausible browser-safe path toward meaningful browser-host
co-op, but it is not yet a completed Graphshell policy or implementation.

Policy baseline:

- Graphshell should treat WebRTC as a future browser-envelope transport option,
  not as an assumed fallback already granted.
- Browser-host co-op should remain degraded or unavailable until the WebRTC
  path is explicitly designed, scoped, and validated.
- If a future browser-host path uses WebRTC, it should be documented as a host
  realization of the same co-op model, not as a separate product.

---

## 7. UI and Routing Implications

Co-op actions should eventually be classifiable per host as:

- `full`
- `delegated`
- `observe-only`
- `unsupported`

Examples of user-facing copy or semantics that should become possible:

- "Host session" available only on validated native hosts,
- "Join via browser-safe transport" only where that path is real,
- "Observe session" where a read-only or proxied mode exists,
- "Unavailable on this host" where no honest realization exists.

This keeps co-op aligned with the broader Graphshell rule that routing and
capability limits should be explainable rather than discovered by failure.

---

## 8. Near-Term Program

The next closure steps are:

1. define a concrete host-by-host co-op capability matrix,
2. decide whether browser-host co-op is first implemented as WebRTC,
   delegation, observe-only mode, or explicit non-support,
3. thread that decision into routing, diagnostics, and feature visibility,
4. keep the portable product model singular even while host realizations differ.

The key architectural discipline is: **one co-op model, explicit host truth**.