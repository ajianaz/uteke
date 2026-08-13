---
title: Memory Lifecycle
---

# Memory Lifecycle

Uteke uses a soft-delete lifecycle model for memory management. No automated process hard-deletes a memory. Memories transition through a reviewable soft-delete state before eventual pruning.

## How It Works

```
remember() → ACTIVE → soft_deprecate() → DEPRECATED (hidden, restorable) → prune() → HARD DELETE
```

| State | Visible in recall? | Restorable? | TTL |
|-------|-------------------|-------------|-----|
| **ACTIVE** | ✅ Yes | N/A | — |
| **DEPRECATED** | ❌ Hidden from recall/search | ✅ `promote()` | 30 days (configurable) |
| **PRUNED** | ❌ Gone | ❌ No | — |

### What triggers deprecation?

1. **Auto-lifecycle cycle** (server background thread, every 7 days by default)
2. **Manual `uteke lifecycle cycle`** command
3. **Explicit `delete()` / `forget()`**: redirected to soft-delete when `soft_delete_only = true` (default)
4. **Dream consolidation**: duplicate memories are deprecated, not deleted

### What triggers hard delete (prune)?

Only **two controlled paths** can hard-delete:

1. **`prune()`**: Expired deprecated memories (past `deprecated_ttl_days`). Runs automatically during lifecycle cycle when `auto_prune_enabled = true`.
2. **`forget()`**: Only when `soft_delete_only = false` (not the default).

## Configuration

See [Configuration → Memory Lifecycle](/configuration#memory-lifecycle) for the full config reference.

Default settings: 1% max deprecation per cycle, 90-day minimum age, 30-day TTL:

```toml
[lifecycle]
soft_delete_only = true
auto_aging_enabled = true
auto_aging_interval_hours = 168
min_age_days = 90
max_access_count = 3
max_deprecate_percent = 1.0
deprecated_ttl_days = 30
auto_prune_enabled = true
dream_dedup_soft_delete = true
```

## CLI Usage

```bash
# Check lifecycle status
uteke lifecycle status

# Run a lifecycle cycle manually
uteke lifecycle cycle

# Restore a deprecated memory
uteke lifecycle promote <memory-id>
```

See [CLI Reference → lifecycle](/cli-reference#uteke-lifecycle) for details.

## HTTP API

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/lifecycle/status` | Active vs deprecated counts |
| `GET` | `/lifecycle/deprecated` | List deprecated memories with TTL metadata (#1007) |
| `POST` | `/lifecycle/cycle` | Run lifecycle cycle |
| `POST` | `/lifecycle/promote` | Promote deprecated → active |

See [HTTP API Reference → Lifecycle](/api-reference#lifecycle) for request/response schemas.

## Best Practices

### Production deployments

- **Keep `soft_delete_only = true`** (default). This is your safety net: no data loss from bugs, misconfigured agents, or accidental deletes.
- **Tune `max_deprecate_percent`** based on your memory volume. For large stores (10K+ memories), 1% per cycle is 100 memories/week. Adjust if that feels too aggressive or too conservative.
- **Monitor deprecated count** via `/lifecycle/status` or `uteke lifecycle status`. A growing deprecated count without corresponding pruning may indicate a TTL misconfiguration.

### Development / testing

- Set `deprecated_ttl_days = 1` for faster iteration during development.
- Set `soft_delete_only = false` only in disposable test databases where you want true hard-delete semantics.

### Disabling auto-lifecycle entirely

```toml
[lifecycle]
auto_aging_enabled = false
```

Memories will still be soft-deleted on explicit `delete()`/`forget()`, but no automatic aging or pruning will occur. You can still run `uteke lifecycle cycle` manually.

## Migration from pre-v0.13.0

Existing databases are automatically migrated:

- A `deprecate_reason` column is added to the `memories` table (nullable, uses `column_exists()` migration pattern, no schema version bump).
- All existing memories remain ACTIVE, no behavior change on upgrade.
- The auto-lifecycle background thread starts with default settings on first server boot.

No manual migration steps required.
