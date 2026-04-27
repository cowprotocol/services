# Review Context — Database Migrations

Loaded by `COW_PR_REVIEW_SKILL.md §3` when a PR touches `database/sql/**`.

This file extends — does not replace — the reminder in `.github/nitpicks.yml`. The nitpick bot posts the generic warning automatically; the reviewer applies judgment using the checks below.

The checks are dictated by *how* we roll out — not by SQL itself.

## Rollout shape

Two facts shape every check below:

1. **k8s rolls pods, not the cluster.** During a release, the new autopilot starts and runs Flyway *before* the old pod is shut down. For a non-trivial overlap window the previous version is processing requests against the new schema. Anything that breaks that path breaks production.
2. **Staging and production are independent and roll out at different times.** A migration lands in staging first; production follows on its own cadence. While the gap is open, our shadow environment can be running an *older* code version against the *newer* schema (or vice versa, briefly). Any change that breaks that combination breaks shadow without breaking the obvious path the author tested.

Both points generalise beyond DB — config schema changes, request/response formats, and message-queue payloads have the same n-1 / staging-vs-production caveat. When a PR touches any of those *and* the DB, mention the cross-cutting risk in the synthesis.

## Non-negotiables

Each item carries its own severity. Items 1–3 are correctness/availability concerns and default to **High**; items 4–5 are author hygiene and default to **Medium**.

1. **Reversibility.** State whether the migration is reversible. If yes, include or link the rollback script. If no, the PR explains *why* irreversibility is acceptable. Missing either → **High**.
2. **n-1 schema compatibility.** The previous app version must still function against the new schema. A migration that drops or renames a column, narrows a type, or adds a `NOT NULL` constraint without code already tolerating both shapes → **High** until the rollout plan is spelled out (typically: ship change in three releases — add → migrate code → drop).
3. **Blocking index creation on hot tables.** Use `CREATE INDEX CONCURRENTLY` on anything in the auction/settlement critical path (`orders`, `trades`, `auctions`, `settlements`, `order_events`, `auction_orders`, `quotes`, `order_quotes`). A blocking `CREATE INDEX` on one of these → **High**.
4. **Authoritative table list.** New tables must appear in `crates/database/src/lib.rs` (search for the top-level table list). Missing → **Medium** (CI may also catch it; still the author's responsibility).
5. **README.** Schema changes update the SQL or DB README, whichever is the convention at review time. Missing → **Medium**.

## Other shapes that usually warrant **High**

- `ALTER TABLE ... ADD COLUMN NOT NULL` on a multi-million-row table without a default — table lock plus slow backfill. Remedy: add nullable, batched backfill, then `NOT NULL` in a later migration.
- Renaming a column in a single migration rather than the three-release add → migrate → drop pattern.
- Adding a `UNIQUE` constraint without first verifying current data is unique (migrations fail when duplicates exist).
- `ALTER COLUMN ... TYPE ...` on a large table without a multi-step migration plan — Postgres rewrites every row and holds an `ACCESS EXCLUSIVE` lock for the duration. Remedy: new column, dual-write, backfill, swap, drop old.

## Usually worth flagging as **Medium**

- New foreign keys without an explicit `ON DELETE` clause (default `NO ACTION` is often surprising).
- New tables without indexes on the columns obvious queries will filter on.

## Not findings

- SQL style — trailing commas, keyword casing, indentation.
- Whether the migration could be one statement instead of three. If correct, accept it.
- Migration filename style — Flyway naming is enforced mechanically.

## Questions worth asking the author

When in doubt, prefer a `Question:` over a flagged `Action:`:

- *"What's the expected row count in `<table>` at rollout time?"* — drives `CONCURRENTLY` and batched-backfill decisions.
- *"Which release pairs this migration with the code change?"* — makes the n-1 reasoning explicit.
- *"How does this look on shadow during the staging→production gap?"* — surfaces cross-environment compatibility before the author finds out by paging.
- *"Has the rollback script been tested against a production-sized dataset?"* — only relevant if a rollback was included.
