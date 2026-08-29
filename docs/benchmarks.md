---
title: Benchmarks
---

# Benchmarks

Real numbers from `uteke bench` on Oracle Cloud ARM (Ampere A1, 4 vCPU, 24GB RAM).
Embedding model: EmbeddingGemma Q4 (768d, ONNX Runtime, CPU-only).

## Results

| Scale | Insert ops/s | Insert Total | Recall Avg | Recall P95 | DB Size | Index Size |
|-------|-------------|-------------|------------|------------|---------|------------|
| 100 memories | 18.5/s | 5.4s | **40ms** | 46ms | 708KB | 319KB |
| 1,000 memories | 21.8/s | 45.9s | **45ms** | 51ms | 5.3MB | 3.2MB |
| 10,000 memories | 6.0/s | 28.0 min | **42ms** | 50ms | 81.3MB | 30.3MB |

## Key Takeaways

### Recall Latency: Flat ~40-45ms from 100 to 10K memories

The killer stat: recall latency barely changes as the store grows.

- 100 memories → **40ms**
- 1,000 memories → **45ms**
- 10,000 memories → **42ms** ← actually *faster* than 1K (warm ONNX cache)

HNSW search is O(log N), so even at 10K memories, the vector index adds <1ms.
The ~40ms floor is dominated by ONNX embedding inference, not search.

The full pipeline (fusion strategy, default since 0.16.0):
1. Query → ONNX embedding generation
2. HNSW vector search → vector ranking
3. FTS5 full-text search + RRF (k=60) → hybrid ranking
4. Weighted RRF fusion of the two rankings (#1123, tuned weights)

Retrieval quality (LongMemEval fast50, session-level): fusion R@5 0.98
vs hybrid 0.9267 vs vector-only 0.854. Vector and hybrid fail on
disjoint question sets — fusing captures both sides' wins.

No network round-trip. No API call. Everything in-process.

### Insert Throughput: 6-22 ops/s (CPU-bound)

Each insert requires an ONNX embedding pass (CPU inference). Throughput drops at scale because HNSW graph traversal grows as the index expands:

- 100 memories → **18.5 ops/s**
- 1,000 memories → **21.8 ops/s**
- 10,000 memories → **6.0 ops/s**

At 6 ops/s, inserting 10K memories takes ~28 minutes. For bulk ingestion, use `uteke import` (batch mode) which pipelines embeddings.

### Storage Efficiency

- 100 memories → 708KB DB + 319KB index = **~10KB per memory**
- 1,000 memories → 5.3MB DB + 3.2MB index = **~8.5KB per memory**
- 10,000 memories → 81.3MB DB + 30.3MB index = **~11.2KB per memory**

Storage scales linearly (~10KB/memory). SQLite + HNSW both grow predictably.

## How to Reproduce

```bash
uteke bench --counts 100,1000,10000 --json
```

Or with a custom store path:

```bash
uteke bench --counts 100,1000 --store /tmp/bench --json
```

## External Evaluation

See [LongMemEval retrieval harness](https://github.com/codecoradev/uteke/tree/develop/benchmarks/longmemeval) for accuracy evaluation against standard benchmarks.

## LongMemEval-S — Retrieval Accuracy (500 questions)

Full validation run of uteke v0.16.0 default strategy (fusion, zero-config) on
LongMemEval-S: 500 questions, session-level retrieval, ~115 haystack sessions per
question (2,415 unique sessions), EmbeddingGemma Q4 CPU-only, deterministic — no
LLM anywhere in the retrieval path.

![uteke vs published systems on LongMemEval-S](assets/longmemeval-comparison.jpg)

### Headline numbers

| Metric | Value | What it means |
|---|---|---|
| **recall_any@5** | **98.2%** | At least one gold session in top-5 — the metric competitor benchmarks publish |
| recall_any@10 | 98.8% | |
| recall_all@10 | 95.4% | **Strict:** every gold session must appear in top-10 |
| strict recall_all@5 | 88.0% | Every gold session in top-5 (mathematical ceiling 99.4% — 3 questions have 6 gold sessions) |
| coverage@5 | 94.3% | Partial credit per question (the harness's default aggregate) |

Gold-session distribution across the 500 questions: 1 gold ×176, 2 ×250, 3 ×41,
4 ×19, 5 ×11, 6 ×3. 43% of questions are multi-session — which is why we report
the strict family at all.

### Why two metric families

`recall_any@K` passes a question when *at least one* gold session is retrieved.
It is the de-facto industry metric — and the one every competitor number in the
chart above uses. But a question whose answer needs evidence from 3 sessions is
only truly solved when **all 3** are retrieved. `recall_all@K` measures exactly
that. It is harder, bounded below recall_any, and to our knowledge no other
system in the comparison publishes it. We report both, from the same run, with
the same data.

### Honesty notes

- The comparison chart mixes evaluation setups: uteke numbers come from our own
  harness on `longmemeval-s` (cleaned set); competitor numbers are from their
  published benchmark documents (accessed Aug 2026) and differ in embedding
  models and pipeline details.
- The FTS5-only bar is an ablation of our own system, not a competitor.
- Raw per-question results for the uteke run: `benchmarks/longmemeval/results_modal_default/` in this repo.

## Environment

| Component | Details |
|-----------|---------|
| Hardware | Oracle Cloud ARM (Ampere A1, 4 vCPU, 24GB RAM) |
| OS | Linux 6.8.0 (aarch64) |
| Rust | 1.85+ |
| Embedding | EmbeddingGemma Q4, 768d, ONNX Runtime CPU |
| Uteke | v0.12.0 |

## Methodology

The benchmark uses `uteke bench` which:
1. Generates deterministic synthetic memories (seeded PRNG)
2. Inserts them one-by-one with embedding
3. Runs recall queries at each scale
4. Measures wall-clock time for insert and recall
5. Reports ops/s, latency percentiles, and storage footprint

No external services. No network. No Docker. Just the binary.
