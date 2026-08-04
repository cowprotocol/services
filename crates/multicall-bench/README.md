# multicall-bench

Local-only harness measuring what `Multicall3` batching buys the autopilot's
balance fetching. **Not meant to be committed** — it shows up as untracked in
`git status`, so stage the PR's files explicitly rather than with `git add -A`.
Being a workspace member also puts a `multicall-bench` entry in `Cargo.lock`;
drop that before committing.

It drives the production `BalanceFetching` implementation through the production
`ethrpc` provider stack, so the numbers include the JSON-RPC batching layer and
the individual-call fallback, not a hand-rolled approximation of them.

## Usage

Dump the working set once, then replay it as often as you like:

```bash
set -a; source /path/to/.env.claude; set +a   # COW_DB_*, ETH_MAINNET_RPC

cargo run --release -p multicall-bench -- dump \
    --network mainnet --output /tmp/mc-fixture-mainnet.json

NODE_URL="$ETH_MAINNET_RPC" cargo run --release -p multicall-bench -- run \
    -f /tmp/mc-fixture-mainnet.json \
    --multicall-batch-size 0,25,50,100,200 \
    --repeat 3 --json /tmp/sweep.json
```

`dump` runs the `OPEN_ORDERS` conditions from `database::orders::solvable_orders`
reduced to the distinct `(owner, sell_token)` pairs, keeping only ERC20 sell
sources without pre-interactions — the queries that actually reach the batched
path. Mainnet yields ~3.1k pairs over ~870 tokens.

Axes, each a comma-separated list, swept as a cartesian product:

- `--multicall-batch-size` — queries per `Multicall3` call. `0` reads them
  individually and is the config every other one is compared against.
- `--ethrpc-batch-size` — JSON-RPC calls coalesced into one HTTP request. `0`
  or `1` removes the batching layer (and forces unlimited concurrency, which is
  what `ethrpc` itself does for that combination).
- `--ethrpc-concurrency` — in-flight batches, `0` for unlimited.
- `--ethrpc-batch-delay-ms` — the batch layer's nagle delay.

`--json` / `--csv` dump per-pass numbers for plotting.

`--block` pins every read to one block: `finalized` (the default), `safe`,
`latest`, a number, or `none` to follow the chain the way production does. Tags
are resolved to a concrete number once at startup, because pinning to the tag
itself would let the block advance underneath the matrix. Pinning is what makes
`mism` and `moved` go to zero — see the caveat below before reading latencies
off a pinned run.

## Reading the output

- `calls` counts **logical** JSON-RPC calls. `ethrpc`'s instrumentation layer
  sits above its batching layer (`crates/ethrpc/src/alloy/mod.rs`), so this is
  the count before coalescing — despite what the metric's own doc comment says.
  `http~` divides it by the batch size as an estimate; nothing below the
  batching layer is instrumented, so the real HTTP count is not measured.
- `mism` counts results differing from the baseline config, `moved` counts
  results differing between a config's own first and last pass. Same code path
  in the second case, so `moved` is the noise floor: only `mism` clearly above
  it is a real difference between the batched and unbatched paths.
- `min_ms` is the most robust column. Node-side variance is large and the median
  only means anything from `--repeat 3` up.

## Measurement traps this harness avoids

- **Synthetic working sets invert the result.** A handful of tokens and
  zero-balance owners sit entirely in the node's state cache, which makes the
  *individual* path look faster. Only the real token-diverse set shows the win,
  which is why there is a `dump` subcommand and no address generator.
- **Cold node state.** The first config through would otherwise pay to warm the
  node's cache for everyone after it, so `--warmup` passes over the whole
  working set run before the matrix starts.
- **Cold connections and the `Multicall3` lookup.** Each config gets a fresh
  provider, so before timing it runs a two-query pass to open the HTTP
  connections and resolve the one-off `eth_getCode` for `Multicall3`.

## Findings, mainnet, 3094 pairs, 2026-08-04

```
  mc  rpc_batch  conc   min_ms   med_ms   calls   http~
────  ─────────  ────  ───────  ───────  ──────  ──────
   0         20    10     1410     1444    6188     310   <- production config today
   0         20    50      319      325    6188     310
   0        100    10      367      377    6188      62
   0        100    50      148      361    6188      62
  50         20    10      278      395      62       4   <- this PR
  50         20    50      187      191      62       4   <- best
  50        100    10      376      415      62       1
  50        100    50      269      278      62       1
```

Multicall batch size barely matters: 25, 50, 100 and 200 all land within noise
of each other (180-230ms min), so no gas-cap knee is visible even at 200 queries
= 400 sub-calls, and 50 is a perfectly good default.

The bigger finding is that at today's `ethrpc` settings the dominant cost is the
concurrency cap, not the call count: leaving the balance reads unbatched and
raising `ethrpc_max_concurrent_requests` from 10 to 50 gets 1444ms -> 325ms,
which beats what `Multicall3` achieves at concurrency 10 (395ms). The two levers
are partly substitutes for latency and compose for the best result (191ms).

For **RPC volume** they are not substitutes at all, and that is the real reason
to do this. Multicall takes a pass from 6188 logical calls to 62, and from ~310
HTTP requests to ~4. Raising the concurrency cap removes none of them — it sends
the same 6188 calls, just more of them at a time, so the node does identical
total work compressed into a shorter burst. It buys latency by pushing *more*
instantaneous load at the node, which is the opposite of what multicall does.

That matters for more than node CPU and bandwidth. Every autopilot component
shares one FIFO batch queue per provider (`ethrpc`'s `BatchCallLayer`), and
balance fetching is ~87% of the autopilot's RPC traffic — 6188 calls per block
flooding that queue is what makes unrelated work (block stream, maintenance
`getLogs`, run-loop cache misses) wait behind it. Cutting the balance reads 100x
frees the queue for everything else; no concurrency setting does that.

So: multicall is the change worth keeping, on volume grounds. The latency win is
a bonus, and part of it is available without multicall at all.

(The shared-queue contention is from the earlier autopilot perf sweep, not
measured by this harness. This harness measured the call counts and the wall
times in the table above.)

Parity is clean. Unpinned, 9 pairs differ from the baseline — rebasing tokens
whose balances grow every block (e.g. `30752038445 -> 30752038831`). The same
drift shows up between two passes of the *individual* path, so it is on-chain
movement, not a decoding difference. Pinned with `--block finalized`, `mism` and
`moved` are **0 across the whole matrix**, which is the clean statement that the
batched path decodes identically to the unbatched one.

## Pinned latencies are not production latencies

Pinning fixes correctness comparisons but changes what is being measured. A
pinned read is a *historical* state read; production reads the latest state,
which the node serves out of hot memory. Same matrix as above, `--block
finalized` (~74 blocks back):

```
  mc  rpc_batch  conc   min_ms   med_ms   calls          vs unpinned
────  ─────────  ────  ───────  ───────  ──────   ──────────────────
   0         20    10     1543     1546    6188    1410 — about same
   0         20    50      373      377    6188     319 — about same
   0        100    50      242      243    6188     148 — fastest here
  25         20    10      510      759     124     ~200 unpinned
  25        100    10      447      500     124     ~200 unpinned
```

Unpinned, multicall batch size was flat from 25 to 200 (180-230ms). Pinned, it
degrades monotonically — 25:349, 50:526, 100:602, 200:569 min ms — and **no
multicall config beats the best individual config** (242ms at batch 100 /
conc 50). The individual path barely changes when pinned; the multicall path
roughly doubles.

The likely reason is that cold historical-state execution makes per-sub-call work
dominate over per-`eth_call` overhead, and 6188 separate `eth_call`s can be spread
across the node's handlers whereas one `Multicall3` execution cannot be split.
That is consistent with geth running batch entries sequentially (see below) while
still parallelising across separate requests. Unverified — it is inference from
these numbers, not something the harness measures.

Practical split:

- **Parity checking → pin.** `mism` must be 0; anything else is a real bug.
- **Choosing the production batch size or `ethrpc` settings → do not pin.**
  Production reads latest state, so only the unpinned run answers that question.
- **RPC volume → either.** 6188 -> 62 calls is identical in both regimes, and
  that is the argument this PR actually rests on.

## What the node does with either shape

**geth executes the calls inside one JSON-RPC batch sequentially**, in a single
`callProc` goroutine — `handleBatch` in [`rpc/handler.go`][handler] loops over
the buffered calls and runs `handleCallMsg` one at a time. (The "RPCs are
processed on background goroutines" comment there is about the batch as a whole
not blocking the connection, not per-call concurrency; several blog posts get
this wrong.) So a JSON-RPC batch buys round-trips, not execution parallelism,
while a `Multicall3` call pays the per-`eth_call` setup — state overlay, EVM
instance, block resolution, JSON parse and serialise — once for the whole fan-out.

Server-side accounting follows from that: per [Chainstack][chainstack], *"1 batch
request of 100 calls consumes 100 requests instead of 1"*, whereas a multicall
counts as one. That is what rate limits and per-request billing see.

There is also a **correctness** argument this PR does not currently make.
[Multicall3][multicall3] guarantees *"all values returned are from the same
block"*; batch entries do not, and Chainstack notes that a block arriving
mid-batch changes the later results. `min(balance, allowance)` is only meaningful
if both reads come from the same state.

Node-side ceilings, all [geth defaults][geth-cli]:

```
--rpc.gascap                    50000000   total gas for one eth_call
--rpc.evmtimeout                      5s   wall time for one eth_call
--rpc.batch-request-limit           1000   entries per JSON-RPC batch
--rpc.batch-response-max-size   25000000   bytes returned per batch
```

200 queries = 400 sub-calls at roughly 5-8k gas each (cold account access + cold
SLOAD + call overhead) is ~2-3M gas, some 20x under the cap — a hand estimate,
but consistent with the flat timings and implying the real ceiling is thousands
of queries per call, not hundreds. Note the batch-request limit also caps
`--ethrpc-batch-size` at 1000 on the individual path.

Worth knowing: [`eth_simulateV1`][simulate] (geth >= 1.14.9, Nethermind >=
1.28.0) is native multicall over JSON-RPC with no deployed contract needed, so it
is an option on chains where the `eth_getCode` check finds no `Multicall3`.

Two open questions:

- **Reth/jsonrpsee batch concurrency is unverified.** Searching only turned up
  the JSON-RPC spec's permissive "may process a batch as concurrent tasks"
  wording, not reth or jsonrpsee source. A reth-backed endpoint may not behave
  like the geth model above.
- **Our endpoint does not match the geth model.** If batch entries ran
  sequentially, `batch=100/conc=10` should be no faster than `batch=20/conc=10`
  (same 10 streams, same 6188 executions); we measured 377ms vs 1444ms. So
  per-request overhead *outside* EVM execution dominates here, which points at a
  proxy or load balancer in front of the node handling batches differently.
  Worth resolving before tuning `ethrpc` settings on the strength of the geth
  model alone.

[handler]: https://github.com/ethereum/go-ethereum/blob/master/rpc/handler.go
[chainstack]: https://docs.chainstack.com/docs/http-batch-request-vs-multicall-contract
[multicall3]: https://github.com/mds1/multicall
[geth-cli]: https://geth.ethereum.org/docs/fundamentals/command-line-options
[simulate]: https://geth.ethereum.org/docs/interacting-with-geth/rpc/ns-eth
