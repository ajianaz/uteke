# Benchmark Results

## Performance Benchmark (`uteke bench`)

Run `uteke bench` to reproduce. Results below are indicative — actual numbers depend on hardware.

| Memories | Insert (ops/sec) | Recall avg (ms) | Recall p95 (ms) | DB size | Index size |
|----------|-----------------|-----------------|-----------------|---------|------------|
| 100      | ~800            | ~0.3            | ~0.5            | ~45 KB  | ~12 KB     |
| 1,000    | ~800            | ~1.2            | ~2.0            | ~450 KB | ~120 KB    |
| 10,000   | ~750            | ~5.0            | ~8.0            | ~4.5 MB | ~1.2 MB    |

Benchmarked on Oracle Cloud ARM (Ampere Altra), CPU-only.

## LongMemEval Retrieval Accuracy

Run `cd benchmarks/longmemeval && ./download_data.sh && python run_eval.py --data data/longmemeval_oracle.json` to reproduce.

### Uteke v0.13.2 (EmbeddingGemma Q4, 768d, local ONNX)

#### Oracle Dataset — 500 Questions (Evidence-only haystack)

Evaluates retrieval when the haystack contains only the evidence sessions (ideal case).

| Question Type | Count | Recall@5 | Recall@10 | NDCG@5 | NDCG@10 |
|---------------|-------|----------|-----------|--------|---------|
| **Overall** | **470** | **0.986** | **0.987** | **0.987** | **0.987** |
| knowledge-update | 72 | 0.972 | 0.972 | 0.972 | 0.972 |
| multi-session | 121 | 0.983 | 0.983 | 0.983 | 0.983 |
| single-session-assistant | 56 | 1.000 | 1.000 | 1.000 | 1.000 |
| single-session-preference | 30 | 1.000 | 1.000 | 1.000 | 1.000 |
| single-session-user | 64 | 1.000 | 1.000 | 1.000 | 1.000 |
| temporal-reasoning | 127 | 0.980 | 0.984 | 0.984 | 0.984 |

> 30/500 questions skipped due to eval harness edge cases (not uteke failures).
> Runtime: ~74 minutes (~8.9s/question). Embeddings pre-computed via batch API, recall via local ONNX.

#### S Dataset — 500 Questions (Full ~50-session haystack)

Evaluates retrieval against a realistic haystack of ~50 sessions per question (~115K tokens context). This is the standard LongMemEval-S configuration.

| Question Type | Count | Recall@5 | Recall@10 | NDCG@5 | NDCG@10 |
|---------------|-------|----------|-----------|--------|---------|
| **Overall** | **470** | **0.854** | **0.885** | **0.810** | **0.823** |
| knowledge-update | 72 | 0.882 | 0.889 | 0.858 | 0.861 |
| multi-session | 121 | 0.833 | 0.896 | 0.801 | 0.830 |
| single-session-assistant | 56 | 0.964 | 0.964 | 0.964 | 0.964 |
| single-session-preference | 30 | 0.800 | 0.800 | 0.734 | 0.734 |
| single-session-user | 64 | 0.875 | 0.891 | 0.809 | 0.814 |
| temporal-reasoning | 127 | 0.811 | 0.853 | 0.741 | 0.757 |

> 30/500 questions skipped due to eval harness edge cases (not uteke failures).
> Runtime: ~4 hours 21 minutes (~31s/question). Embeddings pre-computed via batch API, recall via local ONNX.

#### Oracle vs S Dataset — Difficulty Comparison

| Metric | Oracle (evidence-only) | S Dataset (~50 sessions) | Delta |
|--------|:---------------------:|:------------------------:|:-----:|
| Recall@5 | 98.6% | 85.4% | -13.2pp |
| Recall@10 | 98.7% | 88.5% | -10.2pp |
| Sessions/Question | 2-3 | ~48 | ~20× harder |

### Comparison with Other Memory Systems

| System | LongMemEval Score | Embedding Model | Notes |
|--------|-------------------|-----------------|-------|
| Hindsight | 94.6% | Proprietary | Commercial |
| Mem0 v3 (Pro) | 91.6% | Proprietary | Commercial |
| Mem0 (Free) | 49.0% | Proprietary | Open source |
| **Uteke v0.13.2 (Oracle)** | **R@5: 98.6%** | EmbeddingGemma 300M Q4 | Open source, zero-dep |
| **Uteke v0.13.2 (S)** | **R@5: 85.4%** | EmbeddingGemma 300M Q4 | Open source, zero-dep |

> **Note**: LongMemEval scores from other systems are answer-correctness scores (using GPT-4o as judge), while uteke's harness measures retrieval accuracy. The two are correlated but not directly comparable. We report retrieval metrics (Recall@k, NDCG@k) for transparency.
