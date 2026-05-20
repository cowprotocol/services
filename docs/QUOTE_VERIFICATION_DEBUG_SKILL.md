---
name: debug-quote
description: Debugs why the backend returned a given quote. It does that by inspecting the quote competition as a whole and re-simulating individual quotes when it makes sense.
---

## When to use

- User has a `quote_id`, `trace_id`, or `request_id` and asks why a quote failed / why a specific solver didn't verify.
- User wants per-estimator root-cause across all solvers that ran for one quote.
- User reports a class of failures ("lots of OOG on bitget today") and wants tracing.
- User wants to know **by how much** an `out_amount` claim missed reality (slippage / depth) or **by how much** balance/allowance was short.

## Requirements

| Need | For what |
|---|---|
| `CoW-Prod` MCP (VictoriaLogs) | All steps — pulling logs |
| `$ETH_MAINNET_RPC` env var | Replay options A (raw RPC) and C (cast) |
| `$TENDERLY_ACCESS_TOKEN` | Replay option B (Tenderly) — needed for the saved/shareable simulation link |
| `cast` (foundry, in `$PATH`) | Replay option C only |

The replay node must support `debug_traceCall` with `stateOverrides`.

## Inputs

- **`quote_id`** (decimal, e.g. `1158617496`) — primary key for verification failures; resolve to `trace_id` in step 1.
- **`trace_id`** (32-hex) — direct.
- **`request_id`** (32-hex) — same per-quote, redundant but useful for cross-checking.
- **token pair** (sell + buy, symbol or address) and optionally the **trader wallet** — typical "Insufficient Liquidity" report has no quote_id (the orderbook returns an error instead of a quote). Resolve to `trace_id` in step 1b.
- network — this skill assumes mainnet; adapt the container name (`<network>-api-prod`) for other chains.

## What you'll find out

For each failed estimator: the settle calldata the verifier asked the chain to execute (the `0x1d47e7f4` helper wraps `GPv2Settlement.settle()`), the full call tree, the deepest revert (selector + decoded data), and whether it's a real routing/liquidity bug, a verifier infra bug (gas cap, Permit2 override), or a solver-encoder bug (missing approve, empty interactions, wrong selector).

---

## Step 1 — `quote_id` → `trace_id`

If you only have a `quote_id`, look up the trace:

```
container:="mainnet-api-prod" AND all:"<QUOTE_ID>" AND _msg:="finished computing quote"
| fields _time, parsed.trace_id, parsed.fields.response
```

The `parsed.fields.response` field contains the `OrderQuoteResponse` (sell/buy tokens, amounts, the winning estimator's `verified:` flag, etc.). Save the `trace_id` for steps 2–4.

## Step 1b — token pair (+ optionally wallet) → `trace_id`

When the user reports "Insufficient Liquidity for X→Y" with no `quote_id` (no quote was produced, so no id was minted), find the trace from the token pair instead.

### Resolve symbols to addresses

If the user gave a *symbol* (e.g. `tGLD`):

1. **First stop: cowprotocol/token-lists** — https://github.com/cowprotocol/token-lists is the canonical CoW Swap UI token list. Each chain has its own JSON; grep by `"symbol"`. Symbols are not unique (e.g. `tGLD` resolves to both Tenbin Gold *and* TempleGold on mainnet), so confirm against the address the user actually used if known.
2. Verify any candidate against the chain by probing `name()`/`symbol()`/`decimals()`:
   ```bash
   ETH_MAINNET_RPC=...
   ADDR=0x...
   for sel in 0x06fdde03 0x95d89b41 0x313ce567; do
     curl -s -X POST -H 'Content-Type: application/json' \
       --data '{"jsonrpc":"2.0","method":"eth_call","params":[{"to":"'$ADDR'","data":"'$sel'"},"latest"],"id":1}' $ETH_MAINNET_RPC
   done
   ```
   ABI-decode each result (string for `name`/`symbol`, uint8 for `decimals`).
3. If the symbol isn't in the token list and you can't get it from the user, **ask** rather than guessing — many tokens share popular tickers, and a wrong address means an empty trace search and wasted effort.

### Find the trace

The `calculate_quote` span carries `sell_token` and `buy_token` directly on every `new price estimate` row. Filter on both, then group by `trace_id`:

```
container:="mainnet-api-prod"
  AND _msg:="new price estimate"
  AND parsed.spans.calculate_quote.sell_token:"<SELL_ADDR_LOWERCASE>"
  AND parsed.spans.calculate_quote.buy_token:"<BUY_ADDR_LOWERCASE>"
| stats by (parsed.trace_id) count() as count
```

For a fully no-liquidity case, every estimator returned `Err(...)` and the request_summary status is **404** (the orderbook returns 404 for `OrderQuoteError::Estimator(NoLiquidity)` — *not* 400; 400 means validation/bad-input, which is a different failure mode). Cross-reference to confirm:

```
container:="mainnet-api-prod"
  AND _msg:="request_summary"
  AND parsed.fields.uri:"/api/v1/quote"
  AND parsed.fields.status:"404"
  AND parsed.trace_id:"<CANDIDATE_TRACE_ID>"
```

A given UI usually fires both `priceQuality: optimal` *and* `priceQuality: fast` in parallel for the same pair, so expect two trace_ids per user attempt with the same sell/buy. The ones with optimal quality are preferred for the per-estimator analysis; pick the most recent.

### Narrow by wallet (optional)

If the user gave their wallet address, filter further on the `from` field. `from` is *not* on the `parsed.spans.calculate_quote` span — it lives inside the `Query { ... }` block stored in `parsed.fields.query` on each `new price estimate` row, formatted like `from: 0xabcd…`. Substring-match it:

```
container:="mainnet-api-prod"
  AND _msg:="new price estimate"
  AND parsed.spans.calculate_quote.sell_token:"<SELL_ADDR>"
  AND parsed.spans.calculate_quote.buy_token:"<BUY_ADDR>"
  AND parsed.fields.query:"from: 0xWALLET_LOWERCASE"
| stats by (parsed.trace_id) count() as count
```

When `from` is unknown, the orderbook substitutes the verifier-default placeholder `0xd711bd26bf5b153001a7c0accb289782b6f775e9` (or `0x0000…0000` for some `priceQuality: fast` paths) — matching that filters out user-specific traces but keeps default ones. Useful when distinguishing "user without wallet connected" from "user with a specific wallet".

### Sanity check before continuing

Pull the per-estimator results for the candidate trace (step 2's first query). The shape tells you which sub-flow to follow:

- All estimators `Err(...)` → no-liquidity case; the diagnosis is "which native estimators were asked, what did they return, was the relevant solver wired in?" (see step 5 + the autopilot native-price audit pattern)
- One or more `Ok(... verified: true)` returned but the orderbook still 404'd → ranking-context failure (e.g. autopilot native price returned `Err(NoLiquidity)` for the buy token, short-circuiting `BestBangForBuck`). Search `mainnet-autopilot-prod` logs for the buy_token to see what each native estimator returned.
- `Ok(... verified: false)` with `quote verification failed` log lines → the regular per-estimator verification flow; jump to step 3.

## Step 2 — Pull the quote envelope (per-estimator bids + which failed)

```
container:="mainnet-api-prod"
  AND parsed.trace_id:="<TRACE_ID>"
  AND _msg:="new price estimate"
| fields _time, parsed.fields.estimator, parsed.fields.result
```

Each row: `estimator`, then `Ok(Estimate { out_amount, gas, solver, verified, .. })` or `Err(NoLiquidity | EstimatorInternal(...))`. Filter by `verified: false` for the candidates worth replaying.

To list verification failures with their err selectors:

```
container:="mainnet-api-prod"
  AND parsed.trace_id:="<TRACE_ID>"
  AND _msg:="quote verification failed"
| extract_regexp `data: "(?P<sel>0x[0-9a-f]{8})` from parsed.fields.err
| stats by (parsed.spans.estimator.name, sel) count() as count
```

A bare `execution reverted` (no `data:` field in the err) is its own failure mode — see the playbook in step 5.

## Step 3 — Fetch each failed estimator's resimulate-curl

```
container:="mainnet-api-prod"
  AND parsed.trace_id:="<TRACE_ID>"
  AND _msg:~"^resimulate by setting"
  AND parsed.spans.estimator.name:="<EST>"
```

The `_msg` is a `curl` command with a `--data '<JSON>'` payload (20–90 KB: calldata + state overrides). When the MCP response exceeds ~25k tokens it auto-saves under `.claude/projects/.../tool-results/`; note the path.

Extract the JSON payload to `/tmp/cow-trace/<est>.json`:

```bash
SRC=<auto-saved-path>
mkdir -p /tmp/cow-trace
for est in $(jq -r '.[0].text' "$SRC" | jq -sr '.[] | .["parsed.spans.estimator.name"]' | sort -u); do
  jq -r '.[0].text' "$SRC" \
  | jq -sr --arg e "$est" '.[] | select(.["parsed.spans.estimator.name"]==$e) | ._msg' \
  | grep -oP "(?<=--data ').*?(?=' https://api\.tenderly)" \
  > /tmp/cow-trace/$est.json
done
```

Each `<est>.json` is the Tenderly /simulate body — also the input to all three replay options below.

## Step 4 — Replay (pick one per use case)

### Option A — reth `debug_traceCall` + `run_sim.py`  (programmatic; preferred for root-causing)

Drop this at `/tmp/cow-trace/run_sim.py`:

```python
#!/usr/bin/env python3
"""Replay a Tenderly /simulate payload via debug_traceCall.
Usage: run_sim.py <payload.json> [gas-decimal-override]
Env: ETH_MAINNET_RPC (default: cow public proxy); ALL_CALLS=1 to show non-failing branches.
"""
import json, os, sys, urllib.request

KNOWN = {
    "0x9008d19f58aabd9ed0d60971565aa8510560ab41": "GPv2Settlement",
    "0xc92e8bdf79f0507f65a392b0ab4667716bfe0110": "GPv2VaultRelayer",
    "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2": "WETH",
    "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48": "USDC",
    "0xdac17f958d2ee523a2206206994597c13d831ec7": "USDT",
    "0x000000000022d473030f116ddee9f6b43ac78ba3": "Permit2",
    "0xd524f98f554bd34f4185678f64a85bb98971d314": "0x AllowanceHolder",
    "0x699c5bd4d03d98dabe8ef94ce13ba0314e4d35c8": "0x Settler",
    "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee": "Native ETH",
    "0xe592427a0aece92de3edee1f18e0157c05861564": "UniV3 SwapRouter",
    "0x66a9893cc07d91d95644aedd05d03f95e1dba8af": "Universal Router (V4)",
    "0x000000000004444c5dc75cb358380d2e3de08a90": "UniV4 PoolManager",
    "0x0000000000000000000000000000000000020000": "verifier-helper-precompile",
}
def label(a): return KNOWN.get((a or "").lower(), (a or "?")[:10] + "…" + (a or "")[-4:])
def sel(s): return (s or "")[:10]

def post(p, url, gas_override=None):
    overrides = {}
    for addr, obj in (p.get("state_objects") or {}).items():
        ov = {}
        for k_in, k_out in (("balance","balance"), ("code","code"),
                            ("nonce","nonce"), ("storage","stateDiff")):
            if k_in in obj: ov[k_out] = obj[k_in]
        overrides[addr] = ov
    call = {"from": p["from"], "to": p["to"], "data": p["input"]}
    if gas_override is not None: call["gas"] = hex(gas_override)
    elif p.get("gas"): call["gas"] = hex(p["gas"])
    if p.get("gas_price"): call["gasPrice"] = hex(p["gas_price"])
    block = hex(p["block_number"]) if p.get("block_number") else "latest"
    rpc = {"jsonrpc":"2.0","method":"debug_traceCall","params":[call, block,
        {"tracer":"callTracer","tracerConfig":{"withLog":True},"stateOverrides":overrides}],
        "id":1}
    req = urllib.request.Request(url, method="POST",
        data=json.dumps(rpc).encode(), headers={"Content-Type":"application/json"})
    return json.loads(urllib.request.urlopen(req, timeout=180).read())

def has_err(c): return bool(c.get("error") or c.get("revertReason")) or any(has_err(x) for x in c.get("calls") or [])

def walk(c, depth=0, only_failed=True, out=None):
    if out is None: out = []
    err, rev = c.get("error"), c.get("revertReason")
    suffix = f"  ❌ {err or ''}{' / '+rev if rev else ''}" if (err or rev) else ""
    out.append(f"{'  '*depth}{c.get('type','CALL')} {label(c.get('from'))} -> "
               f"{label(c.get('to'))} sel={sel(c.get('input',''))} gas={c.get('gasUsed','?')}{suffix}")
    o = c.get("output") or ""
    if err and o and o != "0x": out.append(f"{'  '*(depth+1)}↳ revert_data={o[:266]}")
    for child in c.get("calls") or []:
        if only_failed and not has_err(child) and not err: continue
        walk(child, depth+1, only_failed, out)
    return out

def deepest(c, path=None):
    if path is None: path = []
    me = path + [c]; d = None
    for child in c.get("calls") or []:
        r = deepest(child, me)
        if r and (d is None or len(r[0]) > len(d[0])): d = r
    if c.get("error") and (d is None or len(me) > len(d[0])): d = (me, c)
    return d

if __name__ == "__main__":
    p = json.load(open(sys.argv[1]))
    g = int(sys.argv[2]) if len(sys.argv) > 2 else None
    url = os.environ.get("ETH_MAINNET_RPC")
    print(f"# block={p.get('block_number','latest')} gas={'override='+str(g) if g else hex(p.get('gas',0))}")
    res = post(p, url, g)
    if "error" in res and "result" not in res:
        print("# rpc error:", res["error"]); sys.exit(1)
    for line in walk(res["result"], only_failed=os.environ.get("ALL_CALLS")!="1"): print(line)
    d = deepest(res["result"])
    if d:
        path, leaf = d
        print(f"\n# DEEPEST REVERT (depth={len(path)})")
        print(f"#   to: {leaf.get('to')}  sel: {sel(leaf.get('input',''))}")
        print(f"#   error: {leaf.get('error')}")
        if leaf.get('output') and leaf['output'] != '0x':
            print(f"#   revert_data: {leaf['output'][:266]}")
        for i, c in enumerate(path):
            print(f"#   {i}. {c.get('type')} {label(c.get('from'))} -> "
                  f"{label(c.get('to'))} sel={sel(c.get('input',''))}")
    else:
        print("# ✅ no revert at any depth")
```

```bash
python3 /tmp/cow-trace/run_sim.py /tmp/cow-trace/tsolver.json
ALL_CALLS=1 python3 /tmp/cow-trace/run_sim.py /tmp/cow-trace/tsolver.json   # full tree
python3 /tmp/cow-trace/run_sim.py /tmp/cow-trace/tsolver.json 100000000     # bumped gas
```

### Option B — Tenderly `/simulate` (saved, shareable UI; *finish here for human handoff*)

```bash
# drop transaction_index — Tenderly rejects -1 with no block_number ("Transaction index is not allowed when block number is pending")
jq 'del(.transaction_index)' /tmp/cow-trace/tsolver.json > /tmp/cow-trace/tsolver_tenderly.json
source services/.env.claude
curl -sS -X POST \
  -H "X-ACCESS-KEY: $TENDERLY_ACCESS_TOKEN" -H "Content-Type: application/json" \
  --data @/tmp/cow-trace/tsolver_tenderly.json \
  https://api.tenderly.co/api/v1/account/cow-protocol/project/production/simulate \
| jq -r '"https://dashboard.tenderly.co/cow-protocol/production/simulator/" + .simulation.id'
```

Tenderly upstream regularly times out on `$50M`-scale routes (`upstream request timeout`) — fall back to A or C. **Always paste a Tenderly link into the user-facing summary if possible** - it's how the user verifies the root cause.

### Option C — `cast call --trace` (foundry one-liner; quick eyeball)

```bash
python3 - /tmp/cow-trace/tsolver.json <<'PY' | bash
import json, shlex, sys
p = json.load(open(sys.argv[1]))
cmd = ["cast","call","--trace","--rpc-url","$ETH_MAINNET_RPC",
       "--from", p["from"], "--gas-limit", str(p["gas"])]
if p.get("gas_price"): cmd += ["--gas-price", str(p["gas_price"])]
for addr, ov in (p.get("state_objects") or {}).items():
    if "balance" in ov: cmd += ["--override-balance", f"{addr}:{ov['balance']}"]
    if "code"    in ov: cmd += ["--override-code",    f"{addr}:{ov['code']}"]
    if "nonce"   in ov: cmd += ["--override-nonce",   f"{addr}:{ov['nonce']}"]
    for slot, val in (ov.get("storage") or {}).items():
        cmd += ["--override-state-diff", f"{addr}:{slot}:{val}"]
cmd += [p["to"], p["input"]]
print("source services/.env.claude && \\\n" + " \\\n  ".join(shlex.quote(x) for x in cmd))
PY
```

Output is a human-readable call tree with selector decoding (uses 4byte). Bumped-gas re-runs need only `--gas-limit <new>`.

**Tool selection**: A for programmatic walking / decoding the settle calldata / gas binary search / cross-trace pattern checks; C for a fast one-shot decode; B at the end for the user-facing link.

---

## Step 5 — Diagnose the revert

### Step 5a — Reason from the trace (do this first; the table in 5b is a catalog of common patterns, NOT a substitute for thinking)

The goal is not to match the deepest selector against a lookup table — the goal is to explain, frame by frame, **what each call was supposed to do, what it actually did, and where the divergence happened**. Most real failures don't fit one row of the catalog cleanly; they're a story across several frames. Walk the trace and *reason*. Concretely:

1. **Anchor on the deepest reverting frame.** From `run_sim.py`'s output: which contract, which selector, what gas was used, what was the call depth, and what is the *parent frame* expecting back? A swap router reverting at depth 7 inside a Universal Router execute means something different than a top-level `WETH.transfer` reverting at depth 3.

2. **Decode every unknown selector.** Function selectors:
   ```bash
   curl -s 'https://api.openchain.xyz/signature-database/v1/lookup?function=0xAAAA,0xBBBB&filter=true' | jq .
   ```
   Error selectors live in the same database — the first 4 bytes of revert data is just another selector. Cross-check the cheat sheet first; if not there, *always* look it up rather than guessing. Also note: an unverified contract may still expose selectors you can dispatch (`eth_call` with `name()`/`symbol()`/`token0()`/`fee()` etc.) — use that to identify the target.

3. **ABI-decode revert data, not just the selector.** The revert payload after the selector usually carries the diagnostic. `cast abi-decode 'TooMuchSlippage(address,uint256,uint256)' <hex>` tells you which token, the minimum expected, and the actual delivered — which lets you state "missed by N basis points" instead of just "slippage." Same for `ERC20InsufficientBalance(address,uint256,uint256)`, `Error(string)` (`0x08c379a0`), `Panic(uint256)` (`0x4e487b71`), custom DEX errors. If `output` is `0x` (empty), that is itself diagnostic — see step 5e.

4. **Read the parent chain.** From the failing leaf, walk up frame by frame. Annotate each: who called whom, with what selector, with what gas budget, what return path was expected. The story usually emerges between two adjacent frames where one expected a balance/return value the other didn't provide.

5. **Identify the buy-token / sell-token flow.** Re-run with `ALL_CALLS=1` and grep every `transfer`/`transferFrom`/`approve` of the involved tokens. Did Settlement actually accumulate the buy amount before the pay-out? Did sell tokens leave Settlement to a router, and did anything come back? A net-zero buy-token flow before the final pay-out almost always means "the encoded swap didn't run" — but the *why* (empty interactions, wrong selector, OOG mid-route, pool rejected) needs upstream evidence.

6. **Sanity-check gas.** Real swaps cost real gas. A `transferFromAccounts` taking 13k gas is not doing a $10M swap; a Universal Router `execute` taking 50k gas is suspect. Compare the solver's self-reported `gas` (from `new price estimate`) to the trace's `gasUsed`, and to the gas the *swap router* used. Cross-reference against the verifier's 16.78M outer cap (step 5d).

7. **Decode the solver's encoded solution.** The verifier entry `0x1d47e7f4 swap(settlement, tokens[], receiver, settlementCall)` wraps `0x13d79a0b GPv2Settlement.settle(tokens[], clearingPrices[], trades[], interactions[3])`. `interactions[3]` is `(pre, intra, post)` — `pre`/`post` come from the verifier; **only `intra` is the solver's**. Decoding it answers questions like "did the solver even include a swap?", "what router was called?", "is there an `approve` before the transferFrom?". Step 5c walks this.

8. **Account for the verifier's overrides.** The trader address gets a code override (helper contract that signs/approves), the trader's sell-token balance is seeded via a storage-slot override, and `0x…020000` is a precompile that wraps `safeTransferFrom`. A balance/allowance check that fails near the trader frequently traces to a slot mismatch in the storage override — verify the slot the token *actually reads* matches the override (mapping slot for `balanceOf` is `keccak256(addr . slot_index)` for the relevant slot index per token).

9. **Form a hypothesis, then *test* it.** Useful experiments cheap enough to run:
   - Re-run with `gas=100000000` — if it now passes, the failure was the verifier's gas cap, not a real revert.
   - Decode `interactions[]` — if `interactions[1].length == 0`, you've found an empty-solution bug.
   - Re-run with the storage override removed/extended — confirms whether a balance/allowance gap is real on chain or a verifier-setup artifact.
   - Search VictoriaLogs for the same solver across recent quotes (`stats by selector count`) to see if the failure is a one-off or systemic.

10. **Surface context, not labels, in the user-facing summary.** The user wants the *story* of the revert: which contract did what at each frame, what was missing, by how much, and which actor (solver / verifier / liquidity / pool policy / user wallet) is responsible. A one-liner like "TooMuchSlippage" is a label, not a root cause — the root cause is "0x Settler routed through the low-TVL UniV3 USDC/WETH 0.01% pool, which only quoted X but min-out demanded Y; missed by N bps." Always include the ABI-decoded revert args and the relevant parent-frame context in the report.

### Step 5b — Common patterns (catalog of recurring failure modes; NOT exhaustive — always reason from the trace first)

| Symptom on the deepest call | Diagnosis | Fix locus |
|---|---|---|
| `0x97a6f3b9 TooMuchSlippage(addr,uint,uint)` | Real liquidity miss. ABI-decode args: token, expected_min, actual; report the bps miss. | Solver routing |
| `0x39d35496 V3TooLittleReceived` / `0x8b063d73 V4TooLittleReceived` / `0xbb2875c3 InsufficientOutput` | min-out check failed | Solver routing |
| `out of gas: not enough gas for reentrancy sentry` (or pure `execution reverted` at depth 4+ with high gasUsed) | Verifier outer gas cap (`0xFFFFFF` = 16.78M) bites | Re-run with bumped gas (step 5d) |
| Bare `execution reverted` at the final `<buy_token>.transfer(trader, …)` (depth 3), no inner swap path | Likely empty-intra bug — solver returned `out_amount` but no swap calldata. Confirm by decoding `interactions[3]` (step 5c). | Solver settle-encoder |
| Bare `execution reverted` on a `transferFrom` mid-tree | Token's silent-revert path — usually missing `approve` or insufficient balance. Reconstruct ledger from prior calls (step 5e). | Solver settle-encoder |
| `0x08c379a0 Error("trader does not have enough sell token")` | Verifier's pre-check — quote requested by a wallet that doesn't hold the sell token | None — filter out before triaging |
| `0xd81b2f2e AllowanceExpired(uint256)` (Permit2) | Verifier's Permit2 storage override has stale/0 expiry | Verifier setup (services) |
| `0x5cd5d233 BadSignature()` (0x Settler / Permit2) | Permit2 signature crafted with wrong nonce or invalidated by chain state | Solver |
| `0x8a3b7ff1 MaxImbalanceRatioExceeded` / `0x27e92f0f maxTradeSizeRatioExceeded` | Pool refuses oversized trade | None — pool policy |

If the trace doesn't match a row above, **don't force-fit it** — fall back to the reasoning steps in 5a, decode unknown selectors via openchain, and write the actual story.

### Step 5c — Decode the solver's interactions

Empty-interactions bugs (recently common in tsolver) are best confirmed by decoding `interactions[3]` in the settle calldata directly. The verifier wraps `swap(settlement, tokens[], receiver, settlementCall)` (selector `0x1d47e7f4`); inside `settlementCall` you'll find `GPv2Settlement.settle(tokens[], clearingPrices[], trades[], interactions[3])` (selector `0x13d79a0b`). The interactions are split into `(pre, intra, post)` — `pre` and `post` come from the verifier (preSwap hook + trackBalance), so `intra.length == 0` means the solver provided no swap. Read the JSON payload's `input` and walk offsets in Python.

### Step 5d — OOG: bump gas, then binary search

If the simualtion fails due to "out of gas", try to resimulate it with the maximum gas limit for the chain and compare the solver's self-reported `gas` (from the `new price estimate` log) to actual `gasUsed`. Keep in mind potential maximum block gas limits as well as maximum transaction gas limits for the given chain.

### Step 5e — Balance/allowance forensics for silent reverts

When a `transferFrom` reverts with no data, walk every prior call to that token, replay `transfer/transferFrom/approve` into a virtual ledger keyed on `(addr -> bal)` and `(owner, spender) -> allowance`. Apply only on calls that succeeded. At the failing call, compare the requested `dx` to balance(src) and allowance(src→spender) to see which (or both) is short.

Worked example: kipseli's $50M USDC→ETH route routed through V2 to deliver 2,499,469 crvUSD into Settlement, then a Curve `exchange()` reverted because the encoder forgot a `crvUSD.approve(curvePool, …)` interaction — balance was exact, allowance was 0.

---

## Step 6 — Report findings

Default output for a per-quote multi-solver verification debug is **one sorted table covering every estimator that ran** — including the no-quote ones. Don't split into separate tables.

Columns:

| Column | Content |
|---|---|
| `#` | rank by `out_amount` descending; `—` for no-quote rows (placed at the bottom, alphabetic) |
| Estimator | name; mark the winner with ⭐ |
| `out_amount (<buy_token>)` | decimalized to the buy token's units (WETH 18 dp, USDC 6 dp, etc.); `—` for no-quote rows |
| `gas` | solver's self-reported (from the `new price estimate` log) |
| `verified` | ✅ / ❌ / `—` |
| Notes | for **non-verified** rows: a Tenderly simulation link (option B in step 4) plus a one-sentence root cause from the trace replay; for **no-quote** rows: the `Err(...)` reason verbatim from the `new price estimate` log (`NoLiquidity`, `EstimatorInternal(...)`); for verified rows: usually empty |

Lead the response with this table. Place any cross-solver pattern analysis or longer narrative *after* it — the table is the primary artifact the user scans.

Always POST the failed-estimator payload to Tenderly to produce the link, even when reth already gave you the root cause — the user needs the UI to verify against. Drop `transaction_index` from the payload before submitting (Tenderly rejects `-1` with no explicit `block_number`).

---

## Cheat sheet — selectors

| Selector | Meaning |
|---|---|
| `0x1d47e7f4` | verifier entry: `swap(settlement, tokens[], receiver, settlementCall)` |
| `0x2582edb4` | trader-side hook (preSwap helper) |
| `0x3bbb2e1d` | verifier `trackBalance(token, account)` |
| `0x494666b6` | helper `safeTransferFrom(token, amt)` (precompile at `0x…020000`) |
| `0x13d79a0b` | `GPv2Settlement.settle(...)` |
| `0x542eb77d` | trader-helper `preSwap(token,tokenSold,sellAmt,solver,solverAddr)` |
| `0xeb5625d9` | trader-helper approve |
| `0x70a08231` `0xa9059cbb` `0x23b872dd` `0x095ea7b3` `0xdd62ed3e` | ERC20 `balanceOf`/`transfer`/`transferFrom`/`approve`/`allowance` |
| `0x97a6f3b9` | `TooMuchSlippage(address,uint256,uint256)` (0x Settler) |
| `0x5cd5d233` | `BadSignature()` (0x / Permit2) |
| `0xd81b2f2e` | `AllowanceExpired(uint256)` (Permit2; uint = expiry) |
| `0x39d35496` `0x8b063d73` `0xbb2875c3` | V3/V4 too-little-received / `InsufficientOutput` |
| `0x064a4ec6` | `ReturnAmountIsNotEnough(uint256,uint256)` (1inch) |
| `0xe450d38c` | `ERC20InsufficientBalance(address,uint256,uint256)` (OZ v5) |
| `0x8a3b7ff1` `0x27e92f0f` | `MaxImbalanceRatioExceeded` / `maxTradeSizeRatioExceeded` (Balancer / pool) |
| `0xdee51a8a` `0xdcab82e2` `0x2fee3e0e` | Fluid `SafeTransferError` / `LiquidityError` / `DexError` |
| `0xf7bf5832` | `TychoRouter__AmountOutNotFullyReceived(uint256,uint256)` |
| `0x486aa307` | `PoolNotInitialized()` (UniV4) |
| `0x08c379a0` | `Error(string)` (string offset 0x60, length at 0x40) |
| `0x4e487b71` | `Panic(uint256)` |

Decode unknown ones via:

```bash
curl -s 'https://api.openchain.xyz/signature-database/v1/lookup?function=0xAAAA,0xBBBB&filter=true' | jq .
```

## Cheat sheet — addresses

| Address | What |
|---|---|
| `0x9008d19f58aabd9ed0d60971565aa8510560ab41` | GPv2Settlement (mainnet) |
| `0xc92e8bdf79f0507f65a392b0ab4667716bfe0110` | GPv2VaultRelayer (pulls sell tokens from trader → Settlement; selector `0x7d10d11f transferFromAccounts(...)`) |
| `0x000000000022d473030f116ddee9f6b43ac78ba3` | Permit2 |
| `0xd524f98f554bd34f4185678f64a85bb98971d314` | 0x AllowanceHolder |
| `0x699c5bd4d03d98dabe8ef94ce13ba0314e4d35c8` | 0x Settler entry |
| `0xe592427a0aece92de3edee1f18e0157c05861564` | UniV3 SwapRouter |
| `0x66a9893cc07d91d95644aedd05d03f95e1dba8af` | Universal Router V4 (selector `0x24856bc3 execute(bytes,bytes[])`) |
| `0x000000000004444c5dc75cb358380d2e3de08a90` | UniV4 PoolManager |
| `0xc02aaa39…756cc2` `0xa0b86991…06eb48` `0xdac17f95…831ec7` `0xf939e0a0…ac1b4e` | WETH, USDC, USDT, crvUSD |
| `0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee` | Native ETH placeholder |
| `0x0000000000000000000000000000000000020000` | verifier-helper precompile (state-overridden each sim; empty on chain) |

Identify unknowns via reth / cast: ERC20 (`0x06fdde03 name`, `0x95d89b41 symbol`), UniV3 (`0x0dfe1681 token0`, `0xd21220a7 token1`, `0xddca3f43 fee`), Curve (`0xc6610657 coins(i)`).

---

## Source-of-truth pointers

- Verifier setup + state overrides: `crates/price-estimation/src/trade_verifier/mod.rs` (`prepare_state_overrides`)
- Verifier helper Solidity: `contracts/solidity/Solver.sol` (the `swap` / `storeBalance` / `ensureTradePreconditions` functions)
- Resimulate-curl emitter: `crates/simulator/src/tenderly/mod.rs` (`log_simulation_request`)

## Gotchas

- **Tenderly upstream timeouts** for big trades — fall back to reth (option A) or cast (option C). Payload format is identical; only the wrapper and overrides-key naming differ (`storage` ↔ `stateDiff`).
- **Auto-saved MCP query results.** When a victorialogs response exceeds ~25k tokens, it's written under `.claude/projects/.../tool-results/`. Note the path; don't refetch.
- **`extract` is greedy.** If the captured field's delimiter (e.g. `,`) appears inside the value, the capture overflows. Anchor on a stable next-literal or use `extract_regexp`.
- **State-override of `0x000…020000`** is the verifier-helper precompile that wraps `safeTransferFrom`. It only exists inside the simulation — the address is empty on chain.
- **0x Settler revert bubbling** — `TooMuchSlippage` propagates through 4+ frames (settler → AllowanceHolder → settle → helper). The deepest frame is rarely where the revert was *raised*; look for the first frame with revert data, usually a CALL to a UniV3 pool (selector `0x128acb08`) followed by a balance check.
- **Universal Router V4** (`0x24856bc3 execute(bytes,bytes[])`) unpacks command bytes into many sub-calls; traces get deep.
