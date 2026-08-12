#!/usr/bin/env python3
"""
Parallel LongMemEval runner — spawns N workers, each handles a slice of questions
with its own independent uteke store. Writes results to shared JSONL (thread-safe append).
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
from concurrent.futures import ProcessPoolExecutor, as_completed
from multiprocessing import current_process


def session_to_text(session):
    lines = []
    for turn in session:
        lines.append(f"{turn.get('role','unknown')}: {turn.get('content','')}")
    return "\n".join(lines)


def run_uteke(store_path, namespace, subcommand, extra_args=None, timeout=600):
    cmd = [
        "uteke",
        "--store", str(store_path),
        "--namespace", namespace,
        "--json",
    ] + subcommand
    if extra_args:
        cmd += extra_args
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    return result.stdout


def eval_one_question(entry, worker_id, namespace='lmeval'):
    """Evaluate a single question. Returns result dict or None."""
    qid = entry['question_id']
    qtype = entry.get('question_type', 'unknown')
    question = entry['question']
    ev_ids = set(entry.get('answer_session_ids', []))
    session_ids = entry.get('haystack_session_ids', [])
    sessions = entry.get('haystack_sessions', [])
    dates = entry.get('haystack_dates', [])

    if not session_ids or not sessions:
        return None

    # Each worker gets its own store
    store_path = tempfile.mkdtemp(prefix=f'uteke-w{worker_id}-')
    jsonl_path = tempfile.mkstemp(suffix='.jsonl', prefix=f'uteke-imp-{qid}-')[1]

    try:
        # Build JSONL
        with open(jsonl_path, 'w') as f:
            for i, (sid, session) in enumerate(zip(session_ids, sessions)):
                text = session_to_text(session)
                meta = {"session_id": sid}
                if i < len(dates) and dates[i]:
                    meta["date"] = str(dates[i])
                record = {"content": text, "tags": ["longmemeval"], "type": "context", "metadata": meta}
                f.write(json.dumps(record) + "\n")

        # Import (batch)
        stdout = run_uteke(store_path, namespace, ['import', jsonl_path, '--format', 'jsonl'])
        import_data = json.loads(stdout)
        imported_count = import_data.get('imported', 0)
        if imported_count == 0:
            return None

        inserted_sids = set(session_ids)
        evidence_session_ids = ev_ids.intersection(inserted_sids)

        # Recall
        stdout = run_uteke(store_path, namespace, [
            'recall', question, '--limit', '50', '--tags', 'longmemeval', '--min', '0.0'
        ])
        results = json.loads(stdout)
        if not isinstance(results, list):
            results = [results]

        retrieved_session_ids = []
        for r in results:
            meta = r.get('metadata', {})
            sid = meta.get('session_id')
            if sid:
                retrieved_session_ids.append(sid)

        # Metrics
        session_recall = {}
        for k in [5, 10, 50]:
            top_k = retrieved_session_ids[:k]
            hits = len(set(top_k).intersection(evidence_session_ids))
            total_evidence = len(evidence_session_ids) if evidence_session_ids else 1
            session_recall[f'recall_all@{k}'] = hits / total_evidence if total_evidence > 0 else 0.0

        import math
        session_ndcg = {}
        for k in [5, 10, 50]:
            top_k = retrieved_session_ids[:k]
            gains = [1.0 if sid in evidence_session_ids else 0.0 for sid in top_k]
            discounts = [1.0 / math.log2(i + 2) for i in range(len(gains))]
            dcg = sum(g * d for g, d in zip(gains, discounts))
            ideal_len = min(len(evidence_session_ids), len(top_k))
            ideal_gains = [1.0] * ideal_len
            ideal_discounts = [1.0 / math.log2(i + 2) for i in range(len(ideal_gains))]
            idcg = sum(g * d for g, d in zip(ideal_gains, ideal_discounts))
            session_ndcg[f'ndcg_any@{k}'] = dcg / idcg if idcg > 0 else 0.0

        return {
            'question_id': qid,
            'question_type': qtype,
            'retrieval_results': {
                'metrics': {
                    'session': {**session_recall, **session_ndcg},
                }
            }
        }

    except Exception as e:
        print(f'  [W{worker_id}] ERROR qid={qid}: {e}', file=sys.stderr)
        return None
    finally:
        try:
            run_uteke(store_path, namespace, ['forget', '--all', '--confirm'], timeout=60)
        except Exception:
            pass
        shutil.rmtree(store_path, ignore_errors=True)
        if os.path.exists(jsonl_path):
            os.unlink(jsonl_path)


def worker_fn(args_tuple):
    """Wrapper for ProcessPoolExecutor."""
    entry, worker_id = args_tuple
    return eval_one_question(entry, worker_id)


def main():
    parser = argparse.ArgumentParser(description='Parallel LongMemEval runner')
    parser.add_argument('--data', required=True, help='Path to longmemeval JSON')
    parser.add_argument('--output', default='results', help='Output directory')
    parser.add_argument('--workers', type=int, default=4, help='Parallel workers')
    parser.add_argument('--limit', type=int, default=0, help='Limit questions (0=all)')
    args = parser.parse_args()

    with open(args.data) as f:
        data = json.load(f)

    if args.limit > 0:
        data = data[:args.limit]

    os.makedirs(args.output, exist_ok=True)
    results_file = Path(args.output) / 'retrieval_results.jsonl'

    # Resume support
    done_ids = set()
    if results_file.exists():
        with open(results_file) as f:
            for line in f:
                try:
                    entry = json.loads(line.strip())
                    done_ids.add(entry.get('question_id'))
                except json.JSONDecodeError:
                    pass

    remaining = [d for d in data if d.get('question_id') not in done_ids]
    print(f'Total: {len(data)} | Done: {len(done_ids)} | Remaining: {len(remaining)}')
    print(f'Workers: {args.workers}')
    print()

    if not remaining:
        print('All done!')
        return

    start_time = time.time()
    completed = 0
    total = len(remaining)

    # Build worker args — assign worker_id round-robin
    worker_args = [(entry, i % args.workers) for i, entry in enumerate(remaining)]

    with open(results_file, 'a') as fout:
        with ProcessPoolExecutor(max_workers=args.workers) as pool:
            futures = {pool.submit(worker_fn, wa): wa[0] for wa in worker_args}

            for future in as_completed(futures):
                entry = futures[future]
                try:
                    result = future.result()
                    if result is not None:
                        fout.write(json.dumps(result) + '\n')
                        fout.flush()
                except Exception as e:
                    print(f'  ERROR: {e}', file=sys.stderr)

                completed += 1
                elapsed = time.time() - start_time
                rate = completed / elapsed if elapsed > 0 else 0
                eta = (total - completed) / rate if rate > 0 else 0
                qid = entry.get('question_id', '?')
                qtype = entry.get('question_type', '?')
                print(f'  [{completed}/{total}] {qtype:<30} {qid[:20]}  '
                      f'elapsed={elapsed:.0f}s  eta={eta/60:.0f}min  rate={rate:.2f}q/s')

    elapsed = time.time() - start_time
    print(f'\nDone in {elapsed:.1f}s ({elapsed/60:.1f} min)')
    print(f'Results: {results_file}')


if __name__ == '__main__':
    main()
