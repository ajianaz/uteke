---
title: Smart Decay
---

# Smart Decay

Automatically manage memory freshness with importance scoring and aging.

## Pinning

Pin critical memories so they never decay:

```bash
uteke pin <id>
uteke unpin <id>
```

## Importance Scoring

uteke uses composite importance scoring based on access patterns, metadata, and pinning:

```bash
# Recalculate importance scores
uteke importance
```

## Memory Aging

Track memory freshness with hot/warm/cold tiers:

```bash
# Show hot/warm/cold breakdown
uteke aging status

# Preview memories older than 90 days
uteke aging preview --older-than-days 90

# Delete stale memories older than 180 days
uteke aging cleanup --older-than-days 180 --yes
```

## How It Works

| Tier | Description |
|------|-------------|
| **Hot** | Accessed within `hot_days` (default 7), gets a `hot_boost` score bonus |
| **Warm** | Accessed within `warm_days` (default 30) |
| **Cold** | Older than the warm window, or never accessed after creation |

Never-accessed memories are classified as **Cold**. Tier thresholds and boost are configurable via the `[tier]` config section.

Pinned memories bypass aging entirely — they stay in the store regardless of access patterns.

## See Also

- [Installation](/install) — install uteke
- [CLI Reference](/cli-reference) — full command options
