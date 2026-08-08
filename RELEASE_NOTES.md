# v0.13.0: Safe memory lifecycle

Memories now go through a reviewable soft-delete state before anything gets permanently removed. No automated process can hard-delete your data anymore. That includes aging cleanup, deduplication, and bulk operations.

If you're running uteke in production with an agent that writes memories autonomously, this is the release you want.

## Why this matters

Before v0.13.0, delete operations were irreversible. A consolidation cycle or aging cleanup could remove memories your agent still depended on, and there was no way back. If you've ever watched a maintenance routine quietly prune something important, you know the feeling.

Now every deletion flows through a three-state lifecycle:

```
ACTIVE → DEPRECATED (hidden, restorable, 30-day TTL) → PRUNED
```

Deprecated memories are hidden from search and recall but kept on disk. You can restore them any time within the TTL window. Only two code paths can hard-delete: `prune()` when the TTL expires, and `forget()` when you explicitly set `soft_delete_only = false`. Everything else redirects to soft-delete by default.

## What's new

**Lifecycle configuration (`LifecycleConfig`)**

11 configurable fields controlling deprecation thresholds, TTL duration, auto-cycle scheduling, and a per-cycle percentage cap. The cap defaults to 1%, meaning each cycle touches at most 1% of your active memories (clamped between 1 and 50). On a store with 1,000 memories, that's no more than 10 deprecated per cycle. Conservative defaults, adjustable if you need them.

**CLI commands**

```bash
uteke lifecycle status           # see what's deprecated, what's pending prune
uteke lifecycle cycle            # run a manual lifecycle cycle
uteke lifecycle promote <id>     # restore a deprecated memory to active
uteke lifecycle restore <id>     # alias for promote
```

**HTTP API endpoints**

- `GET /lifecycle/status` — current state of deprecated and prunable memories
- `POST /lifecycle/cycle` — trigger a cycle programmatically
- `POST /lifecycle/promote` — restore a specific memory

**Documentation**

New `/memory-lifecycle` docs page with configuration reference, best practices, and migration notes.

## Behavior changes

The background maintenance thread (previously auto-aging) now runs the full two-phase lifecycle cycle: deprecate first, then prune only what's expired. This means scheduled maintenance no longer bypasses the safety net.

All delete paths respect `soft_delete_only`, which is `true` by default:

| Operation | Behavior with `soft_delete_only = true` |
|---|---|
| `aging_cleanup()` | Redirects to `deprecate()` |
| `consolidate()` | Redirects to `deprecate()` |
| `delete()` | Redirects to `deprecate()` |
| `bulk_delete()` | Redirects to `deprecate()` |
| `forget()` | Redirects to `deprecate()` |
| `bulk_forget_*()` | Redirects to `deprecate()` |

If you have existing automation that relies on hard deletes, set `soft_delete_only = false` to restore the old behavior. We recommend leaving it on.

## Bug fixes

**Import now handles JSONL correctly.** The export function writes JSONL (one JSON object per line), but import was treating the entire file as a single JSON object. Export-to-import roundtrips were broken. The importer now detects format automatically and handles all three: JSON arrays, single objects, and JSONL.

**Schema migration is transparent.** The new `deprecate_reason` column is added on first run using a `column_exists()` check. No schema version bump, no manual migration step. Existing databases upgrade in place.

## Tested in production

Full regression suite ran against a clone of a production database (596 memories across 3 namespaces):

- 412 unit tests pass
- 18 CLI integration tests pass
- 13 HTTP API endpoint tests pass
- 5 MCP bridge tool tests pass
