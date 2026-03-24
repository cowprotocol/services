# Code Review Guidelines

These instructions guide automated code reviews. Focus on real bugs and
correctness issues. Do not pad reviews with praise or filler.

## Priorities

Review in this order. Stop at the first category that has findings — do not
bury critical bugs under a wall of style nits.

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
- Database queries include appropriate indexes and won't degrade at scale
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

- **Bug** — will cause incorrect behavior; must fix before merge
- **Nit** — minor improvement, not blocking
- **Question** — unclear intent, needs author clarification

If unsure whether something is a real issue, mark it as a **Question** rather
than asserting a bug.

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
- Suggested fix (code snippet if non-obvious)

Do not summarize the PR. Do not list what looks correct. Only report findings.
If there are no issues, say so in one sentence.
