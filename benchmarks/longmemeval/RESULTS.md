# LongMemEval Benchmark Results

**Uteke version:** 0.13.2
**Embedding model:** EmbeddingGemma 300M Q4 (768d, local ONNX)
**Date:** 2026-08-12

## Oracle Dataset — 500 Questions

**Dataset:** `longmemeval_oracle.json` — evidence sessions only (ideal retrieval).

| Question Type | Count | Recall@5 | Recall@10 | NDCG@5 | NDCG@10 |
|---------------|-------|----------|-----------|--------|---------|
| **Overall** | **470** | **0.986** | **0.987** | **0.987** | **0.987** |
| knowledge-update | 72 | 0.972 | 0.972 | 0.972 | 0.972 |
| multi-session | 121 | 0.983 | 0.983 | 0.983 | 0.983 |
| single-session-assistant | 56 | 1.000 | 1.000 | 1.000 | 1.000 |
| single-session-preference | 30 | 1.000 | 1.000 | 1.000 | 1.000 |
| single-session-user | 64 | 1.000 | 1.000 | 1.000 | 1.000 |
| temporal-reasoning | 127 | 0.980 | 0.984 | 0.984 | 0.984 |

Runtime: ~74 min (~8.9s/question). 30/500 skipped (eval harness edge cases).

## S Dataset — 500 Questions

**Dataset:** `longmemeval_s_cleaned.json` — full ~50-session haystack (~115K tokens).

| Question Type | Count | Recall@5 | Recall@10 | NDCG@5 | NDCG@10 |
|---------------|-------|----------|-----------|--------|---------|
| **Overall** | **470** | **0.854** | **0.885** | **0.810** | **0.823** |
| knowledge-update | 72 | 0.882 | 0.889 | 0.858 | 0.861 |
| multi-session | 121 | 0.833 | 0.896 | 0.801 | 0.830 |
| single-session-assistant | 56 | 0.964 | 0.964 | 0.964 | 0.964 |
| single-session-preference | 30 | 0.800 | 0.800 | 0.734 | 0.734 |
| single-session-user | 64 | 0.875 | 0.891 | 0.809 | 0.814 |
| temporal-reasoning | 127 | 0.811 | 0.853 | 0.741 | 0.757 |

Runtime: ~4h 21min (~31s/question). 30/500 skipped (eval harness edge cases).

## Notes

- **Oracle vs S dataset:** The 13.2pp recall drop from Oracle to S is expected — the S dataset's haystack is ~20× larger (48 vs 2-3 sessions), making retrieval significantly harder.
- **Session-level metrics only.** Turn-level retrieval requires per-turn indexing (not measured by this harness).
- **knowledge-update** remains the hardest type in Oracle (97.2%), but interestingly performs well in S (88.2%) — likely because updated information tends to be distinctive.
- **temporal-reasoning** and **single-session-preference** are the weakest categories in S — see analysis below.
