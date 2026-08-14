# Mutation Testing

Uteke uses [cargo-mutants](https://github.com/sourcefrog/cargo-mutants) for mutation testing — a technique that validates **test quality** (not code quality) by modifying source code and checking if existing tests catch the mutation.

## Quick Start

```bash
# Install cargo-mutants
cargo install cargo-mutants --locked

# Run on a specific file
cargo mutants --file crates/uteke-core/src/salience_recency.rs -j 4 --timeout 60

# Run on all non-excluded files
cd crates/uteke-core && cargo mutants -j 4 --timeout 60
```

## What It Does

cargo-mutants modifies your code automatically:

| Mutation | Example |
|---|---|
| Replace operator | `+` → `*`, `>` → `==`, `*` → `/` |
| Replace constant | `return 0.0` instead of computed value |
| Delete match arm | Remove `"event" =>` from match |
| Flip assignment | `+=` → `-=` |

After each mutation, it runs the test suite. If tests still pass → **missed mutant** (your tests didn't catch the bug).

## When to Run

- **Before releases**: Run on core logic files as a pre-release quality gate
- **After writing new tests**: Verify your tests actually catch bugs
- **CI**: Runs automatically on `develop → main` PRs (see `.github/workflows/mutation-testing.yml`)

## Writing Mutation-Killing Tests

The key insight: **use exact numeric assertions, not just `>` / `<` comparisons**.

### Bad (won't kill mutants)

```rust
// This won't catch replace + with *, because both produce > 0
assert!(salience_score(&m) > 0.0);
```

### Good (kills mutants)

```rust
// This WILL catch replace + with *, because exact value differs
let s = salience_score(&m);  // importance=0.4, access=0, pinned=false
assert!((s - 0.2).abs() < 0.01, "expected ~0.2, got {s}");
//   original: 0.4*0.5 + 0*0.3 + 0 = 0.2
//   mutant:   0.4*0.5 * 0*0.3 + 0 = 0.0  ← caught!
```

### Pattern

1. Pick values where each component of the formula contributes a **distinct, known** amount
2. Compute the expected result by hand
3. Assert with `abs() < epsilon` (floats need tolerance)
4. Add a comment explaining which mutant(s) the test kills

## Excluded Files

See `crates/uteke-core/mutants.toml` for the full exclusion list. Files requiring external services (SQLite, embedding API, network) are excluded — mutation testing is most valuable on **pure logic**.

## Current Coverage

| Module | Mutants | Caught | Missed | Equivalent | Score |
|---|---|---|---|---|---|
| `jaccard.rs` | 12 | 9 | 0 | 3 (unviable) | 100% |
| `salience_recency.rs` | 53 | 48 | 3 | 2 (eq) | 96% |
| `recall_cache.rs` | 25 | 18 | 5 | ~3 (eq) | 80% |
| `chunker.rs` | 163 | — | — | — | TBD (deferred) |

**Overall**: 11 new mutation-killing tests added. Mutation score improved from 76% → 96% for `salience_recency.rs`.

### Equivalent Mutants

Some mutants produce identical behavior regardless of tests. These are documented as "equivalent mutants" and excluded from scoring:

- `apply_boosts: replace > with >=` (2 mutants) — multiplying by weight=0.0 is a no-op
- `recall_cache::put retain: replace < with <=` — get-path TTL check also catches expired entries

Last updated: v0.14.1
