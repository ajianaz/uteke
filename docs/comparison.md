# Comparison

> How Uteke compares to other AI memory layers — a capability-first evaluation framework.

## The 7 Decision Dimensions

When evaluating any AI memory layer, these seven dimensions capture every meaningful difference:

| Dimension | Question |
|---|---|
| **Architecture** | One binary or a stack of services? |
| **Data sovereignty** | Does data ever leave your machine? |
| **Search quality** | Keyword, semantic, or hybrid? |
| **Multi-agent** | Can agents share a memory space? |
| **Time-travel** | Can you query past states? |
| **Integration** | MCP, HTTP API, CLI? |
| **Cost of ownership** | What dependencies are you signing up for? |

## Uteke Capability Matrix

| Capability | Uteke | What it means |
|---|---|---|
| **Architecture** | Single Rust binary | `curl \| sh && done`. No Docker, Python, or database server. |
| **Data sovereignty** | Fully offline | Local ONNX embeddings (768d). Zero telemetry, zero network calls. |
| **Search** | Hybrid (Vector + FTS5 + RRF) | Finds by meaning AND exact keywords. Reciprocal Rank Fusion (k=60) merges both. |
| **Recall speed** | ~45ms P50 at 10K memories | Local execution, no network round-trip. |
| **Multi-agent** | Rooms | Shared memory spaces with author attribution and cross-agent recall. |
| **Time-travel** | Native point-in-time | `uteke recall "deploy" --at 2025-01-15` — queries historical state. |
| **Extraction** | Offline (default) + LLM (optional) | Rule-based extraction works zero-config. Upgrade to LLM if desired. |
| **Integration** | MCP + CLI + HTTP API | JSON-RPC (stdio + HTTP), full CLI, REST API with view-only API keys. |
| **Dependencies** | Zero runtime deps | No Docker, Postgres, Neo4j, Python, or cloud account needed. |
| **License** | Apache 2.0 | Truly open source. |

## The Four Archetypes

Every memory layer falls into one of four archetypes:

### 1. Cloud-Native Platform
- **Promise:** We handle everything. Send us your data.
- **Trade-off:** Data leaves your machine. Per-query costs. Network dependency.
- **Best for:** Teams without data sovereignty concerns.

### 2. Self-Hosted Infrastructure
- **Promise:** Run it yourself with Docker.
- **Trade-off:** You manage Postgres, Neo4j, Qdrant — operational overhead.
- **Best for:** Teams with dedicated infrastructure engineers.

### 3. Python Library
- **Promise:** `pip install` and done.
- **Trade-off:** Python runtime management, package conflicts, GIL limitations.
- **Best for:** Prototyping, notebooks, research.

### 4. Single Binary
- **Promise:** One binary. Zero deps. Works everywhere.
- **Trade-off:** Smaller ecosystem of pre-built integrations.
- **Best for:** Edge computing, privacy-sensitive domains, simplicity-first developers.

**Uteke is Archetype 4** — single binary, zero dependencies, fully offline.

## Decision Tree

```
Data sovereignty requirements?
├── Yes → Can you manage Docker + databases?
│   ├── Yes → Archetype 2 (Self-Hosted)
│   └── No  → Archetype 4 (Single Binary) → Uteke
└── No → Production or prototyping?
    ├── Prototyping → Archetype 3 (Python Library)
    └── Production → Want to manage infrastructure?
        ├── Yes → Archetype 2 (Self-Hosted)
        └── No → Network latency acceptable?
            ├── Yes → Archetype 1 (Cloud-Native)
            └── No  → Archetype 4 (Single Binary) → Uteke
```

## Extraction Comparison

| Aspect | LLM-based (common) | Rule-based (Uteke default) |
|---|---|---|
| **API key required** | ✅ Yes | ❌ No |
| **Data leaves machine** | ✅ Yes (sent to LLM) | ❌ Never |
| **Cost per call** | ✅ Yes | ❌ Free |
| **Setup** | Configure API key, model, endpoint | Works immediately |
| **Quality** | Richer, context-aware | Pattern-based, good for common fact types |
| **Multilingual** | Depends on LLM | ✅ UTF-8 safe (tested with Indonesian, emoji) |
| **Upgrade path** | — | `mode = "llm"` in config to enable LLM extraction |

## Quick Benchmark

| Metric | Uteke | Notes |
|---|---|---|
| Recall (10K memories) | 42ms P50, 50ms P95 | HNSW O(log N), flat scaling |
| Insert throughput | 6–22 ops/s | CPU-bound (ONNX inference) |
| Storage per memory | ~10KB | SQLite + HNSW |
| LongMemEval Recall@5 | 0.958 | EmbeddingGemma Q4 |

Run your own: `uteke bench --counts 100,1000,10000 --json`

## Read More

- 📝 [Blog: Choosing an AI Memory Layer in 2026](/blog/comparison-2026) — Full evaluation framework
- 🏠 [Rooms](/rooms) — Multi-agent shared memory
- 🚀 [Quick Start](/getting-started) — Try Uteke in 30 seconds
- 📊 [Benchmarks](/benchmarks) — Detailed performance numbers
