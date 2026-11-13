# Canonical batches: implementation notes for reviewers

These notes explain how canonical batches for wait-for-CoW orders are
implemented on `diego/wait-for-cow-canonical-batches`: how the pieces fit,
which design decisions to scrutinize, what is guaranteed and what isn't, and
what the tests cover.

The change builds on the branch's first commit, which parses
`enableWaitForCow` from the app-data and stores it as `orders.wait_for_cow`.

## Behavior

With `canonical-batch-interval` set in the autopilot's `[run-loop]` config:

- Orders whose app-data sets `enableWaitForCow` only trade in **canonical
  batches**: auctions the leader runs at or after fixed batch times, which fall
  every `interval` seconds since the Unix epoch.
- An order's **first batch** is the first batch time more than `cutoff`
  seconds after the order was created. All orders waiting for a batch enter the
  same auction, together with the regular orders.
- An order that isn't filled, or is only partly filled, waits for the next
  batch. It's left out of every auction in between.
- The orderbook rejects a wait-for-CoW order whose `validTo` ends before the
  order can take part in its first batch. The error is
  `InsufficientValidToForCanonicalBatch`.
- `GET /api/v1/orders/{UID}/status` reports waiting orders as `scheduled`.
- `GET /api/v1/canonical_batch_schedule`, which is unstable, tells the
  frontend when the next batch starts.

Without the setting, which is the default, nothing changes:

- wait-for-CoW orders trade like regular orders;
- the schedule endpoint returns 404;
- no new order events are written.

## How it works

### One batch period

```text
                      cutoff (c)
                      ├────────┤
  ────────────────────┼────────┼──────────────────────────────┼────▶ wall-clock time
                    T − c      T                         T + interval
                                │
                                └─ batch T: the first auction the leader starts
                                   with its clock at or after T

  created before T − c                 ──▶ first batch is T
  created in [T − c, T + interval − c) ──▶ first batch is T + interval
```

In practice the batch for `T` is the auction built on the first block after
`T`, unless an auction is still running at that point. If no auction runs
between two batch times, the missed batches merge into one.

### Placing an order (orderbook)

1. For wait-for-CoW orders only, `OrderValidator` reads the published schedule.
2. It then takes the creation date, in the same place as before, right before
   the order is built.
3. It checks
   `validTo ≥ first_batch(created) + interval + min_order_validity_period`
   ([order_validation.rs:581](../crates/shared/src/order_validation.rs#L581),
   called at
   [order_validation.rs:1211](../crates/shared/src/order_validation.rs#L1211)).
4. The order is stored as before.

If reading the schedule fails, placing a wait-for-CoW order fails with a 500.
Regular orders don't read the schedule.

### Building auctions (autopilot)

1. **Every cache update** runs the usual filters on every order, wait-for-CoW
   orders included: in flight, balance, dust, native price and so on. It then
   splits the result.
   - Regular orders go into `RawAuctionData`.
   - Wait-for-CoW orders go into `Inner::wait_for_cow`
     ([solvable_orders.rs:331](../crates/autopilot/src/solvable_orders.rs#L331)).
   - With canonical batches off, there is no split.
2. **The run loop reads the clock before `update_caches`**
   ([run_loop.rs:270](../crates/autopilot/src/run_loop.rs#L270)). It then calls
   `canonical_batch_to_run(schedule, now, last_canonical_batch)`
   ([run_loop.rs:295](../crates/autopilot/src/run_loop.rs#L295),
   [run_loop.rs:882](../crates/autopilot/src/run_loop.rs#L882)). This returns the
   latest batch time unless this instance already ran that batch.
3. **`cut_auction` calls `current_auction(batch)`**
   ([solvable_orders.rs:204](../crates/autopilot/src/solvable_orders.rs#L204)).
   - With `Some(T)`, it adds the wait-for-CoW orders whose
     `first_batch(created) ≤ T` to the auction.
   - Every other wait-for-CoW order comes back in the `waiting` list.
4. **`cut_auction` records the waiting orders.** It stores a
   `Filtered(WaitingForCanonicalBatch)` event for each order in `waiting`
   ([run_loop.rs:448](../crates/autopilot/src/run_loop.rs#L448)). It then
   archives and returns the auction exactly as before.
5. **The run loop marks the batch as run.** Once `next_auction` returns the
   batch auction for solving, it sets `last_canonical_batch`. If no auction
   runs, the batch stays pending and the next iteration tries again. That
   happens when the auction is empty, identical to the previous one, or got no
   auction ID.

### Publishing the schedule

- **When an instance starts leading,** the run loop writes `(interval, cutoff)`
  to `canonical_batch_schedule`, or deletes the row when the feature is off
  ([run_loop.rs:253](../crates/autopilot/src/run_loop.rs#L253),
  [persistence/mod.rs:1046](../crates/autopilot/src/infra/persistence/mod.rs#L1046)).
  - With the leader lock off, that's on startup.
  - With the lock on, it's whenever `just_stepped_up()` is true.
  - A failed write is retried on the next iteration.
- **The orderbook reads it through `ScheduleCache`**, which uses the read pool
  and keeps the value for 5 seconds
  ([orderbook/canonical_batch.rs:18](../crates/orderbook/src/canonical_batch.rs#L18)).
  Both validation and the endpoint use this cache.

## Design decisions to scrutinize

1. **The schedule uses wall-clock time on a grid aligned to the epoch.**
   - Batch times need no coordination or stored state.
   - It's the same clock `valid_from` uses.
   - Cost: a clock difference between the orderbook and autopilot hosts eats
     into the cutoff.

2. **The cutoff closes the "late sighting" race.** Without it, an order could
   be created before `T` but only be visible to the autopilot after batch `T`
   was built:
   - an API order gets its creation date when validation finishes, and is
     committed a moment later;
   - an on-chain order is indexed seconds after its block.

   The invariant that makes the cutoff work is in `run_forever`:
   - the run loop reads its clock before the cache reads the database;
   - it only runs batch `T` once that clock has reached `T`.

   So every order created before `T − c` has had at least `c` seconds to be
   committed or indexed before the data batch `T` is built from was read. An
   order that takes longer than `c` misses `T` and waits for `T + interval`.

3. **The run loop decides which auction is a batch, and the cache only keeps
   the orders separate.**
   - Only the run loop knows whether an auction actually runs.
   - `current_auction(None)` never contains wait-for-CoW orders. A future caller
     that forgets the batch decision keeps these orders out of auctions, instead
     of letting them into every auction.

4. **The last batch run is kept in memory.**
   - `last_canonical_batch` lives in `run_forever`, next to `last_auction`.
   - After a restart or a change of leader, the first auction is a batch.
     That can repeat a batch the previous leader already ran in the same
     period, which gives orders an extra attempt but breaks nothing.
   - Recording batches on `competition_auctions` would prevent the repeat.
     That's left as a follow-up.

5. **The `scheduled` status comes from a new filter reason, written by the run
   loop.**
   - After a batch, an unfilled order's latest event is `Ready` or
     `Considered`, which would show as `active` or `solved` for up to a whole
     interval.
   - The new `waiting_for_canonical_batch` reason maps to `scheduled`
     ([orderbook.rs:630](../crates/orderbook/src/orderbook.rs#L630)).
   - The events are written in `cut_auction`, not in the cache. Each event
     batch is written by its own background task, which takes the timestamp
     when it runs. If the cache wrote the waiting event, an order in a batch
     would get it at almost the same moment as `Ready`, and could end up with
     the waiting event as its latest one.
   - Cost: one event-insert task per auction for the waiting set. The insert
     skips orders whose latest event is the same, as the cache's own filter
     events already do.

6. **The schedule reaches the orderbook through the database.**
   - Configuring the interval in the orderbook too could drift from the
     autopilot.
   - The table holds one row at most. `singleton boolean PRIMARY KEY CHECK
     (singleton)` enforces that, and also gives the upsert its `ON CONFLICT`
     target.

7. **The validation margin is `first_batch + interval +
   min_order_validity_period`.**
   - The autopilot drops orders with less than its own minimum validity left
     (1 minute by default). It measures that from when it reads the orders,
     not from the batch time.
   - The extra interval keeps the order eligible as long as its batch starts
     less than one interval late.
   - The orderbook's `min_order_validity_period` stands in for the
     autopilot's. Both default to 1 minute.
   - This is strict for long intervals: a 15-minute interval needs at least
     16 minutes of validity after the first batch. This is an open question.

## Guarantees and limits

With the feature on, these hold:

- A wait-for-CoW order is never in an auction that isn't a canonical batch.
- Every wait-for-CoW order whose first batch is `T` or earlier, and that
  passes the usual filters at that moment, is in the batch `T` auction.
- The batch an order first joins depends only on its creation time and the
  published schedule. The orderbook and the frontend compute the same value.

These don't:

- **That an order trades in its first batch.** It moves to the next batch if:
  - the autopilot is down at the batch time;
  - indexing lags by more than the cutoff;
  - the order is filtered at that moment, for example for balance or a
    missing native price.
- **Exactly one batch per period.** A restart or a change of leader can
  repeat a batch.
- **An exact start time.** A batch starts up to one block after its batch
  time, plus any auction still running.
- **A matching frontend view during a rolling deploy that changes the
  schedule.** The published row switches when the new version starts leading.
  A frontend that fetched the schedule before that, or an orderbook with the
  old value cached (up to 5 seconds), shows the old times until it refreshes.

## Code map

Suggested reading order:

| # | File | What to look at |
|---|---|---|
| 1 | [shared/src/canonical_batch.rs](../crates/shared/src/canonical_batch.rs) | `Schedule` and `first_batch` (the membership rule), `ScheduleReading` |
| 2 | [configs/src/autopilot/run_loop.rs](../crates/configs/src/autopilot/run_loop.rs#L107-L139) | the two settings and `validate` |
| 3 | [autopilot/src/solvable_orders.rs](../crates/autopilot/src/solvable_orders.rs#L147-L221) | `Inner::wait_for_cow`, `CurrentAuction`, `current_auction`; the split at L331; `split_wait_for_cow` and `select_canonical_batch_orders` at L558-L595 |
| 4 | [autopilot/src/run_loop.rs](../crates/autopilot/src/run_loop.rs#L222-L317) | `run_forever`: publishing, clock read, batch decision. Also `cut_auction` (L425), `publish_canonical_batch_schedule` (L485), `canonical_batch_to_run` (L882), metrics (L1069) |
| 5 | [autopilot/src/infra/persistence/mod.rs](../crates/autopilot/src/infra/persistence/mod.rs#L1046) | `publish_canonical_batch_schedule` |
| 6 | [database/sql/V136…](../database/sql/V136__waiting_for_canonical_batch_filter_reason.sql), [V137…](../database/sql/V137__canonical_batch_schedule.sql), [database/src/canonical_batch_schedule.rs](../crates/database/src/canonical_batch_schedule.rs) | migrations, `upsert`/`delete`/`load`; also `OrderFilterReason` in `order_events.rs` and `TABLES` in `lib.rs` |
| 7 | [orderbook/src/canonical_batch.rs](../crates/orderbook/src/canonical_batch.rs), [orderbook/src/database/canonical_batch_schedule.rs](../crates/orderbook/src/database/canonical_batch_schedule.rs) | `ScheduleCache`, and loading and validating the published row |
| 8 | [shared/src/order_validation.rs](../crates/shared/src/order_validation.rs#L566-L594) | `with_canonical_batch_schedule`, `check_valid_through_first_canonical_batch`, call site at L1200-L1215 |
| 9 | [orderbook/src/api/get_canonical_batch_schedule.rs](../crates/orderbook/src/api/get_canonical_batch_schedule.rs), [api.rs](../crates/orderbook/src/api.rs#L284), [post_order.rs](../crates/orderbook/src/api/post_order.rs#L254), [orderbook.rs](../crates/orderbook/src/orderbook.rs#L630), [run.rs](../crates/orderbook/src/run.rs#L444) | endpoint, route, error mapping, status mapping, wiring |
| 10 | [orderbook/openapi.yml](../crates/orderbook/openapi.yml), [database/README.md](../database/README.md) | API and schema docs |
| 11 | [e2e/tests/e2e/wait_for_cow.rs](../crates/e2e/tests/e2e/wait_for_cow.rs) | the end-to-end scenario |

## Database and deploy order

- **Migrations:** V136 adds the `waiting_for_canonical_batch` value to
  `OrderFilterReason`, on its own as earlier enum changes did. V137 creates
  `canonical_batch_schedule`. Both only add things.
- **Deploy order matters for the new enum value.**
  - The orderbook decodes `order_events.reason` when it serves `/status`. An
    orderbook built before this change can't decode
    `waiting_for_canonical_batch`, so `/status` would fail for waiting orders.
  - The autopilot only writes that reason when the interval is set. So deploy
    both services with the interval unset, then turn it on.
  - Any other reader of `order_events.reason`, such as analytics exports,
    needs to know the new value too.
- **The table:** older autopilot versions ignore it, and the orderbook treats
  a missing row as "off".

## API changes

- **New endpoint:** `GET /api/v1/canonical_batch_schedule`, marked unstable.
  - It returns 200 with `{intervalSeconds, cutoffSeconds, nextBatch,
    serverTime}`, or 404 when canonical batches are off.
  - `serverTime` lets a countdown ignore the client's clock.
- **New error:** `OrderPostError` gains the type
  `InsufficientValidToForCanonicalBatch`, returned with status 400.
- **Status:** `/status` maps `Filtered` with `waiting_for_canonical_batch` to
  `scheduled`.
- **Docs:** the description of `waitForCow` on orders now covers the
  canonical-batch behavior.

## Operations

- **Config:**

  ```toml
  [run-loop]
  canonical-batch-interval = "1m"
  canonical-batch-cutoff = "5s" # default
  ```

  Both must be whole seconds. The interval must be between 1 second and
  `i32::MAX` seconds, and the cutoff must be shorter than the interval. Any
  other value fails at startup.
- **Metrics:**
  - `canonical_batch_delay` (histogram) and `canonical_batch_orders` (gauge),
    both in the run loop's `runloop` subsystem;
  - `canonical_batch_waiting_orders` (gauge) in the solvable-orders metrics;
  - `auction_solvable_orders` now counts only regular orders.
- **Logs:**
  - `canonical batch` (info, with `batch_time`, `orders` and `delay`);
  - `published canonical batch schedule` (info);
  - `failed to publish canonical batch schedule` (error).
- **Rollout:**
  1. Deploy both services with the interval unset.
  2. Set the interval on barn, for example `1m`.
  3. Watch the waiting orders, the batch delay, and how many batches produce
     CoWs.
- **Rollback:** unset the interval. Waiting orders then trade in the next
  auction, and the endpoint returns 404 once the new leader starts.

## Tests

| Kind | What | Result |
|---|---|---|
| Unit | schedule math, cutoff boundaries, overflow (`shared`) | pass |
| Unit | config defaults, parsing and validation (`configs`) | pass |
| Unit | `canonical_batch_to_run`, `split_wait_for_cow`, `select_canonical_batch_orders` (`autopilot`) | pass |
| Unit | validation limit with a fixed creation time; end-to-end validation with and without a schedule (`shared`) | pass |
| Unit | endpoint response and serialization (`orderbook`) | pass |
| Postgres | schedule round trip, cutoff `CHECK`, new reason round trip; full suites for `database`, `autopilot`, `orderbook` (112 tests) | pass |
| e2e | `local_node_wait_for_cow` (about 45 s); see below | pass |
| e2e, feature off | `valid_from`, order cancellation, two limit orders, dual autopilot leader | pass |
| Lint | `cargo clippy --workspace --all-features --all-targets -D warnings`, nightly `fmt` | clean |

`local_node_wait_for_cow` checks that:

- the schedule endpoint returns the configured interval and cutoff;
- a wait-for-CoW order with too short a `validTo` is rejected;
- a regular order placed alongside the wait-for-CoW orders trades before the
  batch;
- the wait-for-CoW orders stay out of auctions with status `scheduled` until
  the batch;
- the two orders that can trade, plus the unfillable one, were in the same
  batch auction (checked through the solver competition data);
- the unfillable order is only in auctions right after batch times, waiting
  in between (checked through its order events).

Not covered:

- publishing on a change of leader with the feature on (the dual-autopilot
  test runs with it off);
- `ScheduleCache` expiry;
- the `/status` mapping, which is only checked by the e2e test;
- behavior with many waiting orders;
- whether the two matching orders settle in the same transaction, which
  depends on the solver.

## Smaller things to know

- An auction's `prices` still include the tokens of waiting orders. This is
  harmless.
- Wait-for-CoW orders get fees, quotes and penalty caps attached on every
  cache update, as regular orders do, whether or not a batch is due.
- When the feature is first turned on, the first auction after startup is a
  batch for the latest batch time, which may have passed up to an interval
  earlier.

## Open questions

1. Should the orderbook reject `enableWaitForCow` when canonical batches are
   off, instead of treating those orders as regular orders?
2. Follow-ups:
   - record canonical batches on `competition_auctions`, to avoid repeats
     after a change of leader and for analytics;
   - tell solvers in `/solve` that an auction is a canonical batch.
