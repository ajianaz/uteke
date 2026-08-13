# Benchmark Results

## Performance Benchmark (`uteke bench`)

Run `uteke bench` to reproduce. Results below are indicative — actual numbers depend on hardware.

| Memories | Insert (ops/sec) | Recall avg (ms) | Recall p95 (ms) | DB size | Index size |
|----------|-----------------|-----------------|-----------------|---------|------------|
| 100      | ~800            | ~0.3            | ~0.5            | ~45 KB  | ~12 KB     |
| 1,000    | ~800            | ~1.2            | ~2.0            | ~450 KB | ~120 KB    |
| 10,000   | ~750            | ~5.0            | ~8.0            | ~4.5 MB | ~1.2 MB    |

Benchmarked on Oracle Cloud ARM (Ampere Altra), CPU-only.

## LongMemEval-S Retrieval Accuracy

Dataset: LongMemEval-S (500 questions, multi-session chatbot memory).
Run `cd benchmarks/longmemeval && ./download_data.sh && python run_eval.py` to reproduce.

### Strategy Comparison

| Strategy | Questions | R@5 | R@10 | NDCG@5 | Embedding | Date |
|----------|-----------|-----|------|--------|-----------|------|
| **Vector** | 500 | 85.4% | 88.5% | 0.810 | EmbeddingGemma Q4 (ONNX local) | 2026-08-13 |
| **Hybrid (RRF)** | 50 | 98.0% | 100.0% | 0.960 | EmbeddingGemma 768d (API) | 2026-08-13 |

- **Vector**: Semantic similarity search using embeddings only.
- **Hybrid (RRF)**: Reciprocal Rank Fusion of vector search + FTS5 keyword search. Default strategy.
- RRF k=60, consistent across all code paths.
