---
name: order-batch-debug
description: Debug why a batch of orders failed to execute (or executed slowly). Given a CSV / list of order UIDs, classify each one as truly expired vs filtered-out-earlier, identify the quoting solver, check whether any solver bid on it, and — for solvers that are not co-located — get the driver-side discard reason.
---

## When to use

- User shares a list of order UIDs and asks "why did these expire?" or "why are they slow?"
- Class of orders from a partner / appCode all expiring on a specific network — find the dominant root cause.
- Comparing two quoters' fill rates on their own quoted orders.

## Inputs

- A list/CSV of order UIDs (114-char hex with `0x` prefix). Network is **not** required — the skill probes `api.cow.fi/{network}` to discover it.
- (optional) Time window if you already know it; otherwise decode `validTo` from the trailing 4 bytes of each UID.

## What you'll find out

Per order:

| Column | Values | Source |
|---|---|---|
| `expired` | `yes` (in auction at validTo) / `no` (removed earlier) / `unknown` | debug.cow.fi `/events` last label |
| `expired_detail` | `in_auction_at_validTo` / `invalid_*` / `filtered_from_auction` / `never_qualified_for_auction` / `cancelled` / `no_record` | same |
| `quoter` | submission address | API `quote.solver` |
| `quoter_name` | `tsolver` / `flowdesk-solve` / `kipseli` / … | log mapping (`Creating solver`) or repo config |
| `did_bid` | `yes` / `no` / `unknown` | autopilot `proposed solution` ∪ driver `discarded solution` |
| `bid_layer` | `autopilot` / `driver_discarded` / `both` | which log surfaced it |
| `discard_reason` | text | driver `parsed.fields.err` / order final state |

Per-quoter aggregates: counts of expired-vs-removed-early, did-bid-yes vs no, and a histogram of discard reasons.

## Requirements

| Need | For what |
|---|---|
| `CoW-Prod` MCP (VictoriaLogs) | autopilot `proposed solution` + driver `discarded solution` queries |
| HTTPS access to `api.cow.fi` | order details (`status`, `quote.solver`, `validTo`) |
| HTTPS access to `https://debug.cow.fi/api/orders/{uid}/events?chainId=N` | per-order lifecycle (basic auth; ask user for credentials and store in .env.claude) |

## Co-located vs in-cluster solvers (CRITICAL)

Driver-side discard logs are only visible for solvers running in **our** shared driver pod (`<network>-driver-prod-liquidity`). Co-located solvers run their own driver in their own infra and we do **not** see their internal logs.

Discover the full set of solvers and their co-location status from the autopilot's startup logs:

```
container:=<NETWORK>-autopilot-prod AND _msg:="Creating solver"
| fields _time, parsed.fields.name, parsed.fields.url, parsed.fields.submission_address
```

Each row gives you `(name, url, submission_address)`. Two signals tell you whether a solver is co-located:

1. **URL pattern (primary).** If `parsed.fields.url` resolves to an in-cluster Kubernetes service (host ends in `.svc.cluster.local`, e.g. `bnb-driver-prod-liquidity.services.svc.cluster.local/<solver>/`), the solver runs in our shared driver pod and its driver logs are queryable. If it points to an external host (e.g. `eu-ssb.api.tokkalabs.com`, `cow-driver.knstats.com`, `cow-api.portus.xyz`), the solver is co-located.
2. **Driver-log presence (fallback / cross-check).** Issue a `victorialogs_stats_query_range` against `container:<NETWORK>-driver-prod-liquidity AND `parsed.spans./solve.solver`:<name>` over the time window of interest. **Zero hits ⇒ assume co-located.** A non-zero count confirms in-cluster.

Use these together: derive the list from the URL pattern, then sanity-check the in-cluster bucket with a single stats query — anyone with zero driver-log hits gets demoted to "co-located, opaque" regardless of URL (e.g. driver pod was renamed, deployment was paused, etc.).

For co-located solvers, `did_bid` can only be set from the autopilot `proposed solution` log — if there's no autopilot entry the answer is `unknown`, not `no`. Mark this clearly in the output rather than reusing `no`.

---

## Step 1 — Bulk-fetch order details from the API

Probe one network first to find which the orders are on (for a homogeneous batch try `mainnet`, `bnb`, `arbitrum-one`, `base`, `xdai`, `polygon`, `avalanche`, `gnosis`, `sepolia`, `linea`, `ink`, `plasma`).

```python
# /tmp/cow_debug/fetch_orders.py
import json, urllib.request, urllib.error, ssl, gzip
from concurrent.futures import ThreadPoolExecutor, as_completed

NETWORK = "mainnet"   # or whatever you confirmed
ORDERS = open("orders.txt").read().strip().splitlines()
ctx = ssl.create_default_context()

def fetch(uid):
    url = f"https://api.cow.fi/{NETWORK}/api/v1/orders/{uid}"
    try:
        req = urllib.request.Request(url, headers={"Accept-Encoding":"gzip"})
        with urllib.request.urlopen(req, timeout=30, context=ctx) as r:
            data = r.read()
            if r.headers.get("Content-Encoding") == "gzip":
                data = gzip.decompress(data)
            j = json.loads(data)
        return uid, {
            "uid": uid,
            "status": j.get("status"),
            "validTo": j.get("validTo"),
            "creationDate": j.get("creationDate"),
            "owner": j.get("owner"),
            "sellToken": j.get("sellToken"),
            "buyToken": j.get("buyToken"),
            "quote_solver": (j.get("quote") or {}).get("solver"),
            "quote_verified": (j.get("quote") or {}).get("verified"),
            "signingScheme": j.get("signingScheme"),
            "class": j.get("class"),
        }, None
    except urllib.error.HTTPError as e:
        return uid, None, f"HTTP {e.code}"
    except Exception as e:
        return uid, None, f"ERR {type(e).__name__}: {e}"

with open("orders.jsonl","w") as out, open("orders_errors.txt","w") as err, \
     ThreadPoolExecutor(max_workers=24) as ex:
    for uid, payload, e in (f.result() for f in as_completed(ex.submit(fetch, u) for u in ORDERS)):
        if payload: out.write(json.dumps(payload)+"\n")
        else:       err.write(f"{uid}\t{e}\n")
```

**Caveat — 404s are not bugs.** Orderbook prunes orders after a network-specific retention. Old orders return `404` even if they really existed; mark them `quoter=unknown, expired=unknown`. They may still appear in Victoria Logs if the time window is recent enough.

## Step 2 — Per-order lifecycle from `debug.cow.fi/events`

The `order_events` table compresses runs of identical labels — only label *transitions* are stored, so the **last** event tells you the final classification.

```python
import base64, os
# Credentials live in .env.claude (or 1Password); fetch from the user
# if they're not already in the environment. Do NOT inline them here.
USER = os.environ["DEBUG_COW_USER"]
PWD  = os.environ["DEBUG_COW_PWD"]
auth = base64.b64encode(f"{USER}:{PWD}".encode()).decode()

def fetch_events(uid, chain_id):
    url = f"https://debug.cow.fi/api/orders/{uid}/events?chainId={chain_id}"
    req = urllib.request.Request(url, headers={"Authorization": f"Basic {auth}"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.loads(r.read())   # [{timestamp, label}, …]
```

Map last label → classification:

| Last label | `expired` | `expired_detail` |
|---|---|---|
| `ready`      | `yes` | `in_auction_at_validTo` |
| `considered` | `yes` | `matched_in_winning_solution_but_never_settled` |
| `executing`  | `yes` | `settlement_attempted_but_failed` |
| `created`    | `no`  | `never_qualified_for_auction` |
| `invalid`    | `no`  | `invalid_(insufficient_balance/allowance/sig)` |
| `filtered`   | `no`  | `filtered_from_auction` |

The `/events` endpoint does **not** return the `OrderFilterReason` enum — to break `invalid` apart you need the autopilot logs (`filtered out` / `solvable_orders` lines).

## Step 3 — Solver address ↔ name mapping

`quote.solver` (from the order's API response) is an address. The autopilot logs the mapping at startup — pull it once per network and build an `{addr: name}` dict (and an `{addr: url}` dict for the co-location check from the previous section):

```
container:="<NETWORK>-autopilot-prod" AND _msg:="Creating solver"
| fields _time, parsed.fields.name, parsed.fields.submission_address, parsed.fields.url
```

The autopilot re-emits `Creating solver` whenever it restarts, so a recent window (e.g. last 7 days) reliably contains every solver. Filter by `submission_address` ∈ {addresses you saw in the batch's `quote.solver` values} if you want to keep the result small.

## Step 4 — Autopilot bids (`proposed solution`)

For each *quoter address* in the batch, ask Victoria Logs for the union of bids on its own-quoted UIDs. Use OR-batching (chunks of ≈30 UIDs per query — backtick-escape any field path containing `/`).

```
container:!controller AND network:<NETWORK>
  AND "proposed solution"
  AND `parsed.fields.driver`:<solver_name>
  AND ( all:0xUID1 OR all:0xUID2 OR … )
| fields _time, parsed.fields.orders, parsed.spans.auction.auction_id, parsed.fields.solution
```

The `parsed.fields.orders` field contains a stringified list — extract every 56-byte `0x[0-9a-f]{112}` to handle batched solutions.

For **all** solvers that bid (not just the quoter), drop the `parsed.fields.driver` filter.

## Step 5 — Driver-side discards (in-cluster solvers only)

If the quoter was classified in-cluster (URL ends in `.svc.cluster.local` and the driver-pod log-presence query in step 0 returned a non-zero count), a discarded solution leaves a trace in the shared driver pod. The only info-level discard is `discarded solution: settlement encoding`; `empty`, `duplicated id`, `scoring` are all `debug` and not retained:

```
container:<NETWORK>-driver-prod-liquidity AND network:<NETWORK>
  AND "discarded solution: settlement encoding"
  AND `parsed.spans./solve.solver`:<solver_name>
| fields _time, parsed.fields.orders, parsed.fields.err, `parsed.spans./solve.auction_id`
```

**LogsQL gotcha:** field paths containing `/` (like `parsed.spans./solve.solver`) must be wrapped in **backticks**, not double quotes. `"parsed.spans./solve.solver"` silently matches nothing.

To enumerate matching UIDs cheaply:

```
container:<NETWORK>-driver-prod-liquidity AND network:<NETWORK>
  AND "discarded solution: settlement encoding"
  AND `parsed.spans./solve.solver`:<solver_name>
| stats by (parsed.fields.orders) count() as n
```

(or `victorialogs_field_values` with `field=parsed.fields.orders`.) Intersect that set with the quoter's quoted UIDs.

The `err` field is verbose. Common patterns to bucket:

| Pattern in `err` | Bucket |
|---|---|
| `insufficient funds for gas * price + value: address 0x… have X want Y` | `solver_submission_account_out_of_gas` (point at the `0x…` — that's the solver's submission address) |
| `Ethereum(AccessList("execution reverted"))` | `simulation_revert` (settlement reverted in simulation) |
| `OutOfGas` / `gas required exceeds allowance` | `simulation_oog` |
| `Permit2` / `signature` substrings | `signature_or_permit_failed` |
| anything else | record verbatim |

A bid that reaches autopilot AND is later discarded is possible (different solutions for the same order across different auctions). Track them as a set union — `bid_layer = both` if both signals exist for the same `(solver, uid)`.

In order to diagnose the revert reason, you can use https://debug.cow.fi/api/orders/<order_uid>/debug/simulations?auctionId=<auction id> to fetch calldata and simulate either using the tenderly API or using foundy against an archive RPC node.

## Step 6 — Combine and emit CSV

```
order_id, expired, expired_detail, quoter, quoter_name, did_bid, bid_layer, discard_reason
```

Decision logic for `did_bid`:

```
if quoter in colocated:
    if uid in autopilot_proposed[quoter]:    yes / autopilot
    else:                                    unknown / external_driver_logs_unavailable
else:
    proposed   = uid in autopilot_proposed[quoter]
    discarded  = discarded_per_solver[quoter].get(uid)
    if proposed and discarded:               yes / both
    elif proposed:                           yes / autopilot
    elif discarded:                          yes / driver_discarded
    else:                                    no / ''
```

For `discard_reason` when only autopilot saw the bid (no driver-side discard), derive from the order's final event:

| Last event | Reason for the autopilot-side bid |
|---|---|
| `ready`      | `bid_lost_ranking` (other solver won, or no winner) |
| `invalid`    | `bid_proposed_but_order_became_invalid` |
| `filtered`   | `bid_proposed_but_order_filtered` |
| `executing` / `considered` | `bid_won_but_settlement_failed` (chase via `settlement failed err=…`) |

## Step 7 — Per-quoter summary

Print a table:

```
Quoter            Total  Expired  RemovedEarly  Unk  Bid_yes  Bid_no  Bid_unk
flowdesk-solve      517      421            96    0      163     354        0
tsolver             215      145            70    0       21       0      194    ← co-located, no driver visibility
NO_QUOTER            86        0             0   86        0       0       86
…
```

Plus per-quoter histograms of `expired_detail` and `discard_reason`. The expected shape of the answer is: dominant root cause(s), with order-level evidence.

---

## Common root causes (mainnet / bnb so far)

| Symptom | Root cause | Where you see it |
|---|---|---|
| Hundreds of `driver_discard:insufficient_funds_for_gas` from one in-cluster solver | Solver's submission address ran out of native token | `err` includes `"address 0x… have X want Y"`; the `0x…` is the solver's submission address (cross-check against the `Creating solver` log mapping from step 3) |
| Massive `expired with last_event=ready, no bid from anyone` | Token pair filtered by drivers' risk-detector | Search `"ignored orders with unsupported tokens"` near the order's lifetime — ~one entry per (driver, auction) means **every** driver rejected it |
| `last_event=invalid` cluster | Smart wallet (EIP-1271) users moving funds, or presign revoked | Confirm with `signingScheme` from API; `presignature_events` (DB) or `setPreSignature` on-chain trace for presign |
| Quoter never bids on its own quote | Quoter ≠ bidder by design (e.g. RFQ solvers refuse stale quotes); often paired with EIP-1271 + smart-slippage shrinkage | Check `quote.verified` and the autopilot competition for the auction — usually a different solver wins |
| All bids `bid_lost_ranking` for one solver | Another solver consistently outbids; not a bug | Pull the auction competition from `/api/v1/solver_competition/{auction_id}` to see scores |

## Caveats

- **Time decay.** Victoria Logs retention varies (currently ≥30 days for low-volume networks, less for mainnet). Check log presence with a single `victorialogs_stats_query_range` for the order's window before chunking.
- **External drivers stay opaque.** Co-located solvers do not log into Victoria Logs. If you need their "computed-but-discarded" picture, ask the partner directly (`#solver-{name}` channel) or look at Prometheus `dropped_solutions_total{solver="<name>"}` for an aggregate (no per-order linkage).
- **OR-chunk sizing.** Keep ≤30 UIDs per OR clause to stay well under the LogsQL parse limit and avoid copy-paste corruption when constructing queries by hand. Always read the query back from a file (`Read` tool) before pasting into the MCP call — UIDs in the middle of a long query are easy to mangle.
- **Backticks vs quotes.** `parsed.spans./solve.solver` ⇒ backticks. `"parsed.spans./solve.solver"` silently matches nothing; you'll wonder why the same field works in `field_values` but returns 0 in `query`.
- **`parsed.fields.orders` is a string.** It's the rendered Rust `[Uid(0x…)]` debug-format, not a list of strings. Extract UIDs with `re.findall(r'0x[0-9a-fA-F]{112}', s)`. A single solution can include multiple orders.
- **Quoter ≠ bidder.** "Did the quoter bid?" is a different question from "did anyone bid?". The user usually wants the former (quoter accountability) — but if everything else looks fine, it's worth answering the latter too.
- **Co-location can change between deploys.** Always recompute the in-cluster vs co-located buckets from the autopilot's `Creating solver` URLs (and confirm with a driver-pod log-presence check) for the network and time window you're analyzing — never carry over a hard-coded list from a previous run.
- **Solver promoted/demoted across deploys.** A solver may have been in-cluster a week ago and co-located today (or vice versa). The URL in `Creating solver` changes accordingly, so always pull it for a window that overlaps with the orders' time range, not for "now".

## Reference: useful pre-canned queries

```bash
# Autopilot solver-name ↔ address ↔ URL (one-time, per network).
# URL host suffix `.svc.cluster.local` => in-cluster; anything else => co-located.
container:=<NETWORK>-autopilot-prod AND _msg:="Creating solver"
| fields _time, parsed.fields.name, parsed.fields.submission_address, parsed.fields.url

# Co-location cross-check: any driver-pod log from this solver in the window?
# Zero hits ⇒ assume co-located, regardless of URL.
container:=<NETWORK>-driver-prod-liquidity
  AND `parsed.spans./solve.solver`:<solver>
| stats by (network) count() as total

# Per-day discards by an in-cluster solver
container:=<NETWORK>-driver-prod-liquidity
  AND "discarded solution: settlement encoding"
  AND `parsed.spans./solve.solver`:<solver>
| stats by (network) count() as total

# Did *any* solver bid for an order?
container:!controller AND network:<NETWORK>
  AND "proposed solution"
  AND all:<ORDER_UID>
| stats by (parsed.fields.driver) count() as n

# Was the order excluded by drivers' risk-detector?
container:!controller AND network:<NETWORK>
  AND "ignored orders with unsupported tokens"
  AND all:<ORDER_UID>
| stats by (container) count() as n
```

## Output convention

Save the merged CSV at `<repo>/services/<batch_name>_analysis.csv` (or the working directory the user specified). Always include:

1. The CSV file path.
2. The per-quoter summary table (plain-text, monospace).
3. A short root-cause paragraph naming the dominant bucket(s) and the evidence (e.g., "159 of 162 flowdesk driver-discards were `insufficient funds for gas` on `0xd0ee…5ea8`").
4. An explicit caveat sentence if any quoter is co-located ("for tsolver/kipseli we lack driver-log visibility — those `did_bid=unknown` rows could be either compute-and-discard or never-computed").
