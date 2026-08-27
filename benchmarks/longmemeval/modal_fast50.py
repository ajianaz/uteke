#!/usr/bin/env python3
"""
Modal fan-out runner for the LongMemEval benchmark (uteke).

Runs the EXISTING harness (run_eval.py) unchanged, in parallel, on Modal CPU
containers. Same uteke binary (v0.15.0 GitHub release), same embedding model
(EmbeddingGemma 300M Q4 ONNX baked into the image), same dataset file →
results are directly comparable with local throttled runs.

Why CPU and not GPU: the embedding model is a 300M Q4 ONNX model designed for
CPU inference. The historical bottleneck was (a) 2-core throttling +
sequential execution, not (b) compute. Fan-out over N containers divides
wall-clock by ~N at CPU pricing (<$1 for a full 500Q run).

Usage:
    # Sanity check: 10 questions, 2 shards
    modal run modal_fanout.py --limit 10 --num-shards 2

    # Full 500Q hybrid run (the publishable number)
    modal run modal_fanout.py --strategy hybrid --num-shards 10

    # Full 500Q vector run
    modal run modal_fanout.py --strategy vector --num-shards 10

Results land in results_modal_<strategy>[_<limit>q]/retrieval_results.jsonl
(merged from all shards), ready for: python print_metrics.py <file>

Notes:
- Each shard runs in its own container with its own temp store → no
  cross-contamination between shards (harness wipes per question anyway).
- cpu=2 matches the local throttled run (taskset -c 0-1) for fair per-question
  latency comparison.
- The embedding model is baked at /root/.codecora/uteke/models/ so no
  first-run download happens inside containers.
"""

import json
import pathlib
import subprocess
import os
import sys
import time

import modal

REPO_DIR = pathlib.Path(__file__).parent
UTEKE_VERSION = "v0.15.0"
DATA_FILE = "longmemeval_fast50.json"
RERANK = False  # removed (#1118 cancelled — single-model direction); kept as False for compat
RERANK_DEPTH = 20
# Local source of the embedding model (must contain onnx/ + tokenizer.json).
MODEL_SOURCE = pathlib.Path("/opt/data/.codecora/uteke/models/embeddinggemma-q4")

app = modal.App("uteke-lmeval-fast")

# Volume for durable shard outputs: if the local map() call dies (network,
# spend limit mid-run, Ctrl-C), completed shards are preserved and a rerun
# resumes instead of starting from zero.
vol = modal.Volume.from_name("uteke-lmeval-fast", create_if_missing=True)

image = (
    # Ubuntu 24.04 base: glibc 2.39 — the uteke release binary needs it
    # (debian slim/bookworm ships glibc 2.36 → too old, incl. the "legacy" build).
    modal.Image.from_registry("ubuntu:24.04", add_python="3.11")
    .apt_install("curl", "ca-certificates", "util-linux")
    # uteke release bundle: binary + libonnxruntime colocated (exe-dir lookup).
    .run_commands(
        f"curl -sL https://github.com/codecoradev/uteke/releases/download/{UTEKE_VERSION}/"
        f"uteke-x86_64-unknown-linux-gnu-legacy-{UTEKE_VERSION}.tar.gz | tar xz -C /tmp && "
        "cp /tmp/uteke /usr/local/bin/uteke && cp /tmp/libonnxruntime* /usr/local/bin/ && "
        "chmod +x /usr/local/bin/uteke && uteke --version"
    )
    .pip_install("tqdm>=4.65.0", "numpy>=1.24.0", "onnxruntime>=1.17.0", "tokenizers>=0.19.0")
    # Bake the embedding model so containers never download at runtime.
    # (add_local_* must come last — Modal mounts them at container start.)
    .add_local_dir(str(MODEL_SOURCE), "/root/.codecora/uteke/models/embeddinggemma-q4")
    .add_local_file(str(REPO_DIR / "run_eval.py"), "/root/harness/run_eval.py")
    .add_local_file(str(REPO_DIR / "data" / DATA_FILE), "/root/harness/data.json")
    .add_local_file(str(REPO_DIR / "temporal.py"), "/root/harness/temporal.py")
    .add_local_file(str(REPO_DIR / "compare_fast50.py"), "/root/harness/compare_fast50.py")
)


@app.function(image=image, timeout=14400, cpu=2, volumes={"/root/vol": vol})
def run_shard(spec: dict) -> dict:
    """Evaluate one strided slice of the dataset with the unmodified harness.

    spec: {"strategy": str, "shard_idx": int, "num_shards": int, "limit": int}
    Writes shard output durably to the Modal Volume as soon as it finishes,
    so progress survives local map() interruptions and reruns can resume.
    """
    import json as _json

    strategy = spec["strategy"]
    shard_idx = spec["shard_idx"]
    num_shards = spec["num_shards"]
    limit = spec["limit"]
    temporal = bool(spec.get("temporal", False))

    # Variant-keyed volume path: baseline and temporal shards must never
    # share cache entries, or resume would silently return stale results.
    variant = f"{strategy}_temporal" if temporal else strategy
    vol_path = pathlib.Path("/root/vol") / variant / f"shard_{shard_idx:02d}.jsonl"
    data = _json.loads(pathlib.Path("/root/harness/data.json").read_text())
    if limit and limit > 0:
        data = data[:limit]
    shard = data[shard_idx::num_shards]  # strided → even question-type mix
    expected = len(shard)
    if not shard:
        return {"shard_idx": shard_idx, "n": 0, "elapsed": 0.0, "jsonl": "", "stderr_tail": ""}

    # Volume cache: skip kalau shard SUDAH LENGKAP; resume kalau PARTIAL (dari timeout)
    resume_args = []
    if vol_path.exists():
        prior = [l for l in vol_path.read_text().splitlines() if l.strip()]
        if len(prior) >= expected:
            return {
                "shard_idx": shard_idx,
                "n": len(prior),
                "evaluated": len(prior),
                "elapsed": 0.0,
                "jsonl": "\n".join(prior),
                "stderr_tail": "(resumed from volume)",
            }
        if prior:
            # partial: seed output dir dari Volume agar run_eval --resume melanjutkan
            out_seed = pathlib.Path(f"/tmp/out_{shard_idx}")
            out_seed.mkdir(parents=True, exist_ok=True)
            (out_seed / "retrieval_results.jsonl").write_text("\n".join(prior) + "\n")
            resume_args = ["--resume"]

    pathlib.Path("/tmp/shard.json").write_text(_json.dumps(shard))
    out_dir = f"/tmp/out_{shard_idx}"

    t0 = time.time()
    try:
        proc = subprocess.run(
            [
                sys.executable, "/root/harness/run_eval.py",
                "--data", "/tmp/shard.json",
                "--output", out_dir,
                "--strategy", strategy,
                "--namespace", "lmeval",
                *(["--temporal"] if temporal else []),
                *resume_args,
            ],
            capture_output=True, text=True, timeout=14100,  # 14400 - buffer commit
        )
    except subprocess.TimeoutExpired as e:
        # partial-save: run_eval menulis streaming ke out_dir; simpan apa adanya
        partial = pathlib.Path(out_dir) / "retrieval_results.jsonl"
        jsonl = partial.read_text() if partial.exists() else ""
        vol_path.parent.mkdir(parents=True, exist_ok=True)
        vol_path.write_text(jsonl)
        vol.commit()
        return {
            "shard_idx": shard_idx,
            "n": len([l for l in jsonl.splitlines() if l.strip()]),
            "evaluated": len([l for l in jsonl.splitlines() if l.strip()]),
            "elapsed": round(time.time() - t0, 1),
            "jsonl": jsonl,
            "stderr_tail": f"TIMEOUT partial-save: {len(jsonl.splitlines())}/{len(shard)} Q evaluated",
        }
    elapsed = time.time() - t0
    if proc.returncode != 0:
        raise RuntimeError(f"shard {shard_idx} failed:\n{proc.stderr[-2000:]}")

    jsonl_path = pathlib.Path(out_dir) / "retrieval_results.jsonl"
    jsonl = jsonl_path.read_text() if jsonl_path.exists() else ""

    # Persist this shard durably before returning — see vol comment above.
    vol_path.parent.mkdir(parents=True, exist_ok=True)
    vol_path.write_text(jsonl)
    vol.commit()

    return {
        "shard_idx": shard_idx,
        "n": len(shard),
        "evaluated": jsonl.count("\n"),
        "elapsed": round(elapsed, 1),
        "jsonl": jsonl,
        "stderr_tail": proc.stderr[-500:],
    }


@app.function(image=image, timeout=600, cpu=1, volumes={"/root/vol": vol})
def list_volume(strategy: str) -> list:
    """List completed shards for a strategy on the volume (progress check)."""
    base = pathlib.Path("/root/vol") / strategy
    out = []
    if base.exists():
        for f in sorted(base.glob("shard_*.jsonl")):
            n = sum(1 for l in f.read_text().splitlines() if l.strip())
            out.append({"shard": f.name, "questions": n})
    return out


@app.local_entrypoint()
def main(
    strategy: str = "hybrid",
    num_shards: int = 2,
    limit: int = 0,
    outdir: str = "",
    temporal: bool = False,
):
    if not MODEL_SOURCE.exists():
        sys.exit(f"Model source not found: {MODEL_SOURCE}")
    if not (REPO_DIR / "data" / DATA_FILE).exists():
        sys.exit(f"Dataset not found: {REPO_DIR / 'data' / DATA_FILE}")

    outdir = outdir or f"results_modal_{strategy}" + ("_temporal" if temporal else "") + (f"_{limit}q" if limit else "")
    out_path = pathlib.Path(outdir) / "retrieval_results.jsonl"
    out_path.parent.mkdir(parents=True, exist_ok=True)

    # Progress check from the volume (if any shards completed before a prior
    # interruption, they'll be resumed instead of recomputed).
    try:
        variant = f"{strategy}_temporal" if temporal else strategy
        done = list_volume.remote(variant)
        if done:
            print(f"Volume state: {len(done)} shard(s) already on volume:")
            for d in done:
                print(f"  {d['shard']}: {d['questions']} questions")
    except Exception as e:
        print(f"(volume check skipped: {e})")

    print(f"Fan-out: strategy={strategy} shards={num_shards} limit={limit or 'ALL'}")
    t0 = time.time()

    # Strided slicing means shard i covers data[i::num_shards].
    inputs = [
        {"strategy": strategy, "shard_idx": i, "num_shards": num_shards, "limit": limit,
         "temporal": temporal}
        for i in range(num_shards)
    ]
    merged, seen = [], set()
    for res in run_shard.map(inputs):
        n_eval = 0
        for line in res["jsonl"].splitlines():
            if not line.strip():
                continue
            qid = json.loads(line).get("question_id")
            if qid in seen:
                continue  # dedupe guard (retries)
            seen.add(qid)
            merged.append(line)
            n_eval += 1
        print(
            f"  shard {res['shard_idx']:2d}: {n_eval}/{res['n']} questions "
            f"in {res['elapsed']}s"
        )

    out_path.write_text("\n".join(merged) + ("\n" if merged else ""))
    wall = time.time() - t0
    print(f"\nDone: {len(merged)} questions in {wall:.0f}s wall clock")
    print(f"Results: {out_path}")
    print(f"\nRun: python print_metrics.py {out_path}")
    # Print metrics right away (numpy available locally).
    subprocess.run([sys.executable, str(REPO_DIR / "print_metrics.py"), str(out_path)])
