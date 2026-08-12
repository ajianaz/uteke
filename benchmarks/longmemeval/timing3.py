#!/usr/bin/env python3
"""Time 3 questions to get accurate per-question timing."""
import json, subprocess, tempfile, os, shutil, time

data = json.load(open('/opt/data/github/codecoradev/uteke/benchmarks/longmemeval/data/longmemeval_s_cleaned.json'))

def session_to_text(session):
    lines = []
    for turn in session:
        lines.append(f"{turn.get('role','unknown')}: {turn.get('content','')}")
    return "\n".join(lines)

# Pick 3 questions from different categories (but skip single-session-user which is small)
test_qs = []
for qt in ['temporal-reasoning', 'knowledge-update', 'multi-session']:
    for d in data:
        if d['question_type'] == qt:
            test_qs.append(d)
            break

namespace = 'lmeval'
total_start = time.time()

for i, entry in enumerate(test_qs):
    q_start = time.time()
    qid = entry['question_id']
    qt = entry['question_type']
    ev_ids = set(entry.get('answer_session_ids', []))
    session_ids = entry.get('haystack_session_ids', [])
    sessions = entry.get('haystack_sessions', [])
    dates = entry.get('haystack_dates', [])

    total_words = sum(sum(len(t.get('content','').split()) for t in s) for s in sessions)
    print(f"\n[{i+1}/3] {qt} | {len(session_ids)} sessions | {total_words:,} words")

    store = tempfile.mkdtemp(prefix='uteke-t3-')
    jsonl_path = f'/tmp/t3_{qid}.jsonl'

    with open(jsonl_path, 'w') as f:
        for j, (sid, session) in enumerate(zip(session_ids, sessions)):
            text = session_to_text(session)
            meta = {"session_id": sid}
            if j < len(dates) and dates[j]:
                meta["date"] = str(dates[j])
            record = {"content": text, "tags": ["longmemeval"], "type": "context", "metadata": meta}
            f.write(json.dumps(record) + "\n")

    t0 = time.time()
    result = subprocess.run(
        ['uteke', '--store', store, '--namespace', namespace, '--json', 'import', jsonl_path, '--format', 'jsonl'],
        capture_output=True, text=True, timeout=600
    )
    t_import = time.time() - t0
    import_data = json.loads(result.stdout)
    print(f"  Import: {t_import:.1f}s ({import_data.get('imported',0)}/{len(session_ids)})")

    t0 = time.time()
    result = subprocess.run(
        ['uteke', '--store', store, '--namespace', namespace, '--json', 'recall',
         entry['question'], '--limit', '50', '--tags', 'longmemeval', '--min', '0.0'],
        capture_output=True, text=True, timeout=600
    )
    t_recall = time.time() - t0
    recall_data = json.loads(result.stdout)

    retrieved = []
    for r in recall_data:
        sid = r.get("metadata", {}).get("session_id")
        if sid:
            retrieved.append(sid)

    evidence = ev_ids.intersection(set(session_ids))
    for k in [5, 10]:
        hits = len(set(retrieved[:k]).intersection(evidence))
        total = len(evidence) if evidence else 1
        r_val = hits / total if total > 0 else 0.0
        print(f"  R@{k}: {r_val:.2f}  (hits={hits}/{len(evidence)})")

    t0 = time.time()
    subprocess.run(['uteke', '--store', store, '--namespace', namespace, '--json', 'forget', '--all', '--confirm'],
        capture_output=True, text=True, timeout=60)
    t_forget = time.time() - t0

    q_total = time.time() - q_start
    print(f"  Recall: {t_recall:.1f}s | Forget: {t_forget:.1f}s | Q total: {q_total:.1f}s")

    shutil.rmtree(store, ignore_errors=True)
    os.unlink(jsonl_path)

elapsed = time.time() - total_start
print(f"\n3 questions total: {elapsed:.0f}s = {elapsed/3:.0f}s avg per question")
print(f"Estimated 500 questions: {elapsed/3*500/3600:.1f} hours")
print("DONE")
