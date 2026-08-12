#!/usr/bin/env python3
"""
LongMemEval retrieval evaluation harness for uteke.

Measures how well uteke recalls evidence sessions/turns for each question.

Supports two modes:
  --precompute-embeddings: Pre-compute embeddings via an OpenAI-compatible
                           API before import. Uteke imports the pre-computed
                           vectors and skips local embedding entirely.
                           This is dramatically faster for large datasets.

Usage:
    # Standard mode (local embedding)
    python run_eval.py --data data/longmemeval_oracle.json --output results/

    # Pre-computed mode (remote embedding API)
    python run_eval.py --data data/longmemeval_s_cleaned.json \
        --output results/ \
        --precompute-embeddings \
        --embed-api-base https://your-embed-endpoint.example.com \
        --embed-api-key YOUR_KEY \
        --embed-model gemma-768
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

try:
    import urllib.request
    import urllib.error
except ImportError:
    pass

try:
    from tqdm import tqdm
except ImportError:
    def tqdm(x, **kwargs):
        return x


# ========= Embedding API Client =========

def batch_embed_texts(texts, api_base, api_key, model, batch_size=50):
    """
    Call an OpenAI-compatible /v1/embeddings endpoint to embed texts in batches.

    Args:
        texts: List of strings to embed.
        api_base: Base URL (e.g. https://host.example.com — no trailing slash).
        api_key: Bearer token for auth.
        model: Model name for the API.
        batch_size: Texts per HTTP request.

    Returns:
        List of embedding vectors (list[list[float]]), same order as input.
    """
    all_vectors = []
    url = f"{api_base.rstrip('/')}/v1/embeddings"

    for i in range(0, len(texts), batch_size):
        chunk = texts[i : i + batch_size]
        body = json.dumps({"model": model, "input": chunk}).encode("utf-8")
        req = urllib.request.Request(
            url,
            data=body,
            headers={
                "Content-Type": "application/json",
                "Authorization": f"Bearer {api_key}",
            },
        )
        for attempt in range(3):
            try:
                with urllib.request.urlopen(req, timeout=300) as resp:
                    data = json.loads(resp.read())
                    # Sort by index to guarantee order
                    items = sorted(data["data"], key=lambda d: d["index"])
                    all_vectors.extend(d["embedding"] for d in items)
                    break
            except Exception as e:
                if attempt == 2:
                    print(f"  Embedding API error (batch {i}): {e}", file=sys.stderr)
                    raise
                time.sleep(2 ** attempt)

    return all_vectors


def session_to_text(session):
    """Convert a chat session (list of turns) into plain text."""
    lines = []
    for turn in session:
        role = turn.get("role", "unknown")
        content = turn.get("content", "")
        lines.append(f"{role}: {content}")
    return "\n".join(lines)


def turn_has_answer(turn):
    """Check if a turn is marked as containing the answer."""
    return turn.get("has_answer", False)


def run_uteke(args, store_path, subcommand, extra_args=None):
    """Run a uteke CLI command."""
    cmd = [
        "uteke",
        "--store", str(store_path),
        "--namespace", args.namespace,
        "--json",
    ] + subcommand
    if extra_args:
        cmd += extra_args
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=600)
    if result.returncode != 0:
        raise RuntimeError(f"uteke failed: {' '.join(cmd)}\nstderr: {result.stderr}")
    return result.stdout.strip()


def insert_sessions(args, store_path, entry):
    """
    Insert all haystack sessions for one question into uteke via batch JSONL import.
    This spawns ONE uteke process (one model load) instead of N individual `remember` calls.

    When --precompute-embeddings is enabled, session texts are embedded via the
    configured embedding API before import, and the JSONL includes pre-computed
    vectors. Uteke then imports without calling its local embedder.

    Returns: (set of successfully inserted session_ids,
              dict session_id -> set of turn indices that has answers,
              dict memory_id -> session_id).
    """
    session_ids = entry.get("haystack_session_ids", [])
    sessions = entry.get("haystack_sessions", [])
    dates = entry.get("haystack_dates", [])

    answer_turns = {}  # session_id -> set of turn indices with has_answer
    inserted_sids = set()  # track which sessions actually inserted
    mid_to_sid = {}  # memory_id -> session_id mapping

    # Build JSONL file for batch import
    import tempfile
    jsonl_fd, jsonl_path = tempfile.mkstemp(suffix=".jsonl", prefix="uteke-import-")
    try:
        import os as _os
        with _os.fdopen(jsonl_fd, 'w') as f:
            # Convert sessions to text first
            texts = []
            metas = []
            for i, (sid, session) in enumerate(zip(session_ids, sessions)):
                text = session_to_text(session)
                date = dates[i] if i < len(dates) else None
                meta = {"session_id": sid}
                if date:
                    meta["date"] = date
                texts.append(text)
                metas.append(meta)

            # Pre-compute embeddings via API if enabled
            if args.precompute_embeddings:
                t0 = time.time()
                vectors = batch_embed_texts(
                    texts,
                    args.embed_api_base,
                    args.embed_api_key,
                    args.embed_model,
                    batch_size=args.embed_batch_size,
                )
                embed_elapsed = time.time() - t0
                print(f"  Embedded {len(vectors)} sessions in {embed_elapsed:.1f}s", file=sys.stderr)
            else:
                vectors = [None] * len(texts)

            # Write JSONL
            for text, meta, vector in zip(texts, metas, vectors):
                record = {
                    "content": text,
                    "tags": ["longmemeval"],
                    "type": "context",
                    "metadata": meta,
                }
                if vector is not None:
                    record["embedding"] = vector
                f.write(json.dumps(record) + "\n")

        # Single batch import — one model load for ALL sessions
        try:
            stdout = run_uteke(args, store_path, [
                "import", jsonl_path,
                "--format", "jsonl",
            ])

            # Import returns {"imported": N, "skipped": M}
            # We don't get individual IDs, so we accept all session_ids as inserted
            # Session_id mapping is done via metadata in recall results
            try:
                import_data = json.loads(stdout)
                imported_count = import_data.get("imported", 0) if isinstance(import_data, dict) else 0
            except (json.JSONDecodeError, KeyError):
                imported_count = 0

            # Only mark as inserted if import succeeded
            if imported_count > 0:
                for sid in session_ids:
                    inserted_sids.add(sid)

        except (RuntimeError, subprocess.TimeoutExpired) as e:
            print(f"  Warning: batch import failed/timed out: {e}", file=sys.stderr)
    finally:
        import os as _os
        _os.unlink(jsonl_path)

    # Track answer turns
    for sid, session in zip(session_ids, sessions):
        answer_indices = set()
        for j, turn in enumerate(session):
            if turn_has_answer(turn):
                answer_indices.add(j)
        if answer_indices:
            answer_turns[sid] = answer_indices

    return inserted_sids, answer_turns, mid_to_sid


def recall_and_evaluate(args, store_path, entry, answer_sessions, inserted_sids, mid_to_sid):
    """
    Run uteke recall for the question, then evaluate retrieval accuracy.

    Returns dict with retrieval metrics for this question.
    """
    question = entry["question"]
    # Only count evidence sessions that were actually inserted (avoid false negatives
    # from insert failures).
    evidence_session_ids = set(entry.get("answer_session_ids", [])) & inserted_sids

    # Run recall — fetch top-50 for Recall@5/10/50
    try:
        output = run_uteke(args, store_path, [
            "recall", question,
            "--limit", "50",
            "--tags", "longmemeval",
            "--min", "0.0",  # Disable threshold — evaluate raw retrieval ranking (#995)
        ])
    except (RuntimeError, subprocess.TimeoutExpired) as e:
        print(f"  Warning: recall failed/timed out: {e}", file=sys.stderr)
        return None

    # Parse results
    try:
        results = json.loads(output)
    except json.JSONDecodeError:
        print(f"  Warning: could not parse recall output", file=sys.stderr)
        return None

    if not isinstance(results, list):
        results = [results]

    # Extract session_ids from recall metadata.
    # Uteke recall returns memory_id + metadata.session_id for each result.
    # We also fall back to mid_to_sid for older versions.
    retrieved_session_ids = []
    for r in results:
        # Primary: read session_id directly from metadata
        meta = r.get("metadata", {})
        sid = meta.get("session_id")
        if sid:
            retrieved_session_ids.append(sid)
            continue
        # Fallback: memory_id -> session_id mapping from insert
        mid = r.get("memory_id") or r.get("id")
        if mid and mid in mid_to_sid:
            retrieved_session_ids.append(mid_to_sid[mid])

    # --- Session-level metrics ---
    # Recall@k: fraction of evidence sessions in top-k
    session_recall = {}
    for k in [5, 10, 50]:
        top_k = retrieved_session_ids[:k]
        hits = len(set(top_k) & evidence_session_ids)
        total_evidence = len(evidence_session_ids) if evidence_session_ids else 1
        session_recall[f"recall_all@{k}"] = hits / total_evidence if total_evidence > 0 else 0.0

    # NDCG@k for sessions
    session_ndcg = {}
    for k in [5, 10, 50]:
        top_k = retrieved_session_ids[:k]
        # Binary relevance: 1 if evidence session, 0 otherwise
        gains = [1.0 if sid in evidence_session_ids else 0.0 for sid in top_k]
        discounts = [1.0 / np_log2(i + 2) for i in range(len(gains))]
        dcg = sum(g * d for g, d in zip(gains, discounts))

        # Ideal DCG: all relevant items at top
        ideal_len = min(len(evidence_session_ids), len(top_k))
        ideal_gains = [1.0] * ideal_len
        ideal_discounts = [1.0 / np_log2(i + 2) for i in range(len(ideal_gains))]
        idcg = sum(g * d for g, d in zip(ideal_gains, ideal_discounts))

        session_ndcg[f"ndcg_any@{k}"] = dcg / idcg if idcg > 0 else 0.0

    return {
        "session": {**session_recall, **session_ndcg},
        # Note: turn-level retrieval requires per-turn indexing, which this
        # harness does not do (sessions are inserted as single memories).
        # Turn-level metrics are omitted to avoid misleading copies of
        # session-level numbers.
    }


def np_log2(x):
    """Compute log2 without numpy dependency for this helper."""
    if x <= 0:
        return 0.0
    import math
    return math.log2(x)


def main():
    parser = argparse.ArgumentParser(description="LongMemEval retrieval eval for uteke")
    parser.add_argument("--data", required=True, help="Path to longmemeval JSON file")
    parser.add_argument("--output", default="results", help="Output directory")
    parser.add_argument("--namespace", default="lmeval", help="Uteke namespace")
    parser.add_argument("--limit", type=int, default=0, help="Limit questions (0 = all)")
    parser.add_argument("--keep-store", action="store_true",
                        help="Keep the uteke store after eval (for debugging)")
    parser.add_argument("--resume", action="store_true",
                        help="Resume from existing results file (skip already-evaluated questions)")
    parser.add_argument("--reset-every", type=int, default=20,
                        help="Wipe and recreate the store every N questions to prevent memory buildup (default: 20)")

    # Pre-computed embeddings via external API
    parser.add_argument("--precompute-embeddings", action="store_true",
                        help="Pre-compute embeddings via an OpenAI-compatible API before import")
    parser.add_argument("--embed-api-base", default=os.environ.get("EMBED_API_BASE", ""),
                        help="Embedding API base URL (e.g. https://host.example.com)")
    parser.add_argument("--embed-api-key", default=os.environ.get("EMBED_API_KEY", ""),
                        help="Bearer token for the embedding API")
    parser.add_argument("--embed-model", default=os.environ.get("EMBED_MODEL", "gemma-768"),
                        help="Model name for the embedding API")
    parser.add_argument("--embed-batch-size", type=int, default=50,
                        help="Number of texts per embedding API request (default: 50)")

    # Validate pre-compute args
    args = parser.parse_args()
    if args.precompute_embeddings:
        if not args.embed_api_base:
            parser.error("--embed-api-base is required when --precompute-embeddings is set")
        if not args.embed_api_key:
            parser.error("--embed-api-key is required when --precompute-embeddings is set")

    # Load data
    with open(args.data) as f:
        data = json.load(f)

    if args.limit > 0:
        data = data[:args.limit]

    print(f"LongMemEval retrieval evaluation")
    print(f"  Questions: {len(data)}")
    print(f"  Namespace: {args.namespace}")
    if args.precompute_embeddings:
        print(f"  Mode: pre-computed embeddings via {args.embed_api_base}")
        print(f"  Embed model: {args.embed_model}, batch size: {args.embed_batch_size}")
    else:
        print(f"  Mode: local embedding (ONNX)")
    print()

    # Create temp store
    store_path = Path(tempfile.mkdtemp(prefix="uteke-lmeval-"))
    print(f"Store: {store_path}")

    os.makedirs(args.output, exist_ok=True)
    results_file = Path(args.output) / "retrieval_results.jsonl"

    # Resume support: load already-evaluated question IDs
    done_ids = set()
    if args.resume and results_file.exists():
        with open(results_file) as f:
            for line in f:
                try:
                    entry = json.loads(line.strip())
                    done_ids.add(entry.get("question_id"))
                except json.JSONDecodeError:
                    pass
        if done_ids:
            print(f"Resume: {len(done_ids)} questions already evaluated, skipping...")

    total_start = time.time()
    evaluated = 0

    # Open in append mode for resume, write mode for fresh run
    mode = "a" if args.resume and done_ids else "w"
    with open(results_file, mode) as fout:
        for idx, entry in enumerate(tqdm(data, desc="Evaluating")):
            qid = entry.get("question_id", f"q{idx}")

            # Skip if already evaluated (resume mode)
            if qid in done_ids:
                continue

            # Periodic store reset to prevent memory buildup
            if evaluated > 0 and evaluated % args.reset_every == 0:
                shutil.rmtree(store_path, ignore_errors=True)
                store_path.mkdir(parents=True, exist_ok=True)

            # Insert sessions
            inserted_sids, answer_sessions, mid_to_sid = insert_sessions(args, store_path, entry)

            # Recall + evaluate
            metrics = recall_and_evaluate(args, store_path, entry, answer_sessions, inserted_sids, mid_to_sid)

            if metrics is not None:
                result_entry = {
                    "question_id": qid,
                    "question_type": entry.get("question_type", "unknown"),
                    "retrieval_results": {"metrics": metrics},
                }
                fout.write(json.dumps(result_entry) + "\n")
                fout.flush()  # Flush for resume safety

            evaluated += 1

            # Clean up memories for this question (avoid cross-contamination).
            # If forget fails, wipe the entire store to guarantee a clean slate.
            try:
                run_uteke(args, store_path, ["forget", "--all", "--confirm"])
            except (RuntimeError, subprocess.TimeoutExpired):
                print(f"  Warning: forget --all failed; removing store to reset", file=sys.stderr)
                shutil.rmtree(store_path, ignore_errors=True)
                store_path.mkdir(parents=True, exist_ok=True)

    elapsed = time.time() - total_start
    print(f"\nDone in {elapsed:.1f}s")
    print(f"Results saved to {results_file}")
    print(f"\nRun: python print_metrics.py {results_file}")

    # Cleanup
    if not args.keep_store:
        shutil.rmtree(store_path, ignore_errors=True)


if __name__ == "__main__":
    main()
