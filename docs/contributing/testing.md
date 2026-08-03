---
title: Testing
---

# Testing

Uteke has a contract: every core subsystem must have tests that lock its invariants. This guide explains what that means and how to verify your changes.

## Running checks

All checks must pass before any PR is merged:

```bash
# Format check
cargo fmt --all -- --check

# Lint (warnings are errors)
cargo clippy --workspace --all-targets -- -D warnings

# All tests
cargo test --workspace

# Build (release mode)
cargo build --workspace --release

# API docs freshness
cargo run -p docgen
```

CI runs all of these on every push to `develop` and `main`.

## Core subsystems

These paths require tests because a local fix here can have global blast radius:

### Write path (remember)

The write path is the most critical: it's the only place where data enters the system.

| Invariant | What to test |
|-----------|-------------|
| SQLite-first | If usearch write fails, metadata is still in SQLite |
| Atomic save | usearch index file is never corrupt after crash mid-write |
| Duplicate handling | Re-embedding same content doesn't create duplicates |

```bash
cargo test --workspace -- remember
```

### Read path (recall)

Recall must return correct ranked results across all strategies.

| Invariant | What to test |
|-----------|-------------|
| RRF correctness | Vector-only and FTS5-only results are merged by rank position, not score |
| Cache coherence | Stale cache entries never returned (TTL 5min) |
| Strategy isolation | `graph` cache entries don't collide with `hybrid` |
| Entity filter | Results are correctly scoped to entity/category |

```bash
cargo test --workspace -- recall
```

### Delete path (forget)

| Invariant | What to test |
|-----------|-------------|
| Lock ordering | usearch lock acquired before SQLite delete |
| Consistency | After forget, ID is absent from both SQLite and usearch |

```bash
cargo test --workspace -- forget
```

### Schema migrations

| Invariant | What to test |
|-----------|-------------|
| Additive only | Migration from any historical version to latest succeeds |
| Zero data loss | All existing memories survive migration |
| Idempotent | Running the same migration twice doesn't fail or duplicate |

```bash
cargo test --workspace -- migration
```

### FTS5

| Invariant | What to test |
|-----------|-------------|
| Phrase match | Exact phrase queries return exact matches |
| Token-OR fallback | When phrase match returns nothing, token-OR is tried |

### API registry sync

| Invariant | What to test |
|-----------|-------------|
| Handler ↔ Registry | Every handler route exists in `ENDPOINTS` constant and vice versa |

```bash
cargo test --workspace -- registry
```

## Integration tests

Integration tests live in `crates/uteke-core/tests/`. These test full command flows (remember → recall → forget) against a real SQLite + usearch instance.

```bash
# Run integration tests only
cargo test --workspace --test "*"
```

## What makes a good test

1. **Tests the invariant, not the implementation** — if you refactor, the test should still pass
2. **Isolates the subsystem** — mock what you can, but test real SQLite/usearch for write/read/delete paths
3. **Has a clear assertion** — no "it doesn't panic" tests; assert the expected behavior
4. **Runs fast** — < 10ms per unit test, < 100ms per integration test
5. **Doesn't depend on order** — each test is independent; no shared mutable state

## Coverage expectations

Not every line needs a test. Prioritize:

- **Must**: write path, read path, delete path, schema migrations
- **Should**: API endpoint handlers, MCP tool implementations
- **Nice**: CLI argument parsing, logging, error formatting
