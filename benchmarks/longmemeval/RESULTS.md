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
