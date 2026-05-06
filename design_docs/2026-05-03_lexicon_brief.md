# Strophos Lexicon Brief

**Status**: Active / working snapshot
**Date**: 2026-05-03
**Scope**: Establishes the post-rename naming scheme and in-product vocabulary for the Strophos-rooted project. Where this brief and [`TERMINOLOGY.md`](TERMINOLOGY.md) disagree on terms covered here, this brief is authoritative until `TERMINOLOGY.md` is revised. Terms not addressed here continue to follow `TERMINOLOGY.md`.

**Execution status**: **All renames are unexecuted in code.** This brief defines the target state. Filesystem moves, Cargo package renames, doc-root migrations, and inline-text rewrites are pending explicit triggers (see §6).

**Naming history note**: The product name went through two commits this same day. *Lemni* was the first lock; rejected after a deeper sweep surfaced [Lemni Inc.](https://www.lemni.com/), a Sequoia-backed AI-agent SaaS startup with a Class 9 USPTO mark. *Strophos* (with *Strophalos* as evocative long-form) replaced it after sweeps confirmed clean trademark, crates.io, and software-namespace surfaces.

---

## 1. Top-Level Naming

| Name | What it labels | Notes |
|------|----------------|-------|
| **Strophos** *(evocative form: **strophalos**)* | The product / unified top-level supercrate | Replaces both *Graphshell-as-product-name* and *Verse-as-network-layer*. The Navigator is a single surface with configurable scope; "local browsing" and "networked community graph" are scope settings, not separate products. *Strophos* (Greek *στρόφος*, "twist/turn") is the project/brand name; *strophalos* (Hekate's Wheel — the magical knowledge-navigation instrument) is available as the evocative instance form when project-vs-instance distinction is helpful. |
| **Verso** | Engine-manager supercrate | Orchestrates Wry, Serval, Middlenet, file/media viewers. Owns iroh transport, master keypair (via shared core), session lifecycle primitives. |
| **Murmur** | Bilateral p2p comms supercrate | Cable, co-op session chat, bilateral identity derivation. Migrated from Verso. *Bilateral-only* — many-to-many comms live in Mootcore. |
| **Mootcore** | Community / federation supercrate | Manages moots, demesnes (federations of moots), replication, governance. Replaces the former Verse layer. |
| **Graphshell** | Shell layer | UI workbench, Navigator surface, tile tree, control UX. **Now a *component* within Strophos**, not the product brand. |
| **Serval** | Servo-wgpu fork | One of Verso's engines. Rename from `servo-wgpu` still gated on milestone ordering (iced host migration M5a, webrender SPIR-V backend, servo-wgpuification). |
| **Netrender** | webrender-wgpu fork | Same milestone-ordering constraint as Serval. |
| **Middlenet** | Smolweb engine | Portable engine for Gemini/Gopher/static HTML/Markdown/RSS-Atom and similar. Pre-existing. |
| **Wry** | System webview engine | Third-party. One of Verso's engines for OS-native HTML rendering. |

---

## 2. Architectural Roles

Three other supercrates orbit Strophos:

- **Verso** — *the viewer/engine domain*. What renders content, what manages browser engines, what provides the bilateral-peer transport primitives (iroh, ALPN registration, session lifecycle). Verso : engines :: Mootcore : community-protocols.
- **Murmur** — *the bilateral-comms domain*. One-to-one p2p messaging and co-op session chat. Cable lives here. Bilateral identity derivation (per-cabal-keypair-from-master) lives here. Consumes Verso's transport primitives.
- **Mootcore** — *the community/federation domain*. Many-to-many: Matrix rooms, Nostr publication, IRC public lanes, ATproto, ActivityPub (where adopted), and Strophos-native moot infrastructure. Federation, replication, moot discovery, tessera validation across federated communities (demesnes).

Within Strophos:

- **Graphshell** is the shell component (UI, workbench, Navigator). Consumes everything above.
- **Mnem** is the local memory/database component (your private accumulated browsing graph, distinct from any moot's flora). Persistence layer (fjall + redb + rkyv) lives here.

---

## 3. In-Product Lexicon

Vocabulary used in user-facing language and architecture docs:

| Term | Role |
|------|------|
| **moot** *(count noun)* | A persistent themed federatable graph-view community. Each named community is a moot. Generic English-word usage; brand-trademark surface is not engaged because Strophos is the product, *moot* is the count noun. |
| **demesne** *(count noun)* | A federation of moots — a sovereign cluster. (Feudal: a lord's land held for own use; here: a moot-cluster held in common.) Demesnes can validate each others' tessera and exchange engrams. |
| **suzerainty** *(relation)* | The relationship between a demesne and its constituent moots — overlordship without absorbing internal sovereignty. Each moot keeps its own governance; the demesne provides shared infrastructure and tessera-validation. |
| **volvelle** | UI form factor — what a moot looks like when expanded radially in the Navigator. (From medieval *volvelle*: a rotating-disc paper instrument for aligning astronomical/calendrical knowledge.) Same primitive as outer-graph-node when collapsed; volvelle when focused-into. |
| **astroid** | Internal vocabulary for the hub-collapse UX. Collapsing a graphlet to its central/oldest node forms an astroid-shaped boundary with cusps where major outgoing edges remain. The astroid curve (algebraic geometry: path of a point on a small circle rolling inside a circle 4× its radius) gives the shape its name. |
| **tessera** | Trust / contribution / reputation token. Accrues through community contribution (storage hosting, bounty fulfillment, longtime good standing), redeemable for service / knowledge / access. Demesnes can validate each others' tessera. (Roman *tessera hospitalis*: guest-friendship token between communities — two halves that fit together.) |
| **engram** | Canonical portable contribution payload — `TransferProfile` envelope plus typed `EngramMemory` items. Already canonical; see [`verse_docs/implementation_strategy/engram_spec.md`](verse_docs/implementation_strategy/engram_spec.md). Replaces any earlier "gist" terminology. |
| **flora** | Accumulated body of engrams that constitutes a moot's culture / geist. A moot's flora grows as members submit engrams. |
| **mnem** | Private local accumulated browsing memory — your personal graph, distinct from any moot's flora. Component name within Strophos. (Short for *mnemosyne*.) |
| **kith / kin** | Contact tier distinction. *Kith* = those known to you (acquaintances, neighbors). *Kin* = close (friends, family, chosen relations). Orthogonal to moot membership: a person can be your kin and not be in any of your moots; they can be a moot-mate without being kith. |
| **strophalos** *(evocative instance)* | An individual running instance of Strophos as the user's personal knowledge-wheel. Optional vocabulary; only used where the project-vs-instance distinction is helpful (e.g., "your strophalos has 47 moots and 3 demesnes"). |

---

## 4. Retired Terms

Do not revive without explicit decision:

| Retired term | Replacement |
|--------------|-------------|
| **Lemni** *(prior product-name commit, retired same day)* | Replaced by **Strophos**. Rejected after sweep surfaced [Lemni Inc.](https://www.lemni.com/) — Sequoia-backed Amsterdam AI-agent SaaS startup with Class 9 USPTO mark. Direct adjacent vertical = wall. |
| **Lemniscate** *(prior long-form)* | Replaced by **Strophalos**. Workable Class 9/42 surface but materially risky given Lemni Inc.'s adjacent claim (likelihood-of-confusion with the LEMNI mark). |
| **Verse** *(network layer)* | Folded into Strophos-at-network-scope. The Navigator handles networked-community as a form-factor of the same surface. `verse_docs/` directory expected to migrate into a combination of `strophos_docs/` and `mootcore_docs/` over time. |
| **Murmuration** *(community layer name)* | Replaced by **Mootcore** (supercrate name) + **moot** (count noun) + **astroid** (UX vocab). Murmuration was a TESS wall — Murmuration, Inc. (civic-tech nonprofit, Class 42) holds active marks. |
| **Gist** *(contribution unit)* | Replaced by **engram** (already canonical and richer; see `engram_spec.md`). |
| **Flock** *(contact grouping)* | Replaced by **kith / kin** distinction (more nuanced relational tiering). |
| **Graphshell** *(as product brand)* | Demoted to component name (the shell layer within Strophos). The crate name and shell-layer references survive; the product brand moves to Strophos. |

---

## 5. Triad Sonics

The three supercrate names form a sonic triad:

- **Verso** (VER-so) — the page being turned (Latin *verso*, "turned")
- **Murmur** (MUR-mur) — the soft sound between people
- **Strophos** (STRO-fos) — the wheel that turns through knowledge (Greek *strophos*, "twist/turn")

Each is two syllables, classical (Latin/Greek), soft consonants. Note the double meaning across Verso and Strophos: both name a *turn* — Verso is the *turned page*, Strophos is the *turning instrument*. Together with Murmur (the intimate sound between the turners) they read as a single project lexicon, not unrelated names assembled by accident.

The strophalos (Hekate's Wheel) gives the long form its conceptual depth: Hekate is the goddess of crossroads, thresholds, and liminal places, and a knowledge-graph browser is fundamentally a crossroads navigator. Browsing-as-threshold-crossing is the framing the name carries.

**Cultural awareness:** Hekate's Wheel is a live emblem in Hellenic Reconstructionist and Dianic Wicca traditions. The symbol means something to people who use it religiously; project communications should be aware of that without being apologetic.

---

## 6. Pending Triggers (Mechanical Work)

This brief is design-state only. The following remain pending explicit go-ahead from the maintainer before any of them happen:

1. **Stand up new doc roots**: `strophos_docs/`, `mootcore_docs/`, `murmur_docs/` parallel to existing `verso_docs/`, `nostr_docs/`, `matrix_docs/`.
2. **Migrate `verse_docs/` content** into the appropriate combination of `strophos_docs/` (product/Navigator concerns) and `mootcore_docs/` (community/federation concerns).
3. **Cable migration plan**: separate plan doc under `murmur_docs/implementation_strategy/` describing what moves Verso → Murmur (Cable application logic, bilateral identity derivation, co-op chat) and what stays in Verso (iroh transport, ALPN registration, session lifecycle, master keypair access).
4. **Top-level crate rename**: `graphshell` (current top-level Cargo package) → `strophos`. Probably big-bang once all dependent doc/code references are surveyed.
5. **Sub-component renames** still gated on existing milestone ordering: `servo-wgpu/` → `serval/` and webrender-wgpu fork → `netrender/` after iced host migration M5a, webrender SPIR-V backend, and servo-wgpuification land.
6. **`TERMINOLOGY.md` revision**: full rewrite to absorb this brief, reorganize the legacy section, and remove now-obsolete deprecated-term cross-references.
7. **CLAUDE.md global instructions** ([`~/.claude/CLAUDE.md`](~/.claude/CLAUDE.md)) reference Graphshell as the project; this should update to Strophos once renames execute.
8. **README.md / PROJECT_DESCRIPTION.md** at the repo root reflect the new product brand.

---

## 7. References

- Memory: [`project_naming_state.md`](~/.claude/projects/c--Users-mark--Code/memory/project_naming_state.md) — full naming-decision history with rejected candidates and TESS findings
- Memory: [`project_tessera_trust_token.md`](~/.claude/projects/c--Users-mark--Code/memory/project_tessera_trust_token.md) — the tessera concept reservation
- Memory: [`user_aesthetic_word_list.md`](~/.claude/projects/c--Users-mark--Code/memory/user_aesthetic_word_list.md) — pool of evocative/niche words for future component naming
- Existing: [`TERMINOLOGY.md`](TERMINOLOGY.md) — pre-Strophos canonical terminology (still authoritative for terms not addressed here)
- Existing: [`engram_spec.md`](verse_docs/implementation_strategy/engram_spec.md) — engram canonical spec (1100+ lines, predates this brief)
- Existing: [`engram intelligence-memory plan`](verse_docs/implementation_strategy/2026-02-26_intelligence_memory_architecture_stm_ltm_engrams_plan.md)
- Existing: [`COMMS_AS_APPLETS.md`](graphshell_docs/implementation_strategy/social/comms/COMMS_AS_APPLETS.md) — Comms surface family (lives within Strophos; consumes Mootcore)
- Existing: [`VERSO_AS_PEER.md`](verso_docs/technical_architecture/VERSO_AS_PEER.md) — Verso role spec (predates Cable migration)
