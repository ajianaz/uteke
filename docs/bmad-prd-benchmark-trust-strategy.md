# PRD — Benchmark-as-Trust Strategy: uteke Public Benchmarks + Cloud "Proof Mode"

**Version:** 1.0-draft (from Round 10 multi-agent discussion, 2026-08-29)
**Status:** AWAITING OWNER APPROVAL (per approval-gate rule, 2026-08-29)
**Source:** `disc:uteke-benchmark-strategy-r10` (CTO moderator; CFO, CLO, CMO, COO — 4/4 collected)

---

## 1. Problem Statement

Uteke v0.16.0 achieves 98.2% recall_any@5 on LongMemEval-S (500Q, full run) — the
highest publicly disclosed number we could verify — yet the project has 44 stars and
no public benchmark presence. Competitors (agentmemory 27.7K stars) publish lower
numbers with larger audiences. Meanwhile, the owner is considering whether uteke-cloud
should offer selfhosted users a way to visualize benchmark results of their own
installs.

## 2. Goals & Non-Goals

### Goals
1. **G1 — Publish a credible, legally-safe benchmark page** (docs + chart) that
   positions uteke's retrieval quality without triggering a benchmark war.
2. **G2 — "Proof Mode" in uteke-cloud**: free-tier hook letting selfhosted users run
   evals locally and visualize their results in the cloud dashboard.
3. **G3 — Recurring credibility maintenance**: monthly competitor-number monitoring
   with primary-source citations and dates.

### Non-Goals
- End-to-end QA leaderboard entry (different league — retrieval ≠ QA accuracy).
- Cloud-side eval execution (eval-as-a-service) until measured demand.
- External audit certification before MRR ≥ $10K.
- Chasing recall_all@5 beyond current 88.0% via diversity optimization (parked by owner).

## 3. Users & Personas

| Persona | Need | Served by |
|---|---|---|
| Selfhoster (skeptical dev) | "Don't trust vendor benchmarks — verify yourself" | Proof Mode (G2) |
| OSS evaluator (HN/reddit reader) | One-glance credible comparison | Chart + dual-metric page (G1) |
| Cloud user (Personal/Pro) | Scheduled evals, history, share links | Pro tier features |

## 4. Headline Numbers (verified 2026-08-29, run `results_modal_default`)

| Metric | Value |
|---|---|
| recall_any@5 | **98.2%** |
| recall_any@10 | 98.8% |
| recall_all@10 (strict) | 95.4% |
| recall_all@5 (strict binary) | 88.0% (ceiling 99.4%) |
| coverage@5 (partial credit) | 94.3% |

Competitor reference (recall_any@5, their published docs): agentmemory 95.2, MemPalace 96.6.
Ablation: uteke FTS5-only 91.4; agentmemory BM25-only 86.2.

## 5. Positioning & Claims (CMO + CLO joint constraint)

- **Hero claim = efficiency**, not rank: "Top-tier LongMemEval-S accuracy, 23 ms warm,
  CPU-only, no LLM in the retrieval path."
- Ranked claim MUST be qualified: *"Highest among publicly disclosed recall_any@5
  results on LongMemEval-S as of 29 Aug 2026; internal benchmark, not third-party
  certified."*
- Dual-metric honesty lives in the methodology section ("we publish what others don't"),
  not the headline.
- Never frame as "beats X" — frame as "same benchmark, open harness, verify yourself."
- Chart: axis from 0, one metric per chart, uteke in accent color only, plain-text
  competitor names (no logos), trademark disclaimer footer.

## 6. Feature: Proof Mode (uteke-cloud)

### Scope (MVP, per COO estimate: 2–3 days)
1. POST endpoint: upload eval results JSON (schema-validated, ≤1 MB) — 4–6 h
2. Per-user result storage (SQLite/S3, quota: free = 1 result, Pro = unlimited) — 2 h
3. Client-side chart rendering (Chart.js; zero server rendering) — 4–8 h
4. Auth + rate limit — 2–4 h

### Explicitly deferred
- Cloud-side eval execution (queue, sandbox, dataset hosting/licensing) — 2–3 weeks,
  revisit at 500+ cloud users or 10+ feature requests.
- Public shareable dashboards (Pro), team leaderboards (Team) — post-beta.

### Compliance prerequisites (CLO, BLOCKING before live)
- Privacy Policy with explicit retention (delete at 30 days or with workspace) and
  no-training clause.
- uteke-cloud positioned as data **processor**; user remains controller.
- Encryption at rest + internal access audit log.

## 7. Pricing Interaction (CFO)

No new tiers. Existing ladder (2026-08-21 decision) holds: **Starter $5 / Pro $10 / Team $25**.

| Capability | Free/Starter | Pro $10 | Team $25 |
|---|---|---|---|
| Proof Mode viz + 1 result | ✅ | ✅ | ✅ |
| Scheduled local eval + history | — | ✅ | ✅ |
| Share dashboard / team leaderboard | — | — | ✅ |

Narrative sync task: converge "Personal $5" references to the 21-Aug ladder **before**
the benchmark page ships (avoid two competing price stories).

## 8. Rollout Sequence (owner-approval gate before EACH step)

1. ✅ PR #1141 — docs page + chart (CI running)
2. README section + badge (after docs merge)
3. Blog post (codecora.dev)
4. X/Twitter thread (English)
5. Show HN — only after Proof Mode is live (HN punishes unverifiable claims)

## 8.1 Substantiation file (CLO requirement, before any public claim)

Archive into `docs/assets/bench-substantiation/` (private until needed):
- Timestamped snapshots of competitor benchmark pages (agentmemory, MemPalace)
- Our run logs + metrics computation script + commit SHA of the run
- Access dates + versions for every cited number

## 9. Maintenance (COO owns)

- Monthly cron: monitor competitor releases/claims (0.5 day setup, 1–2 h/month).
- Every table entry carries: primary source link, access date, uteke version.
- Owner approval remains the final gate before any number goes public.

## 10. Risks

| Risk | Mitigation |
|---|---|
| agentmemory reruns & publishes 97+ | Efficiency hero claim unaffected; update table with their new number, cite date |
| "Internal benchmark" skepticism | Open harness + Proof Mode = "run it yourself" |
| Disk exhaustion on cloud box (hit 91% once) | No cloud eval execution in MVP; disk guard if ever added |
| Legal challenge on comparison | Substantiation file + qualified claims + per-row source footnotes |
| Two pricing stories circulating | Converge narrative before page ships |

## 11. Success Metrics

- Docs page live with zero retraction-level complaints (30 days)
- ≥10 Proof Mode uploads in first month post-launch
- Show HN post ≥50 points with verifiable-claim sentiment
- Zero legal/compliance incidents
