#!/bin/bash
# LongMemEval S benchmark progress monitor
# Reports: count, growth rate, estimated completion, alert if stalled

RESULTS="/opt/data/github/codecoradev/uteke/benchmarks/longmemeval/results_s/retrieval_results.jsonl"
LOG="/opt/data/github/codecoradev/uteke/benchmarks/longmemeval/results_s_run.log"
PREV_COUNT_FILE="/tmp/uteke-lmeval-prev-count"

CURRENT=$(wc -l < "$RESULTS" 2>/dev/null || echo "0")
TOTAL=500

# Previous count for growth calculation
PREV=$(cat "$PREV_COUNT_FILE" 2>/dev/null || echo "0")
echo "$CURRENT" > "$PREV_COUNT_FILE"

GROWTH=$((CURRENT - PREV))
PCT=$((CURRENT * 100 / TOTAL))

# Check if process running
if pgrep -f "run_eval.py.*results_s" > /dev/null 2>&1; then
    STATUS="🟢 RUNNING"
else
    STATUS="🔴 STOPPED"
fi

# Last progress line from log
LAST_PROGRESS=$(grep "Evaluating:" "$LOG" 2>/dev/null | tail -1 | sed 's/.*Evaluating:/Evaluating:/' || echo "N/A")

# Time stamp
NOW=$(date '+%H:%M:%S')

# Alert logic
ALERT=""
if [ "$STATUS" = "🔴 STOPPED" ] && [ "$CURRENT" -lt "$TOTAL" ]; then
    ALERT="⚠️ PROCESS DEAD — needs restart"
elif [ "$GROWTH" -eq 0 ] && [ "$STATUS" = "🟢 RUNNING" ]; then
    ALERT="⚠️ NO GROWTH — possible stall (but may be processing large question)"
fi

echo "📊 LongMemEval S Benchmark — ${NOW}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Progress: ${CURRENT}/${TOTAL} (${PCT}%)"
echo "Growth: +${GROWTH} since last check (30 min ago)"
echo "Status: ${STATUS}"
echo "Last: ${LAST_PROGRESS}"
if [ -n "$ALERT" ]; then
    echo "${ALERT}"
fi
