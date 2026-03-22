# graphshell

    A prototype, spatial, P2P web browser

## Canvas

- Force-directed node graph canvas (that's the spatial bit) where nodes represent content (webviews, documents, media, file directories)
- The graph IS the session state, not a visualization or projection.
- Nodes are related by edge types organized into families: semantic, traversal, containment, arrangement, imported
- Manipulate graphlet structure manually. A graphlet is the connected component
  produced by the currently active edge projection, not a permanently fixed
  object.
- Configure what edge families/selectors are rendered per graph, per graph
  view, or for an explicit node selection

## Workbench

    Tile-tree arrangement of content views

- Panes: chromeless, ephemeral content view. No graph representation
- Tiles: container + pane with a tab handle, arrangable. Represented as nodes in the graph
- Tile groups: container with multiple tiles with impermanent layout. A tile
  group can stay linked to a graphlet definition or detach as an arrangement
  snapshot
- Frames: persisted arrangement of tiles (like split/quarterscreen arrangements). Represented as a box of nodes, each centered where their tiles are in the arrangement. Nodes in the frame can still have edges leading out of the frame.

## Navigator (in dev)

    a UI projection over your graph and workbench state

- Sidebar: full hierarchical tree — nodes, graphlets, frames, open tiles, cold members included. Automatically scopable to the graph or workbench, or includes both
- Toolbar: compact version of the sidebar for when you want the gutter space back


## Commands (done, description pending)

    Every action has a canonical ID, preconditions, and a reason when it's disabled. No silent dead buttons.

- Command palette: keyboard-invoked, fuzzy search over all registered actions
- Radial menu: context-aware, pointer/touch, geometry-constrained so label text doesn't collide
- Keybindings: keyboard-first throughout, fully remappable

## Lens system (to polish)

    A named configuration of how the graph renders and behaves. Applied per graph view, not globally.

- Layout: physics preset — force-directed, tree, ring, bus, etc.
- Filter: faceted filter on nodes and edges — what's visible and why
- Theme: visual encoding of edge families, node roles, badges
- Physics: simulation parameters — attraction, repulsion, damping
- Scene: saved combination of the above, switchable without touching graph truth

## Diagnostics (done and always expanding)

    The app, watching itself.

- Every subsystem emits structured events through a shared channel bus: compositor frames, layout coverage, UX tree snapshots, storage health, focus state, security events
- Channels have declared severity and schema versions — shape changes are caught by snapshot tests
- Whenever I run into an issue, my first move is the diagnostics pane. That's the goal: you open it and it tells you where things went wrong before you reach for a debugger

## Accessibility

    Everyone benefits from clear accessibility semantics, not just screen reader users.

- The UX tree semantic projection feeds AccessKit — screen readers get the same structured view of the workbench that the diagnostics pane does
- Keyboard navigation with deterministic focus order, region cycling, no focus traps
- Every surface must either meet the accessibility contract or explicitly declare partial support — silent regressions are caught by contract tests
- WCAG 2.2 Level AA target

## Build and Run (Standalone)

Graphshell has scripts for defining build environments, but I use cargo first and foremost, so that's what I'd try first on your system. Remember though, this is a prototype! Be prepared to bugfix or stick to releases for more predictable behavior.

    # Build (debug profile, aka default)
    cargo build

    # Run (debug profile)
    cargo run -- https://example.com (or the first node will default to https://www.servo.org)

    # Test (debug profile by default)
    cargo test

    # Check/format/lint (you'll probably catch doc formatting issues, recommend you exclude design_docs)
    cargo check
    cargo fmt
    cargo clippy

Use release profile only when you need runtime/perf parity:

    cargo build --release
    cargo run --release -- https://example.com

Optional helper scripts in `scripts/dev/` are wrappers around cargo for lane-safe target directories and convenience; they are not required nor really recommended for normal development.

See `design_docs/graphshell_docs/technical_architecture/BUILD.md` for platform prerequisites, debug testing workflows, and extended cargo usage. All of that is inferred from Servo, especially the platform prerequisites because I don't have a linux or mac system to test on. Make an issue if they're inaccurate and/or you found a workaround.

## Environment Overrides

These environment variables override CLI/prefs for quick tuning (occasionally good for avoiding long builds):

- `GRAPHSHELL_PERSISTENCE_OPEN_TIMEOUT_MS`: Startup persistence open timeout in ms (0 = wait indefinitely).
- `GRAPHSHELL_VERSE_INIT`: `off`, `background` (default), or `blocking`.
- `GRAPHSHELL_TRACING_FILTER`: Overrides tracing filter string.
- `GRAPHSHELL_GRAPH_DATA_DIR`: Override graph persistence directory.
- `GRAPHSHELL_GRAPH_SNAPSHOT_INTERVAL_SECS`: Override snapshot cadence (seconds).
- `GRAPHSHELL_DEVICE_PIXEL_RATIO`: Override device pixel ratio.
- `GRAPHSHELL_SCREEN_SIZE`: Override screen size, e.g. `1280x720`.
- `GRAPHSHELL_WINDOW_SIZE`: Override initial window size, e.g. `1024x740`.
- `GRAPHSHELL_HISTORY_MANAGER_LIMIT`: Max entries shown in History Manager lists.

## Verse - P2P Networking, in your browser! (being designed)

    pool storage, browsing reports, and data weights into a decentralized, permissions-based, and federated network of communities.

    **Optional, decentralized network components**

- Nostr NIP-72 communities for creating, curating, and sharing various amenities (indices, applets, graph views, hosted sites, mods for graphshell (commands, layouts, scenes, etc.), model skills, forums), semantically-categorized, and self-organized.

### Decentralized Search (shout out, YaCy)

    distributed, topic-scoped search indices stored as blobs

- Communities maintain index artifacts: tantivy segments, technically
- - Subscribe to indices you trust
- - Queries run locally against your own graph and any indices you've downloaded.
- - Alternatively, you can query a search provider in a verse for indices too large to download, possibly for a fee
- - Indices are forkable, mutually composable, and should be encrypted/permission-oriented.
- Communities fund crawling through bounties: post a target, user crawler(s) build the index, validators check it, tokens/receipts release to crawler contributors.
- No central search engine: the community that cares about a topic maintains the index for it.


### LoRA / FLoRA

    Contributors keep their raw data local and submit adapter weight updates to build communal algorithms/model adaptors, specialized to their subject matter.

- - With access rights, you can download versions of that verse's LoRA as a portable knowledge layer for your own model, fork it, use it to grow your own private verse's LoRA... the possibilities for open source LLM/model tailoring are pretty big.
- - Verse communities can gate model access by contribution, reputation, or moderation policy, including review buffers before submitted updates are merged into the shared adapter.
- - - Curate and semantically-grade datasets to meet communities' data bounties
- - - Circumstantial value dependent on dataset size, character, consistency, quality, range, subject, and semantics
- - keep your raw data private always, but the more you share weights derived from that data without significant updates to the dataset, the less rare and valuable the weights become.

### Decentralized Storage Time Bank

    shared storage sourced from either self/peer/priority-hosting, or decentralized storage economy

- - Commit storage to the network for a certain amount of time, and recieve tokenized compensation. I don't want to make an asset for $$$$, just a utility for a real problem!
- - The goal is to make the networked data reliably accessible and only to permitted users, so you'd basically get paid in the right to more storage, depending on the number of peers looking for the data and the number of seeds

### Reputation

    All of this — storage, indexing, crawling, reviews, FLora contributions — is accounted through a Proof of Access ledger. Receipts are evidence of work, not money.

- Reputation is always computed; financialization is optional and off by default. I don't want to see sats everywhere; talk about a hostile, fork over your cash ux!
- Reputation half-life: reputation decays gently over time so early participants don't entrench permanently
- - High water mark: the highest your rep gets in a community is persisted as a separate rep field
- - This decay rate (halving rep by specified time) can be adjusted by the rep attributing party, between the bounds of a month and a year.
- - - Floor: can't be so fast that less than a month away tanks half your rep
- - - Ceiling: can't be so slow that someone inactive for a year retains  power
- Community-specific reputations
- - Provide you with crediblity depending on how people view the standards of the community
- - Communities decide their own reward schedules, what they value, what they accept in type, granular semantics, and quality, and how quickly their reputations decay.
- - For network reputation, receipts have reputational utility but characterize what you do ~in context~, disallowing low-volume, low quality reputational inflation
- - - Blobs served: storage and retrieval cycles
- - - Index quality (community/validator semantic grading vs contributor's manifest and grading)
- - - FLoRA contributions (weight submissions)
- - - Review, admin, moderation work

### Co-op Browsing

    Share a graph session with other participants — browsing together or asynchronously.


- Collaborative browsing where changes to a shared graph synchronize across participants
- Async mode: check a shared session out like a git branch, browse and edit independently, then merge your diffs back -- or don't!
- Live mode: join a shared co-op session, a real-time synchronized graph view instance with time-synchronized web processes and version-controlled history (just like the local, offline graph!)
- - See your guest/host/peer's (customizable) cursor flit around the canvas!
- - Highlight a node with (auto-assigned but configurable) peer accent color when you open a page,
- - Screenshare a webview to another participant; the share is cast into a pane or an arrangeable tile on their end
- Built on [iroh](https://github.com/n0-computer/iroh) for transport

### Nostr (spine partially reticulated)

    Graphshell uses Nostr as its identity and social/post layer

- Identity: your Nostr keypair is your identity across the whole stack.
- - Identifies you to peers, communities, and co-op sessions; I want to pair it with other keys, too (like Matrix IDs) but we'll see how that plays out.
- Follow other users' public graphs; browsing activity/chains of posts/comments could be surfaced as a graphlet (each post having a url, timestamp, and a reply edge to a prior post), or more simply presented in an applet or web app
- Verse communities are Nostr communities (NIP-72) under the hood — moderation, membership, and governance happen there
- DMs and notifications use Nostr relays (NIP-17 sealed gift-wrap)
- No account nor server required, just a keypair. But you can be a relay and rebroadcast your choice of stuff!

### Matrix Rooms (not yet)

    Between a co-op session and a full Verse community, this is the tier for mid-sized groups of peers

- A Matrix room gives a group a persistent, federated space for coordination without committing to managing a verse
- Useful for small teams, classrooms, or private research groups that want shared browsing history/chat/graph views without running a community node
- Graph snapshots and session links can be shared into Matrix rooms directly from the workbench
- The room's persistence, storage budget (self/peer hosted), moderation are determined by the admins/membership


## AI Disclaimer

First, a disclaimer: I use and have used AI to support this project.

The idea itself is not the product of AI. I have years of notes in which I drafted the graph browser idea and the decentralized network component. I iterated my way into the insight that users should own their data, not be tracked, and we ourselves can capture much richer browsing insights than trackers. That's the real source of the second, prospective half of this project, the Verse bit.

Now, that said, there's tremendous potential in this tech. I have no fear of new tech. The costs and benefits are able to be balanced if they are managed collectively and not monopolistically. And we can use our collective resources so much more efficiently, with AI.

We need not destroy the environment, poison our water, or heat our atmosphere just because we have new tools. That's a capitalism problem. My thing is, stop using *any* tools to capitalize on and exploit people.

The biggest cost people bear is the cost of being exploited by profit-driven companies. Can you imagine the money they're going to squeeze from you in the future, when you really do need some level of routinized, predictable computation to deal with every day life?

I want to make sure normal people have a path to being able to manage and benefit from the tremendous amounts of data they own and make, that privacy is the default on the web, and that we can pool our resources in a consensual, communal manner.

I'm not an experienced developer in the least but I've got opinions, a smidgen of coding experience, and honestly, I want to learn how to use these discursive tools and see how far I can get with them. I've also followed the Servo community for years, despite not being a real developer: please contribute if you are able!

This is an open source, non-commercial effort. These ideas work much better open source forever as far as I'm concerned.

## History

My first inkling of this idea actually came from a mod for the game Rimworld, which added a relationship manager that arranged your colonists or factions spatially with links defining their relationships. It occurred to me that this UI, reminiscent of a mind map, would be a good fit for representing tabs spatially, and that there were a lot of rule-based options for how to arrange not just the browsing data, but tons of data patterns in computing.

I learned there was a name for this sort of UI: a force-directed node graph. A repeating, branching pattern of nodes connected to nodes by lines (edges). The nodes are browser tabs (or any file, document, applet, application, etc.), edges represent the relationship between the two nodes (clicked hyperlink, historical previous-next association, user-associated), and all nodes have both attractive and repellant forces which orient the graph's elements.

Depending on the behavior you want from the graph or the data you're trying to represent, you alter the canvas's physics and node/edge rules/types. You could filter, search, create new rules and implement graph topologies conducive to representing particular datasets: trees, buses, self-closing rings, etc.

This leads to rich, opinionated web browsing datasets, and the opportunity to pool our resources to visualize the accessible web with collective browsing history that is anonymous, permissions- and reputation-based, peer-to-peer, and open source. The best implementation of both halves would be somewhere between federated googles combined with subreddits with an Obsidian-esque personal data management layer.

Other inspirations:

- The Internet Map <https://internet-map.net/>
- YaCy (decentralized search index)
- Syncthing (open source device sync)
- Obsidian (canvas, plugins)
- Anytype (IPFS, shared vaults)

### Want to help?

If you can, **please contribute to [Servo](https://servo.org)**! Servo needs high-quality contributions (and cash) more than ever to close the WPT gap and implement the web standards that will empower graphshell and many more sophisticated browsers to come.

**Servo does not accept AI contributions**, but the components shared with Firefox may be covered by Firefox's more permissive AI policy.

If you want to help improve graphshell's infrastructure and are intimidated (like me) by Servo's complexity...

- raise issues in this repo if you actually use this hobby project of mine
- you can also contribute to the crates I'm using, in particular egui: egui_glow, egui_graphs, egui_tiles, egui-winit, egui-file-dialog, egui-notify.
- I'm a real sucker for a helper, please tell me if I'm approaching this naively and you have a better recommendation!

Lastly, I am not a rich man, and I would like to live alone and work on labors of love. If you find that you have a ducat or two left after tithing the big projects, here's [my ko-fi link](https://ko-fi.com/markik).
