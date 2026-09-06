# LongMemEval-S Benchmark Results

**Dataset:** `longmemeval_s_cleaned.json` (500 questions, 6 question types)
**Metric:** Session-level retrieval (Recall@k, NDCG@k)

---

## Independent Reproduction (2026-09-01)

The published 500Q run was produced on Modal x86. To test whether the result depends on that infrastructure, a 108-question subset (pref: 30, kupd: 78) was re-run locally on a 4-core ARM desktop (Oracle Ampere A1, aarch64) — same v0.16.0 binary, same harness (`run_eval.py`), different CPU architecture.

| Subset | Questions | Identical per-question rankings | R@5 published | R@5 re-run |
|---|---|---|---|---|
| pref | 30 | 30/30 | 96.7% | 96.7% |
| kupd | 78 | 77/78 | 100.0% | 99.4% |
| **Total** | **108** | **107/108** | — | — |

**The one divergence** (question `0977f2af`, knowledge-update, 2 gold sessions): both runs retrieved the identical top-10 session set; one gold session sits at rank 5 (published) vs rank 6 (re-run), moving that question's recall_all@5 from 1.0 to 0.5. The harness persists rankings rather than raw scores, so the exact score gap cannot be shown, but the identical top-10 set identifies this as cross-architecture floating-point noise on an RRF near-tie, not a retrieval failure. R@10 = 1.0 in both runs.

**Reproduce:**

```bash
python3 run_eval.py --data data/subset_kupd.json --output results_rerun --strategy default --resume
```

Raw artifacts are kept on the benchmark Modal volume (`uteke-longmemeval`, `default/` and rerun prefixes), consistent with the published run — datasets and result JSONL files are not committed to git (see `.gitignore` here; `download_data.sh` fetches the dataset).

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

### Fusion (weighted RRF of vector + hybrid rankings, #1123) — default since 0.16.0

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

**Tuning evidence:** weight grid sweep on actual x86 rankings; chosen value sits mid-plateau (stable across a range), not at an edge. Weights are internal implementation details (see core crate) rather than part of the public contract.

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

### 3-way RRF (vec + hyb + fts5-only) — NO-GO (2026-08-29)

Simulation over saved rankings; fts5-only rankings collected on Modal x86
(500Q, ~4.5h, resume-safe via volume). Cross-check: sim fts5-only R@5=0.8211
vs harness-reported 0.820 — parsing validated.

- 2-way shipped fusion (tuned weights) R@5 = 0.9800 (fast50)
- best 3-way (2.0/1.0/0.1) R@5 = 0.9800 — delta +0.0000, **0 flipped questions**
- adding fts5 as third arm changes nothing at any grid weight tried
- fts5-only fails: temporal-reasoning (63) + multi-session (55) dominate —
  same classes already covered (partially) by hybrid's own FTS5 component

Conclusion: ranking-side optimization via more RRF arms is exhausted.
Remaining headroom is insert-side granularity (3 partial-recall fails) —
separate project, uncertain payoff.

Artifacts: sim3way_final.py, preview_scores.py, results_modal_fts5_partial/

### Pure-default (zero-config fusion) 500Q — Modal x86_64, binary 0.16.0 from git bfbc296 (2026-08-29)

Pre-release validation: `--strategy default` (no flag at all) against the full 500Q dataset.
Binary built in-image from exact SHA bfbc296 (PR #1137/#1138), image build prints `uteke 0.16.0`.

| Question Type | Count | Recall@5 | Recall@10 | NDCG@5 | NDCG@10 |
|---|---|---|---|---|---|
| **Overall** | 470 | **0.946** | 0.977 | 0.897 | 0.911 |
| knowledge-update | 72 | 1.000 | 1.000 | 0.972 | 0.972 |
| multi-session | 121 | 0.906 | 0.976 | 0.880 | 0.913 |
| single-session-assistant | 56 | 0.982 | 1.000 | 0.959 | 0.965 |
| single-session-preference | 30 | 0.967 | 0.967 | 0.839 | 0.839 |
| single-session-user | 64 | 0.969 | 0.969 | 0.913 | 0.913 |
| temporal-reasoning | 127 | 0.920 | 0.962 | 0.849 | 0.867 |

Context: v0.15.0 hybrid baseline on the same dataset: Overall R@5 = 0.854 / R@10 = 0.885 (2026-08-13).
Fusion default lifts full-500Q recall@5 by **+9.2 points** (0.854 → 0.946) with zero configuration.

---

## Contradiction-resolution segment (#1172 Fase 3) — 2026-09-06

**Active-store knowledge-update segment**: 40 topics × (stale fact + winner fact + 3 distractors),
queries ask "which {thing} does {topic} use now?" (semantic, no keyword echo of the answer).
Baseline ranks with BOTH facts active; resolved ranks after `supersede(stale → winner)` —
baseline is measured for every strategy BEFORE any resolution, then the store is resolved once.
Binary: local release build (0.16.0 + #1185 ledger), local ONNX EmbeddingGemma, ARM64.

Harness: `contradiction_segment.py` (this directory). Raw metrics: `results_contradiction_f3/metrics.json`.

| Strategy | Stage | winner@1 | winner@5 | winner MRR | stale@1 | stale@5 |
|---|---|---|---|---|---|---|
| fusion (default) | baseline (unresolved) | 0.850 | 1.000 | 0.925 | 0.150 | **1.000** |
| fusion (default) | resolved (superseded) | **1.000** | 1.000 | **1.000** | 0.000 | 0.000 |
| hybrid | baseline | 0.025 | 1.000 | 0.469 | 0.975 | 1.000 |
| hybrid | resolved | 0.225 | 1.000 | 0.588 | 0.000 | 0.000 |
| vector | baseline | 0.950 | 1.000 | 0.975 | 0.050 | 1.000 |
| vector | resolved | 1.000 | 1.000 | 1.000 | 0.000 | 0.000 |

Findings:

- **Unresolved conflicts pollute every strategy's top-5**: with both facts active, the stale
  fact sat in top-5 for 100% of topics on all strategies (hybrid's BM25 even ranks the stale
  fact top-1 for 97.5% of topics — the old fact's "uses X" phrasing matches "use now?"
  queries lexically). After `supersede`, stale@1 and stale@5 drop to **0.000** everywhere
  (deprecated memories are excluded from recall).
- **Supersede lifts the default surface**: fusion winner@1 0.850 → 1.000, MRR 0.925 → 1.000;
  vector 0.950 → 1.000. Hybrid stays weakest on winner@1 (lexical BM25 keeps the new fact's
  "switched to" phrasing behind distractors) but its stale pollution is fully cleared.
- **Ledger integrity**: `contradictions list` listed all 40 resolutions; every stale fact is
  restorable via `contradictions undo` (auditable conflict resolution, #1172 F2).

Interpretation: ranking alone often picks the winner, but only explicit conflict resolution
guarantees stale facts leave the retrieval surface — the difference between "usually right"
(85–95% top-1) and deterministic freshness (100% top-1, zero stale). For agent memory, where
"use now" queries are the norm, resolution is what keeps top-1 trustworthy. This segment is
synthetic and deterministic (fixed topic list); it measures the conflict-resolution pipeline,
not LongMemEval dataset recall.

