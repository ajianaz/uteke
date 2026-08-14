---
title: Organizing Memories
---

# Organizing Memories

Uteke stores all memories in a **flat structure** — there are no rigid L0–L3 layers or forced hierarchies. Instead, an **implicit hierarchy** emerges naturally from three existing fields:

| Field | Purpose | Range |
|---|---|---|
| `memory_type` | What kind of knowledge this is | `fact`, `procedure`, `preference`, `decision`, `context`, `event` |
| `importance` | How much weight this carries in recall scoring | `0.0` – `1.0` |
| `pinned` | Protect from aging/pruning | `true` / `false` |

Combined with built-in **recency decay** (type-based half-life) and **salience scoring** (access frequency), these fields create a practical hierarchy without the complexity of explicit layers.

## Implicit Layer Mapping

| Layer | `memory_type` | `importance` | `pinned` | Example |
|---|---|---|---|---|
| **Long-term core** | `decision`, `preference` | 0.9+ | ✅ | "User prefers dark mode", "Decided on OAuth 2.1" |
| **Scenario context** | `procedure`, `context` | 0.5–0.8 | ❌ | "Auth middleware runs before route handler" |
| **Atomic facts** | `fact` | 0.3–0.7 | ❌ | "Migration 0042 adds indexes on users.email" |
| **Ephemeral** | `fact`, `event` | 0.1–0.3 | ❌ | "Today's meeting was at 2pm" (ages out via recency decay) |

## How Decay Works

Each `memory_type` has a configurable **half-life** (`type_half_life_days` in config):

- **Decisions** and **preferences** are evergreen — slow decay (default: 365 days)
- **Procedures** and **context** decay moderately (default: 90 days)
- **Facts** and **events** decay faster (default: 30 days)

Memories that are frequently recalled drift **upward** in salience. Cold memories drift **downward** and eventually get deprecated → pruned.

**Pinned** memories are excluded from aging entirely.

## Recommended Usage Patterns

### Store a long-term decision

```bash
uteke remember "We chose PostgreSQL for ACID compliance and JSONB support" \
  --type decision --importance 0.95 --pinned
```

### Store a preference

```bash
uteke remember "User prefers concise responses without filler" \
  --type preference --importance 0.9 --pinned
```

### Store a procedure

```bash
uteke remember "To deploy: push to main, CI builds Docker image, auto-deploy to prod" \
  --type procedure --importance 0.6
```

### Store an ephemeral fact

```bash
uteke remember "Standup moved to 9:30am today" \
  --type event --importance 0.2
```

This will naturally age out — no manual cleanup needed.

### Store an atomic fact

```bash
uteke remember "The config file is at ~/.codecora/uteke/uteke.toml" \
  --type fact --importance 0.5
```

## When to Pin

Pin only what you **never want to lose**:

- Core architectural decisions
- User preferences that define behavior
- Critical procedure steps
- Anything that would be expensive to re-derive

Do **not** pin:
- Ephemeral facts (today's meeting time)
- Contextual information (current sprint goals)
- Anything that will become stale

## Configuring Decay

Customize half-life per type in `uteke.toml`:

```toml
[aging]
enabled = true

[aging.type_half_life_days]
decision = 365
preference = 365
procedure = 90
context = 90
fact = 30
event = 14
```

Shorter half-life = faster decay. Set very high values (e.g. `9999`) for types you want to keep indefinitely without pinning.

## Summary

| Mechanism | Handles |
|---|---|
| `memory_type` | What kind of knowledge (affects decay rate) |
| `importance` | How much weight in recall scoring |
| `pinned` | Protect from aging entirely |
| Recency decay | Temporal relevance (old → forgotten) |
| Salience scoring | Access frequency (popular → boosted) |

No explicit layers needed. The combination of these fields gives you a **self-organizing hierarchy** that adapts to usage patterns automatically.
