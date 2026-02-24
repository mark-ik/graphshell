# Local Intelligence Integration Strategy (2026-02-24)

**Status**: Research
**Goal**: Define the technical stack and user experience for integrating local ML/AI into Graphshell.

## 1. Engine Selection: The Burn-Only Path

### Recommendation: `burn` (Universal Acceleration)
We select **`burn`** as the sole inference engine to maximize hardware compatibility on consumer devices.

| Feature | `burn` (wgpu) | `candle` (cuda/metal) |
| :--- | :--- | :--- |
| **Hardware Support** | **Universal** (Vulkan, DX12, Metal, OpenGL) | Fragmented (CUDA required for non-Apple) |
| **Consumer Viability** | High (runs on Intel iGPUs, AMD, etc.) | Low (requires Nvidia GPU or Mac) |
| **Architecture** | Pure Rust, dynamic graph | Pure Rust |
| **Model Ecosystem** | Manual architecture definition required | `candle-transformers` (plug-and-play) |

**Why Burn?**: Graphshell is a consumer desktop app. We cannot assume users have Nvidia GPUs with CUDA drivers installed. `burn`'s WebGPU backend allows us to run hardware-accelerated inference on almost any modern GPU (including integrated graphics) with zero driver setup.

### The Trade-off: Architecture Implementation
Unlike `candle`, which has a rich library of pre-ported models, `burn` requires us to define the model architectures (e.g., BERT, Llama) in our codebase.
*   **Strategy**: We will implement the specific architectures we need (e.g., `BertModel` for embeddings, `LlamaModel` for text) using Burn's module system.
*   **Weights**: We will load standard Hugging Face `safetensors` files, mapping the tensor names to our Burn module structure at runtime.

---

## 2. Model Management Strategy

We adopt a **Two-Tier Model Strategy** to balance capability with download size.

### Tier 1: "Invisible" Embeddings (Semantic Core)
*   **Purpose**: Semantic physics, clustering, "related nodes", auto-tagging suggestions.
*   **Model**: `sentence-transformers/all-MiniLM-L6-v2` (Architecture: BERT).
*   **Size**: ~20–90 MB.
*   **UX**:
    *   **Zero Config**: Downloaded automatically on first run (or bundled if license permits).
    *   **Always On**: Runs on CPU. Fast enough to process nodes as they are added.

### Tier 2: "Opt-In" Generative (LLM)
*   **Purpose**: Summarization, "Chat with Graph", Entity Extraction.
*   **Model**: `Meta-Llama-3.2-3B-Instruct` (Architecture: Llama).
*   **Size**: 2 GB – 6 GB.
*   **UX**:
    *   **Explicit Opt-In**: User clicks "Enable Local Assistant" in Settings.
    *   **Download Manager**: App fetches weights. Default source is Hugging Face Hub, but architecture supports pluggable sources (IPFS, Local File, S3).
    *   **Resource Aware**: Only loads into RAM when needed.

---

## 3. Integration Architecture

### 3.1 The Intelligence Agent
Implemented via `AgentRegistry`.

`B` represents the Burn Backend (e.g., `Wgpu` or `NdArray`).

```rust
struct LocalIntelligenceAgent<B: burn::tensor::backend::Backend> {
    embedding_model: Option<BertModel<B>>, // Implemented in mods/native/intelligence/models/bert.rs
    llm_model: Option<LlamaModel<B>>,      // Implemented in mods/native/intelligence/models/llama.rs
}

impl Agent for LocalIntelligenceAgent {
    fn on_intent(&mut self, intent: &GraphIntent) {
        if let GraphIntent::UpdateNodeUrl { key, .. } = intent {
            self.queue_for_embedding(*key);
        }
    }
}
```

### 3.2 The Vector Index
We need a place to store the `Vec<f32>` embeddings produced by Tier 1.
*   **Location**: `IndexRegistry`.
*   **Backend**: `lance` (embedded vector DB) or a lightweight HNSW crate (`hnsw_rs`).
*   **Query**: `index.search(vector, k=5)` returns nearest `NodeKey`s.

### 3.3 Semantic Physics Hook
The physics engine (`CanvasRegistry` -> `LayoutAlgorithm`) queries the `IndexRegistry`.
*   **Force**: $F = k \cdot \text{cosine\_similarity}(\vec{A}, \vec{B})$.
*   **Result**: Nodes about "Rust" cluster together physically, even if they don't link to each other.

---

## 4. User Experience Flow

1.  **Fresh Install**: App works offline. No AI.
2.  **Background**: App downloads Tier 1 embedding model (showing a small spinner in status bar).
3.  **Usage**: User adds 10 bookmarks.
4.  **Magic**: Nodes slowly drift into clusters (e.g., "News", "Tech", "Recipes") driven by the embedding force.
5.  **Discovery**: User types "healthy dinner" in Command Palette.
    *   `ActionRegistry` routes query to `IndexRegistry`.
    *   Vector search finds the "Recipes" cluster nodes (even if they don't contain the word "dinner").
6.  **Upgrade**: User enables "Summarization". App downloads 4GB LLM. Hovering a node now shows a generated summary.

---

## 5. Model Sovereignty (Avoiding Vendor Lock-in)

To ensure Graphshell works even if Hugging Face goes down (or changes policy), we decouple the *model definition* from the *weight source*.

### 5.1 The Model Manifest
Instead of hardcoding URLs, the app uses a manifest to resolve weights:

```json
{
  "model_id": "llama-3.2-3b",
  "architecture": "llama",
  "hash": "sha256:...",
  "sources": [
    { "type": "verse", "uri": "verse://<cid>" },
    { "type": "ipfs", "uri": "ipfs://<cid>" },
    { "type": "https", "uri": "https://huggingface.co/meta-llama/..." },
    { "type": "https", "uri": "https://models.graphshell.org/..." }
  ]
}
```

### 5.2 Candidate Models

#### Tier 1: Embeddings (Architecture: BERT)
*   **Primary**: `sentence-transformers/all-MiniLM-L6-v2` (22MB).
*   **License**: Apache 2.0 (Fully Open Source).
*   **Strategy**: **Bundle**. Small enough to ship with the app. Compatible with MPL 2.0.

#### Tier 2: Generative (Architecture: Llama-like)
*   **Primary**: `Phi-3.5-mini-instruct` (3.8B).
*   **License**: MIT (Fully Open Source).
*   **Alternative**: `Qwen2.5-3B-Instruct` (Apache 2.0).
*   **Strategy**: **Download**. Legally shippable, but 2GB+ is too large for a default installer.
*   **Note**: We prefer these over Llama 3.2 (Community License) to maintain a strict Open Source dependency chain.

#### Tier 3: Vision (Architecture: Florence)
*   **Primary**: `Florence-2-base` (230MB).
*   **License**: MIT.
*   **Strategy**: Download on demand.

### 5.3 Licensing & Distribution
*   **Compatibility**: Graphshell is MPL 2.0. Models licensed under Apache 2.0, MIT, or BSD are compatible for distribution alongside MPL code.
*   **Bundling**: We bundle Tier 1 models because they are small and essential.
*   **On-Demand**: We download Tier 2 models to respect user bandwidth/disk, not due to license constraints.

---

## 6. Concrete Feature Examples

### 6.1 The "Synthesizer" (Tier 2 Generative)
While Tier 1 models (Embeddings) find *connections*, Tier 2 models (LLMs) generate *content*.

*   **Auto-Summary Tooltips**: When hovering a `Cold` node (no active webview), the LLM reads the cached text and generates a 2-sentence summary for the tooltip.
*   **"Explain Selection"**: User selects 5 nodes about a topic and asks "What is the consensus here?". The LLM reads the content of those 5 nodes and generates a synthesis node.
*   **Structured Extraction**: User clips a recipe or product page. The LLM extracts specific fields (Ingredients, Price, SKU) into the node's metadata, turning unstructured web pages into a database.

### 6.2 The "Librarian" (Tier 1 Embeddings)
These models run continuously in the background to organize the graph.

*   **Semantic Physics**: Nodes attract not just based on links, but based on topic similarity. A node about "Rust" will physically drift towards other Rust nodes.
*   **Fuzzy Concept Search**: Searching for "learning resources" finds nodes titled "Tutorial", "Guide", and "Documentation" because they are semantically close.

### 6.3 The "Archivist" (Tier 3 Vision)
*   **Screenshot Search**: User pastes an image. The model captions it ("Screenshot of a terminal error"). The image becomes searchable by its content.

---

## 7. Expanded Model Catalog (License-Compatible)

Focusing on "Little Models" (Tier 1/2) that run efficiently on consumer hardware.

### Tier 1: Micro-Embeddings (The "Subconscious")
1.  **`Snowflake-Arctic-Embed-XS`** (Apache 2.0, ~22MB): Extremely efficient. Good for background clustering.
2.  **`BGE-Micro-v2`** (MIT, ~15MB): The smallest viable embedding model. Ideal for mobile/low-power.
3.  **`Nomic-Embed-Text-v1.5`** (Apache 2.0, ~137MB): Supports "Matryoshka" learning (variable dimension), allowing trade-offs between speed and accuracy dynamically.
4.  **`LaBSE`** (Apache 2.0, ~470MB): Language-agnostic. Essential if the user browses in multiple languages (clusters "Cat" and "Gato" together).

### Tier 2: Tiny Generative (The "Reflexes")
5.  **`SmolLM2-1.7B-Instruct`** (Apache 2.0, ~1GB): State-of-the-art for its size. Good for simple summaries and extraction.
6.  **`Qwen2.5-0.5B-Instruct`** (Apache 2.0, ~350MB): Tiny but capable of logic. Runs instantly on almost anything.
7.  **`Danube3-500M-Chat`** (Apache 2.0, ~350MB): Optimized for chat. Good for "talking to a node."
8.  **`TinyLlama-1.1B-Chat`** (Apache 2.0, ~640MB): A robust baseline for older hardware.

### Tier 3: Specialized (The "Senses")
9.  **`Moondream2`** (Apache 2.0, ~1.5GB): Tiny Vision-Language Model. Runs on CPU. Can describe images.
10. **`Whisper-Tiny.en`** (MIT, ~75MB): Speech-to-text. Essential for indexing Audio nodes.

---

## 8. Application Matrix (20 New Use Cases)

### Graph Organization & Physics
1.  **Sentiment Physics**: Color nodes by sentiment (Green=Positive, Red=Negative). Apply repulsion between opposing sentiments.
2.  **Language Clustering**: Automatically group nodes by language (e.g., Rust docs cluster separately from Python docs).
3.  **Reading Level Heatmap**: Visualize complexity. "Introductory" nodes drift to the periphery; "Academic" nodes pull to the center.
4.  **Semantic Deduplication**: Identify nodes with different URLs but identical meaning (e.g., a blog post cross-posted to Medium).
5.  **Topic Modeling Layers**: Generate "Topic Region" labels (e.g., "Finance", "Cooking") that float over graph clusters.

### Navigation & Hygiene
6.  **Link Rot Prediction**: Analyze page text for "404", "Moved", or domain parking text. Flag nodes as "Rotten".
7.  **Dead Link Repair**: For "Rotten" nodes, use the LLM to construct a valid Wayback Machine URL.
8.  **Desire Path Prediction**: Highlight the "most likely next node" based on the current node's content and user history.
9.  **Spam/Ad Filtering**: Dim or auto-archive nodes that match "SEO spam" or "Content Farm" patterns.

### Content Extraction
10. **Code Snippet Extraction**: Automatically pull code blocks from web nodes into child "Snippet" nodes (Text/Markdown).
11. **Citation Graphing**: Extract references/bibliography from a page and auto-create edges to those URLs if they exist in the graph.
12. **Meeting/Event Extraction**: Detect dates/times in text and offer to create "Event" nodes or calendar entries.
13. **Auto-Edge Labeling**: When a user links two nodes, suggest a label for the edge (e.g., "contradicts", "supports", "example of").

### Visuals & UI
14. **Smart Icons**: If a node lacks a favicon, generate a relevant Emoji icon based on content analysis.
15. **Tab Group Naming**: Generate concise, descriptive names for auto-grouped tabs (e.g., "Rust Async Docs").
16. **Visual Saliency Crop**: Use a vision model to center node thumbnails on the most "important" part of the page (content vs nav).

### Multimodal & Synthesis
17. **Audio Search**: Index the content of `Audio` nodes (podcasts, voice notes) using Whisper so they appear in text search.
18. **Image Tagging**: Auto-tag `Image` nodes with detected objects ("diagram", "screenshot", "nature").
19. **Workspace Chat**: "Chat with your Workspace" — RAG over the currently open nodes.
20. **Graph Diff Summary**: When opening a shared workspace, summarize what changed: "Alice added 5 nodes about React and removed the old docs."

---

## 9. Recommended Minimal Stack

To cover all 20 applications with the smallest possible footprint while maintaining open licenses (Apache 2.0 / MIT), we recommend this specific 4-model stack.

**Total Size: ~680 MB** (Fits easily in RAM alongside browser)

### 1. Logic & Text: `Qwen2.5-0.5B-Instruct` (~350 MB)
*   **Role**: Summarization, Extraction, Classification, Chat.
*   **Why**: Significantly outperforms other sub-1B models on structured tasks (JSON extraction). Apache 2.0.
*   **Burn Implementation**: Native `Llama` architecture (Qwen is Llama-compatible).
*   **Feature Flag**: `intelligence-llama`.

### 2. Embeddings: `all-MiniLM-L6-v2` (~22 MB)
*   **Role**: Clustering, Semantic Physics, Deduplication, Search.
*   **Why**: Industry standard efficiency/performance ratio. Apache 2.0.
*   **Burn Implementation**: Native `Bert` architecture.
*   **Feature Flag**: `intelligence-bert` (Core).

### 3. Vision: `Florence-2-base` (~230 MB)
*   **Role**: Image Tagging, Smart Icons, Saliency Cropping.
*   **Why**: Unlike chat-based VLMs, Florence is task-trained for region proposals and object detection, enabling features like "Smart Crop" that chat models cannot do. MIT.
*   **Burn Implementation**: `burn-import` via ONNX (due to complex encoder-decoder architecture).
*   **Feature Flag**: `intelligence-vision`.

### 4. Audio: `Whisper-Tiny.en` (~75 MB)
*   **Role**: Audio Indexing.
*   **Why**: Standard, reliable, lightweight. MIT.
*   **Burn Implementation**: Native `Whisper` architecture.
*   **Feature Flag**: `intelligence-audio`.

---

## 10. Verse Model Distribution (The "Hugging Face" Alternative)

Fetching models from Verse is the ultimate goal for model sovereignty. It turns model acquisition into a graph traversal problem.

### 10.1 Tier 1: Direct Sharing (The "Sneakernet" Equivalent)
*   **Mechanism**: You pair with a friend who has the model.
*   **Action**: They share a workspace containing a "Model Node" (a node representing the model file).
*   **Transfer**: Graphshell syncs the model blob via iroh (resumable, fast).
*   **Use Case**: Sharing a finetuned LoRA or a specific quantized version within a team.

### 10.2 Tier 2: The Model Index (The "Decentralized Hub")
*   **Concept**: A "Model Index" is just a specialized Verse Index Artifact (see `storage_economy_and_indices.md`).
*   **Content**: Instead of web pages, it indexes `ModelManifest` files and points to `VerseBlob`s containing the weights.
*   **Curation**:
    *   **Community**: A "License-Compatible Models" community maintains an index of safe, open models.
    *   **Verification**: Validators check that the weights match the hash and the license is compatible (Apache/MIT).
*   **Discovery**: Users subscribe to the "Open Source Models" Verse. New models appear in their local search/command palette automatically.

---

## 11. Verse Intelligence: Network-Level Learning

Beyond just distributing static models, Verse can evolve its own intelligence layer using the four selected models as a foundation.

### 11.1 The Data Flywheel (Dataset Generation)
Verse's primary contribution to AI is **High-Quality, Human-Curated Data**.
*   **The Input**: Users browse, tag (UDC), link, and annotate nodes in Graphshell.
*   **The Artifact**: These actions generate **Reports** (VerseBlobs).
*   **The Value**: A Report is a labeled training example: "Here is a URL, its content, and exactly how a human categorized and linked it."
*   **Scale**: A community of 100 users generates a massive, clean instruction-tuning dataset for `Qwen` or `MiniLM` just by using the browser.

### 11.2 Community Fine-Tuning (Refining Capabilities)
Instead of training massive base models, Verse communities can produce **LoRA Adapters** (Low-Rank Adaptations) for the base models.
*   **Scenario**: The `#rust-lang` Verse community aggregates 10,000 reports on Rust crates.
*   **Action**: A curator (or automated peer) fine-tunes `Qwen2.5-0.5B` on this dataset.
*   **Result**: A `rust-expert.lora` file (~10MB).
*   **Distribution**: This adapter is published to the Verse. Subscribers to `#rust-lang` automatically download it.
*   **Effect**: Their local Qwen model now understands Rust idioms, crate dependencies, and community terminology far better than the base model.

### 11.3 The "Model Swarm" (Distributed Inference)
While Tier 1 focuses on local inference, Tier 2 allows peers to offer inference as a service.
*   **Specialization**: Peer A has a powerful GPU and runs `Llama-3-70B`. Peer B runs `Florence-2-Large`.
*   **Routing**: A user's local `IntelligenceAgent` can route complex queries to these specialized peers (via `QueryRequest`) if local models are insufficient.
*   **Privacy**: This remains optional and trust-based.

### 11.4 Is this Machine Learning?
Yes, this is **Decentralized Federated Learning**.
*   **Traditional ML**: Central server scrapes web -> trains model -> serves API.
*   **Verse ML**: Users curate web -> Verse aggregates data -> Communities fine-tune adapters -> Peers run inference.

---

## 12. Pluggable Intelligence Architecture (The "Operator Registry")

To support the vision of user-chosen, upgradeable, and FOSS models, we define a **Model Registry** (distinct from the code `ModRegistry`).

### 12.1 The Model Registry
*   **Role**: Manages available model weights, architectures, and capabilities.
*   **Entries**: `ModelDefinition` (manifest) + `ModelWeights` (blob).
*   **Sources**: Local disk, Verse (P2P), HTTP (Hugging Face/S3).

### 12.2 Capability Contracts
Features do not depend on specific models; they depend on **Capabilities**.

| Feature | Requirement | Satisfied By (Examples) |
| :--- | :--- | :--- |
| **Semantic Physics** | `capability: embedding` | `MiniLM-L6`, `BGE-Micro`, `Bert-Large` |
| **Summarization** | `capability: text-generation` | `Qwen2.5-0.5B`, `Phi-3.5`, `Llama-3-8B` |
| **Smart Icons** | `capability: vision-labeling` | `Florence-2`, `Moondream` |

### 12.3 Configuration Profiles
Users select a "Intelligence Profile" that maps capabilities to specific models.

*   **"Minimal FOSS"** (Default): `Qwen2.5-0.5B` + `MiniLM-L6`. (~400MB RAM).
*   **"Power User"**: `Llama-3-8B` + `BGE-Large`. (~6GB RAM).
*   **"Visionary"**: Adds `Florence-2` for image features.

### 12.4 The Upgrade Path
1.  **Discovery**: The `ModelRegistry` subscribes to a "Model Updates" Verse channel.
2.  **Notification**: "A better embedding model (`MiniLM-L12`) is available. Upgrade?"
3.  **Hot Swap**: The `IntelligenceAgent` unloads the old model and loads the new one. The interface (`embed(text) -> Vec<f32>`) remains stable.

### 12.5 Adapter Injection (Personalization)
This architecture enables **LoRA Injection**.
*   **Base**: User picks `Qwen2.5-0.5B` (Generic).
*   **Adapter**: User loads `rust-lang.lora` (from Verse).
*   **Runtime**: The inference engine applies the adapter weights on top of the base model.
*   **Result**: A personalized, domain-expert model without downloading a full fine-tune.