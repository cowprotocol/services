# Code Review Guidelines

These instructions guide automated code reviews. Focus on real bugs and
correctness issues. Do not pad reviews with praise or filler.

## Priorities

Review in priority order.

1. **Correctness** — logic errors, wrong return values, off-by-one, missing
   error propagation, incorrect arithmetic (especially with U256/BigDecimal)
2. **Safety** — panics in production paths (`unwrap` on fallible operations),
   unchecked arithmetic overflow, missing bounds checks
3. **Security** — SQL injection, command injection, secrets in logs,
   unvalidated external input at system boundaries
4. **Concurrency** — data races, deadlocks, incorrect `Send`/`Sync` bounds,
   holding locks across await points
5. **Backwards compatibility** — API breaking changes, database migration
   issues, settlement contract interaction changes

## Always check

- Token amount conversions between decimal and wei are correct (scaling
  direction, precision loss)
- Error types from external APIs (solvers, DEX aggregators) are mapped to the
  correct internal variant — wrong mapping produces noisy logs or silently
  drops valid quotes
- New `async` code does not block the Tokio runtime (no blocking I/O, no
  `std::thread::sleep`, no heavy computation without `spawn_blocking`)
- Database queries that touch large tables have been checked against existing
  indexes. If a PR adds or modifies a query on a large table, request
  `EXPLAIN ANALYZE` output (before and after) if not already included in the
  PR description
- Settlement-related changes are backward-compatible with in-flight auctions
- Changes to auction or solver logic preserve existing solver competition
  fairness

## Do NOT flag

- Style or formatting issues — `cargo fmt` and `clippy` handle these
- Missing documentation or comments on clear code
- Suggestions to add more tests unless a specific untested edge case is
  identified
- Pre-existing issues not introduced by the PR
- Theoretical concerns that require specific unlikely conditions
- `unwrap()` on values that are guaranteed by construction or prior checks
- Differences in naming conventions that are consistent within the changed
  file

## Severity

Use the code-review plugin's standard categories:

- **Normal** — a bug or issue that should be fixed before merging
- **Nit** — a minor improvement, not blocking

Do not flag pre-existing issues not introduced by the PR. If unsure whether
something is a real issue, ask a clarifying question rather than asserting a
bug.

## Skip these files

- `Cargo.lock`
- Generated contract bindings under `crates/contracts/`
- Database migration files (review schema changes only, not the generated SQL)
- Test fixture JSON files

## Format

For each finding, include:
- File path and line number
- What is wrong (one sentence)
- Why it matters (one sentence)
- For small, self-contained fixes, include a committable GitHub suggestion
  block. For larger fixes, describe the recommended approach in prose.

Do not summarize the PR. Do not list what looks correct. Only report findings.
If you do not find issues, simply comment: LGTM
