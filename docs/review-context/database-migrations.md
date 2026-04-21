# Review Context — Database Migrations

Loaded by `COW_PR_REVIEW_SKILL.md §3` when a PR touches `database/sql/**`.

This file extends — does not replace — the reminder in `.github/nitpicks.yml`. The nitpick bot posts the generic DB warning automatically; the reviewer applies judgment using the checks below.

## Non-negotiables (all **High** until addressed)

1. **Reversibility.** The migration must state whether it's reversible. If yes, the rollback script is present or explicitly linked in the PR description. If no, the PR explains *why* irreversibility is acceptable for this change. Missing either → **High**.

2. **Compatibility window.** During rollout, k8s starts the new autopilot, runs Flyway, and only then shuts down the old pod. That overlap means the **previous** version must still be able to process requests on the **new** schema. Any migration that breaks the old schema path (e.g. drops a column the old code reads) → **High** until the PR lays out an n-1 compatibility plan, typically:
   - Release A: make the change compatible (add new column, keep old).
   - Release B: ship code that uses the new column.
   - Release C: drop the old column.

3. **Blocking index creation on hot tables.** Tables in the critical path — `orders`, `trades`, `auctions`, `settlements`, `order_events`, `auction_orders`, `quotes`, `order_quotes` — must use `CREATE INDEX CONCURRENTLY`. A blocking `CREATE INDEX` on one of these → **High**.

4. **Table-list update.** New tables must be added to the authoritative list at `crates/database/src/lib.rs:51-87`. Missing → **Medium** (not High because a CI check may also catch it, but it's still the author's responsibility).

5. **DB README update.** Schema changes must update the DB README (`crates/database/README.md` or the SQL folder's README — whichever is the convention at review time). Missing → **Medium**.

## Usually worth flagging (**High**)

- `ALTER TABLE ... ADD COLUMN NOT NULL` on a multi-million-row table **without** a default → table lock + slow backfill → **High**. Remedy: add as nullable, backfill in a batched job, then add `NOT NULL` in a later migration.
- Dropping a column still referenced in code the old pod runs → **High** (see §2 compatibility window).
- Renaming a column in a single migration (rather than add-new + migrate + drop-old across three releases) → **High**.
- Adding a `UNIQUE` constraint without verifying current data is unique → **High** (migration will fail in prod if duplicates exist).
- Changing a column's type in place (e.g. `VARCHAR(32)` → `VARCHAR(64)`) on a large table without a plan for the rewrite cost → **Medium** to **High** depending on table size.

## Usually worth flagging (**Medium**)

- Migration scripts without comments explaining the "why" when the "what" is non-obvious.
- Introducing a foreign key without `ON DELETE` behavior being specified (the default is `NO ACTION`, which can be surprising).
- New tables without indexes on columns that obvious queries will filter on.

## Not findings

- SQL style: trailing commas, keyword casing, indentation.
- Whether the migration could have been one statement instead of three — if it's correct, it's correct.
- Whether the migration file should have been named differently (Flyway naming convention is enforced mechanically).

## Questions worth asking the author

When reviewing, these often deserve a `Question:` in a finding:

- "What's the expected row count in `<table>` in production at rollout time?" (drives the need for `CONCURRENTLY`, batched backfill).
- "Which release are we pairing this migration with on the code side?" (drives the n-1 compatibility reasoning).
- "Have you tested the rollback script against a production-sized dataset?" (if one is included).
