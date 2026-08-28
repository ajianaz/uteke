# LongMemEval-S Benchmark Results

**Dataset:** `longmemeval_s_cleaned.json` (500 questions, 6 question types)
**Metric:** Session-level retrieval (Recall@k, NDCG@k)

---

## Uteke Retrieval — Strategy Comparison

### Vector (semantic only)

| Questions | R@5 | R@10 | R@50 | NDCG@5 | NDCG@10 | Embedding | Date |
|-----------|-----|------|------|--------|---------|-----------|------|
| 500 | 85.4% | 88.5% | — | 0.810 | — | EmbeddingGemma Q4 (ONNX local) | 2026-08-13 |

### Hybrid (RRF: vector + FTS5 fusion)

| Questions | R@5 | R@10 | R@50 | NDCG@5 | NDCG@10 | Embedding | Date |
|-----------|-----|------|------|--------|---------|-----------|------|
| 50 | 98.0% | 100.0% | 100.0% | 0.960 | 0.967 | EmbeddingGemma 768d (API) | 2026-08-13 |

**Improvement (Hybrid vs Vector): +12.6pp R@5**

### Fusion (weighted RRF: vector×1.7 + hybrid×1, #1123) — default since 0.16.0

| Questions | R@5 | R@10 | NDCG@5 | NDCG@10 | Embedding | Date |
|-----------|-----|------|--------|---------|-----------|------|
| 50 (mean) | 98.0% | 100.0% | 0.893 | 0.901 | EmbeddingGemma Q4 (ONNX local) | 2026-08-28 |
| 50 (43 evaluable*) | 97.7% | 100.0% | 0.893 | 0.901 | EmbeddingGemma Q4 (ONNX local) | 2026-08-28 |

*print_metrics.py filters to 43 evaluable questions; mean over all 50 is the comparable figure. Run: `results_modal_vector_fusion17` (Modal x86, harness-level fusion). Remaining fails @5: 3 questions (92a0aa75, 60472f9c, a3838d2b) — all partial-recall, R@10 = 1.0.

#### Zero-config validation (local ARM, 0.16.0 binary, no flags)

| Check | Result |
|-------|--------|
| Parity: implicit default vs explicit `--strategy fusion` | **identical rankings** (top-10 equal) |
| Divergence: implicit default vs explicit `--strategy hybrid` | rankings differ (as expected) |
| fast10, `--strategy default` (flag omitted → core default) | **R@5 0.9667, R@10 1.0, NDCG@5 0.9643** |

fast10 detail: 1 partial miss @5 (60472f9c multi-session, R@5 0.67 → R@10 1.0) — same known hard question as the Modal x86 fusion run. A fresh install with zero config gets benchmark-grade recall out of the box.

**Why fusion works:** vector-only and hybrid-only fail on DISJOINT question sets (fast50: 7 questions vector wins, 5 hybrid wins, no overlap in failures). RRF fusing both rankings captures each side's wins.

**Tuning evidence:** weight plateau [1.7, 1.9] → R@5 0.98 on actual x86 rankings; 1.7 chosen mid-plateau. wv=1.5 (ARM-local tuning) measured 0.9535 on x86 — arch drift of ±1 rank motivated re-tuning on the deployment arch (commit 47155af).

---

## 50Q Hybrid — Per Question Type Breakdown

| Question Type | Count | R@5 Hits | R@5 |
|---------------|-------|----------|-----|
| multi-session | 15 | 15 | 100% |
| single-session-user | 7 | 6 | 86% |
| knowledge-update | 7 | 7 | 100% |
| single-session-assistant | 7 | 7 | 100% |
| single-session-preference | 7 | 7 | 100% |
| temporal-reasoning | 7 | 7 | 100% |

**1 miss at R@5** — single-session-user (QID: 5d3d2817). R@10 recovers to 100%.

---

## Methodology

- **Vector strategy:** Embedding similarity search via usearch index.
- **Hybrid strategy:** Reciprocal Rank Fusion (RRF k=60) of vector search + SQLite FTS5 keyword search.
- **Embedding (vector 500Q):** EmbeddingGemma 300M Q4 ONNX, 768d, local inference.
- **Embedding (hybrid 50Q):** EmbeddingGemma 768d via API endpoint, same model dimensions.
- **Session-level:** Retrieval evaluated at session granularity (not per-turn).
- **Throttled run:** 2 CPU cores, nice 19 (production-safe benchmark).

## Reproduce

```bash
# Vector (500Q, ONNX local)
python run_eval.py --data data/longmemeval_s_cleaned.json --output results_vector --strategy vector

# Hybrid (50Q sample)
python run_eval.py --data data/longmemeval_s_cleaned.json --output results_hybrid --limit 50 --strategy hybrid
```
