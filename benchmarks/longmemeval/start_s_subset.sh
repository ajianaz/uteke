#!/usr/bin/env bash
# Auto-start S subset run after Oracle completes.
# Usage: bash start_s_subset.sh
set -euo pipefail

cd /opt/data/github/codecoradev/uteke/benchmarks/longmemeval
export PATH="/opt/data/.local/bin:$PATH"

# Wait for Oracle run to finish (check if process still running)
while pgrep -f "run_eval.py.*longmemeval_oracle" > /dev/null 2>&1; do
    sleep 30
done

echo "$(date '+%Y-%m-%d %H:%M:%S') — Oracle run finished, starting S subset..."

# Clean up any previous S results
rm -rf results_s

# Start S subset run
python3 run_eval.py \
    --data data/longmemeval_s_cleaned.json \
    --output results_s/ \
    --resume \
    2>&1 | tee results_s_run.log

echo "$(date '+%Y-%m-%d %H:%M:%S') — S subset run complete."
echo "Run: python3 print_metrics.py results_s/retrieval_results.jsonl"
