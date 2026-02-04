# CoW Protocol Order Debug Skill

Debug why CoW Protocol orders fail to match. Requires DB access + Victoria Logs access (via Grafana).

## Quick Checklist

Run through these in order:

1. [ ] **Order status** — Check API status first (cancelled/expired/fulfilled/open)
2. [ ] **User cancellation** — If cancelled, search logs for `order cancelled ORDER_UID` FIRST
3. [ ] **Order in auction** — Was order in autopilot auction? When?
4. [ ] **Solver bids** — Did any solver bid? What happened to their solution?
5. [ ] **Settlement outcome** — Did settlement succeed/fail/timeout?
6. [ ] **Limit price sanity** — Was quote reasonable? Check slippage, fees, gas
7. [ ] **Price movement** — Did price move between quote and expiry?

---

## 1. Fetch Order Data

```bash
# Replace $NETWORK (mainnet/gnosis/arbitrum) and $ORDER_UID
curl -s "https://api.cow.fi/$NETWORK/api/v1/orders/$ORDER_UID" | jq .

# For staging orders:
curl -s "https://barn.api.cow.fi/$NETWORK/api/v1/orders/$ORDER_UID" | jq .
```

### GPV2Order Struct (Smart Contract Source of Truth)

| Field | Meaning | Debug Notes |
|-------|---------|-------------|
| `sellToken` | Token being sold | |
| `buyToken` | Token being bought | |
| `sellAmount` | Amount to sell (wei) | For sell orders, this is exact |
| `buyAmount` | Min amount to receive | For buy orders, this is exact |
| `validTo` | Unix timestamp expiry | Check if expired |
| `appData` | Hash of metadata JSON | Contains hooks, partner fees, flash loan hints |
| `feeAmount` | **Legacy, always 0** | Fee now in limit price |
| `kind` | "sell" or "buy" | |
| `partiallyFillable` | bool | Swaps = false (fill-or-kill), limits can be true |
| `sellTokenBalance` | **Legacy, always "erc20"** | Balancer vault balances never took off |
| `buyTokenBalance` | **Legacy, always "erc20"** | |
| `signingScheme` | eip712/ethsign/presign/eip1271 | See signing section |
| `signature` | The actual signature bytes | |
| `receiver` | Who gets buy tokens | null = order owner |

**Additional API fields:**
- `class`: "market" vs "limit" — see note below
- `status`: fulfilled/open/cancelled/expired
- `surplusFee`: Protocol's fee estimate for limit orders
- `surplusFeeTimestamp`: Must be <10 min old or order won't enter auction

**Note on order class:** In the DB, almost every order is stored as `class = 'limit'`. The "market" vs "limit" distinction is about **fee policy**, not order type:
- **Market order**: Had a quote attached, and the order's limit price is within that quote (in-market). Gets market fee policy.
- **Limit order**: Either no quote, or limit price is outside the quote (out-of-market). Gets limit fee policy with surplus fee.

The `appData.metadata.orderClass` field shows what the UI intended, but the actual classification is determined by comparing the order's price to the quote at placement time.

---

## 2. Signing Schemes

Orders can fail if signature validation fails. Different schemes have different failure modes:

| Scheme | Type | Validation | Common Failures |
|--------|------|------------|-----------------|
| `eip712` | EOA | Static, checked once | Sig doesn't match order fields, or signed by unexpected user |
| `ethsign` | EOA (legacy) | Static, checked once | Same as above |

**Note on unexpected signers:** The majority of signature issues are valid signatures but signed by an unexpected user. This causes the settlement contract to attempt transferring tokens from an account that doesn't have the necessary balance.
| `presign` | Smart contract | On-chain state (`setPreSignature`) | User called `setPreSignature(uid, false)` to cancel |
| `eip1271` | Smart contract | Calls `isValidSignature()` at settlement time | Contract state changed, Safe signer removed, custom logic rejects |

**EIP-1271 is dynamic** — signature can be valid at order placement but invalid later. Autopilot re-checks these every auction.

```bash
# Check if presign is set (returns signed boolean)
cast call $SETTLEMENT_CONTRACT "preSignature(bytes)" $ORDER_UID --rpc-url $RPC
```

---

## 3. Check Logs (Victoria Logs via Grafana)

Logs are stored in Victoria Logs and accessible via Grafana API.

**Query template (bash):**
```bash
# Set time range (milliseconds)
NOW_MS=$(($(date +%s) * 1000))
FROM_MS=$((NOW_MS - 43200000))  # 12 hours ago

# Set search term (use full order UID with 0x prefix)
SEARCH_TERM="0xd997dc715a7610c75e5f97548685befacb7ea5ad878cb4bac1816903514ed84d1dffc418c0d83bd8b98ab3d2e07b83bf5439f4236981a392"

# Query Victoria Logs
# - NOT container:controller excludes nginx access logs
# - network:$NETWORK filters to specific chain (mainnet, bnb, arbitrum-one, base, etc)
source .env.claude
curl -s "${GRAFANA_URL}/api/ds/query?ds_type=victoriametrics-logs-datasource" \
  -H 'accept: application/json' \
  -H 'content-type: application/json' \
  -H "x-datasource-uid: ${VICTORIA_LOGS_DATASOURCE_UID}" \
  -H 'x-plugin-id: victoriametrics-logs-datasource' \
  -H "Authorization: Bearer ${GRAFANA_API_TOKEN}" \
  --data-raw "{\"queries\":[{\"refId\":\"A\",\"datasource\":{\"type\":\"victoriametrics-logs-datasource\",\"uid\":\"${VICTORIA_LOGS_DATASOURCE_UID}\"},\"editorMode\":\"code\",\"expr\":\"NOT container:controller ${SEARCH_TERM} | sort by (_time) asc\",\"queryType\":\"instant\",\"maxLines\":100}],\"from\":\"${FROM_MS}\",\"to\":\"${NOW_MS}\"}" \
  | jq -r '.results.A.frames[0].data.values[1][]'
```

**Key query parameters:**
- `expr`: Search term (just text, e.g., order UID or auction ID). Add `| sort by (_time) asc` for chronological order
- `queryType`: Use `"instant"` to get log lines. Use `"hits"` to get histogram/counts
- `maxLines`: Number of log lines to return
- `from`/`to`: Millisecond timestamps

**Example searches:**
```bash
# Search by order UID for a specific network, excluding nginx
"expr": "NOT container:controller network:bnb ORDER_UID | sort by (_time) asc"

# Search by auction ID
"expr": "NOT container:controller 22788649 | sort by (_time) asc"

# Search by solver name + auction
"expr": "NOT container:controller baseline 22788649 | sort by (_time) asc"

# Filter by log content on specific network
"expr": "NOT container:controller network:mainnet order cancelled | sort by (_time) asc"

# Search by request_id to trace quote→bid issues (useful when order was placed with quote from solver X but that solver never bid)
"expr": "NOT container:controller $REQUEST_ID | sort by (_time) asc"
```

**Useful filters:**
- `NOT container:controller` — excludes nginx access logs (REQUIRED for order UID searches)
- `network:$NETWORK` — filter by chain (works for some log sources, not all)

**Note:** Victoria Logs uses simple text matching. Always use the **full order UID with 0x prefix** for reliable matching.

**IMPORTANT - Run targeted lifecycle queries:** Always use `NOT container:controller` to exclude nginx:

```bash
# Run these queries IN PARALLEL to quickly find all lifecycle events (use FULL order UID with 0x):
"expr": "NOT container:controller order created ORDER_UID | sort by (_time) asc"
"expr": "NOT container:controller order cancelled ORDER_UID | sort by (_time) asc"
"expr": "NOT container:controller proposed solution ORDER_UID | sort by (_time) asc"
"expr": "NOT container:controller settlement failed ORDER_UID | sort by (_time) asc"
"expr": "NOT container:controller filtered ORDER_UID | sort by (_time) asc"

# Find discarded solutions where order appears in calldata (use regex with order UID bytes without 0x prefix)
"expr": "discarded .*ORDER_UID_WITHOUT_0X.* | sort by (_time) asc"
```

**What to look for:**
- `order created` — order placement with quote_id
- `New orders in auction` — order entered auction
- `computed solutions` — solver found a route
- `solved auction` — solver submitted winning bid
- `filtered out in-flight` — order being settled
- `order cancelled` — user cancelled via API

**Get auction competition data:**
```bash
curl -s "https://api.cow.fi/$NETWORK/api/v1/solver_competition/$AUCTION_ID" | jq .
```

---

## 4. Common Log Patterns

**IMPORTANT:** Many log messages use **spaces** not underscores (e.g., `order cancelled` not `order_cancelled`).

**Order lifecycle (search by order UID):**
```
orderbook::api::post_order: order created           # Order placed
autopilot::run_loop::observe: New orders in auction # Added to auction
driver::infra::observe: computed solutions          # Solver found route
driver::infra::observe: solved auction              # Solver won
autopilot::run_loop: filtered out in-flight         # Being settled
autopilot::run_loop: settlement failed              # Settlement failed (check err=)
orderbook::orderbook: order cancelled               # User cancelled via API
```

**Issues to watch for:**
- `order cancelled` — user cancelled the order (check timestamp vs settlement!)
- `settlement failed err=Timeout` — driver timed out during settlement
- `settlement failed` — settlement failed (other reasons)
- `filtered` — order excluded from auction (check reason)
- `error` or `Error` — something went wrong
- `revert` — simulation or settlement failed
- `insufficient_balance` / `insufficient_allowance` — user moved funds

---

## 5. Quote History

### Method 1: API response (easiest)
The order API response includes the quote that was used:

```bash
curl -s "https://api.cow.fi/$NETWORK/api/v1/orders/$ORDER_UID" | jq '.quote'
```

Returns:
```json
{
  "sellAmount": "4300531427036176000",
  "buyAmount": "16788289774218687968",
  "feeAmount": "3270684063997860",
  "solver": "0x3980daa7eaad0b7e0c53cfc5c2760037270da54d",
  "verified": true,
  ...
}
```

### Method 2: Database
```sql
SELECT q.id, q.sell_amount, q.buy_amount, q.gas_amount, q.solver, q.created
FROM quotes q
JOIN order_quotes oq ON oq.quote_id = q.id
WHERE oq.order_uid = '\x$ORDER_UID_HEX';
```

### Method 3: Logs (fallback)
Find the quote_id from the "order created" log:

```bash
"expr": "NOT container:controller order created ORDER_UID | sort by (_time) asc"
```

**Example log line:**
```
orderbook::api::post_order: order created order_uid=0x... quote_id=Some(2720468) quote_solver=Some(0x3980...)
```

Then search for quote details by ID:
```bash
"expr": "NOT container:controller $QUOTE_ID | sort by (_time) asc"
```

---

## 6. Quoting Deep Dive

Quotes determine the limit price users sign. Bad quotes = orders that can't fill.

### Quote Process

```
UI requests quote → Orderbook sends "fake auction" (single order, infinite slippage) to all solvers
                           ↓
                    Solvers return: exchange rate + calldata (recipe)
                           ↓
                    In parallel, orderbook also fetches:
                    - Gas price estimate
                    - Native price of sell token (to convert gas cost)
                    - Native price of buy token (needed for surplus scoring later)
                           ↓
                    Simulate winning solver's calldata → get gas units
                           ↓
                    network_fee = gas_units × gas_price / sell_token_native_price
                           ↓
                    Return quote with exchange rate + network fee
```

### Quote Types

| Type | Behavior | Use Case |
|------|----------|----------|
| **Fast** | Returns after first 3 solver responses, always unverified | UI responsiveness |
| **Optimal** | Waits for all solvers (5s timeout), attempts verification | Actual order placement |
| **Native** | Cached quote for "buy 0.1 ETH with token X" | Native price lookups |

**Verified vs Unverified:**
- Verified = simulation succeeded, high confidence quote is achievable
- Unverified = simulation failed or skipped, solver might have bad math

### Limit Price Calculation

```
min_buy_amount = (sell_amount - network_fee) × exchange_rate × (1 - slippage) × (1 - partner_fee)
```

**Smart slippage**: Smaller orders get higher slippage bc network fee dominates. A 10% gas price spike on a $10 order (where fee is ~$2) eats way more than on a $1M order.

---

## 7. Order Placement Validation

Orderbook rejects orders that have no chance of executing. Checks:

| Check | Failure Mode |
|-------|--------------|
| Signature valid | Bad sig, wrong signer |
| Balance sufficient | Fill-or-kill needs full amount, partial needs >0 |
| Approval set | Need approval on GPV2VaultRelayer (not settlement contract directly) |
| AppData pre-image exists | AppData JSON must be provided in full with order, or pre-image must be added to backend beforehand |
| Rate limit | Too many orders per trader |
| Quote attached + valid | If quote ID provided, must exist and match |

**If order placed without quote** (common for bots): Orderbook re-quotes to classify as market vs limit order.

---

## 8. Autopilot Filtering (Why Order Not In Auction)

Even after placement, autopilot filters orders each auction. Current filters:

| Filter | Why |
|--------|-----|
| Signature re-check | presign/eip1271 can become invalid |
| Balance re-check | User moved funds |
| Native price exists | Can't score surplus without ETH-denominated value |
| Fee policy applied | Protocol fee calculation |

**Mainnet currently has ~6000 orders in auction** — drivers also do their own prioritization/filtering.

---

## 9. Limit Order Specific Checks

### 9.1 Surplus Fee Validation

```bash
# From order JSON, verify:
surplusFee != null
surplusFeeTimestamp is within last 10 minutes
```

**If missing/stale, check surplus fee computation logs:**
```bash
# Victoria Logs query
"expr": "surplus_fee $ORDER_UID | sort by (_time) asc"
```

**Surplus fee error logs:**
```bash
"expr": "surplus_fee error | sort by (_time) asc"
```

### 9.2 Auction Filtering Check

```bash
# Check if order is in current auction:
curl -s "https://api.cow.fi/$NETWORK/api/v1/auction" | jq '.orders[] | select(.uid == "$ORDER_UID")'
```

If not present, order is filtered. Check filter logs:
```bash
# Victoria Logs query
"expr": "filtered $ORDER_UID | sort by (_time) asc"
```

**Common filter reasons:**
- `insufficient_balance`
- `insufficient_allowance`  
- `invalid_signature` (ERC-1271 state changed, presign cancelled)
- `pre_interaction_error`
- `no_native_price` (can't get ETH price for buy token)

### 9.3 Market Price Verification

Compute effective sell amount:
```
effectiveSellAmount = sellAmount - surplusFee
```

**For SELL orders:**
```bash
curl -s -X POST "https://barn.api.cow.fi/$NETWORK/api/v1/quote" \
  -H 'content-type: application/json' \
  -d '{
    "from": "$OWNER",
    "sellToken": "$SELL_TOKEN",
    "buyToken": "$BUY_TOKEN",
    "kind": "sell",
    "sellAmountAfterFee": "$EFFECTIVE_SELL_AMOUNT"
  }' | jq '.quote.buyAmount'
```
→ Order's `buyAmount` should be **less than** this quote.

**For BUY orders:**
```bash
curl -s -X POST "https://barn.api.cow.fi/$NETWORK/api/v1/quote" \
  -H 'content-type: application/json' \
  -d '{
    "from": "$OWNER",
    "sellToken": "$SELL_TOKEN",
    "buyToken": "$BUY_TOKEN",
    "kind": "buy",
    "buyAmountAfterFee": "$BUY_AMOUNT"
  }' | jq '.quote.sellAmount'
```
→ Order's `effectiveSellAmount` should be **greater than** this quote.

---

## 10. Settlement Flow (On-Chain)

When driver wins, it has 2-3 blocks to land the tx.

### Settlement Contract Execution Order

```solidity
settle(
    IERC20[] tokens,           // All tokens involved
    uint256[] clearingPrices,  // Exchange rates
    Trade[] trades,            // Orders being filled
    Interaction[][3] interactions  // [pre, main, post]
)
```

**Execution sequence:**
1. **Pre-interactions** — Solver prep + user pre-hooks (unstaking, approvals, etc)
2. **For each trade:**
   - Convert Trade → Order struct
   - Verify signature (presign/eip1271 checked NOW)
   - Compute transfer amounts
   - Update filledAmounts mapping (replay protection)
   - Transfer sell tokens INTO settlement contract
3. **Main interactions** — The actual swaps/routing (Uniswap calls, etc)
4. **Pay out** — Transfer buy tokens to receivers, enforce min amounts
5. **Post-interactions** — Solver cleanup + user post-hooks (bridging, etc)

### Driver Submission Behavior

- Uses private RPCs (MEV Blocker) to avoid failed tx costs + get MEV protection
- Gas bumps on each block if not included
- Monitors chain state, cancels if settlement becomes invalid (liquidity moved, etc)
- **Penalty** if solution proposed but not settled

---

## 11. Auction Runtime Issues

Order is in auction but still not matching?

**Auction orders log:**
```bash
# Victoria Logs query (just the auction ID number)
"expr": "$AUCTION_ID | sort by (_time) asc"
```

**Specific auction run:**
```bash
"expr": "$RUN_ID | sort by (_time) asc"
```

### JIT Orders & CoW AMMs

Solvers can inject "just-in-time" orders (e.g., from market makers). These normally don't count toward surplus scoring bc they're not public.

**Exception:** CoW AMM contracts are whitelisted — autopilot includes "surplus capturing JIT order owners" in auction. Orders from these contracts DO count for surplus.

If debugging a CoW AMM interaction, check if the AMM contract is in the whitelist.

---

## 12. Circuit Breaker Monitoring

The circuit breaker watches all on-chain settlements and compares against off-chain auction outcomes.

**It enforces:**
- Winning solver is actually the one settling
- Settled amounts match reported amounts
- No protocol violations

**Violations → solver jailed** (deny-listed until they contact team, explain, fix).

Check circuit breaker logs if solver claims they won but settlement didn't happen or was rejected.

---

## 13. DB Queries (Direct Access)

### Check order state in DB:
```sql
SELECT
  uid, creation_timestamp, owner, sell_token, buy_token,
  sell_amount, buy_amount, valid_to, kind, class,
  surplus_fee, surplus_fee_timestamp
FROM orders
WHERE uid = '\x$ORDER_UID_HEX';
```

### Check order lifecycle events:
The `order_events` table tracks order state changes. This is often the fastest way to understand what happened.

```sql
SELECT timestamp, label::text
FROM order_events
WHERE order_uid = '\x$ORDER_UID_HEX'
ORDER BY timestamp;
```

**Event labels:**
| Label | Meaning |
|-------|---------|
| `created` | Order was placed |
| `ready` | Order ready for auction inclusion |
| `considered` | Order was considered in an auction |
| `executing` | Order is being settled (in-flight) |
| `traded` | Order was filled on-chain |
| `cancelled` | User cancelled the order |
| `filtered` | Order was filtered out of auction |
| `invalid` | Order became invalid (balance/allowance/signature) |

**Example lifecycle:** `created` → `ready` → `considered` → `executing` → `traded`

### Check quotes for order:
```sql
SELECT 
  q.id, q.sell_token, q.buy_token, q.sell_amount, q.buy_amount,
  q.gas_amount, q.solver, q.created
FROM quotes q
JOIN order_quotes oq ON oq.quote_id = q.id
WHERE oq.order_uid = '\x$ORDER_UID_HEX';
```

### Check auction inclusion history:
```sql
SELECT 
  auction_id, order_uid, included, filtered_reason
FROM auction_orders
WHERE order_uid = '\x$ORDER_UID_HEX'
ORDER BY auction_id DESC
LIMIT 20;
```

### Check executed settlements:
```sql
SELECT
  tx_hash, solver, order_uid, executed_sell_amount, executed_buy_amount
FROM settlements s
JOIN trades t ON t.settlement_id = s.id
WHERE t.order_uid = '\x$ORDER_UID_HEX';
```

### Check presignature events (for presign orders):
```sql
SELECT block_number, signed
FROM presignature_events
WHERE order_uid = '\x$ORDER_UID_HEX'
ORDER BY block_number;
```
If `signed = false`, the user revoked their presignature on-chain.

---

## 14. AppData Deep Dive

AppData is a hash of a JSON document (the JSON must be provided in full or its pre-image registered beforehand). **Cannot be verified on-chain** (smart contract just sees hash), so all enforcement is off-chain/soft.

### Common AppData Fields

```json
{
  "version": "1.0.0",
  "metadata": {
    "partnerFee": {
      "recipient": "0x...",
      "bps": 30
    },
    "hooks": {
      "pre": [{ "target": "0x...", "callData": "0x...", "gasLimit": "100000" }],
      "post": [{ "target": "0x...", "callData": "0x...", "gasLimit": "100000" }]
    },
    "flashLoan": {
      "lender": "0x...",
      "token": "0x...",
      "amount": "1000000000000000000"
    }
  }
}
```

**Debug implications:**
- Partner fee misconfigured → order's effective price is wrong
- Pre-hook fails → settlement reverts at pre-interaction stage
- Post-hook fails → settlement reverts after swaps (user loses gas but trade doesn't complete)
- Flash loan hints help solver but don't guarantee execution

```bash
# Fetch appData content
curl -s "https://api.cow.fi/$NETWORK/api/v1/app_data/$APP_DATA_HASH"
```

---

## 15. Useful Links

| Resource | URL |
|----------|-----|
| Order Explorer | `https://explorer.cow.fi/orders/$ORDER_UID` |
| Grafana Logs (Victoria Logs) | `$GRAFANA_URL/explore` (see .env.claude) |
| API Docs | `https://api.cow.fi/docs/` |
| Block-to-Date | `https://etherscan.io/blockdateconverter` |
| Barn (Staging) | `https://barn.cow.fi` |
| Settlement Contract | `0x9008D19f58AAbD9eD0D60971565AA8510560ab41` |

---

## 16. Decision Tree

```
Order not matched?
│
├─ Is order in auction?
│  ├─ NO → Check autopilot logs for filter reason
│  │       → Common: balance, allowance, signature, no native price
│  │
│  └─ YES → Did solvers bid?
│           ├─ NO → Price probably out of market
│           │       → Verify with quote API
│           │       → Check price movement since quote
│           │
│           └─ YES → What happened to winning bid?
│                    → Check solver pod for revert/error
│                    → Get auction_id, check competition endpoint
│
├─ Is it a limit order?
│  └─ Has surplusFee? Is it fresh (<10min)?
│     → NO: Check surplus fee computation logs
│
├─ Check signing scheme
│  └─ presign/eip1271? → State may have changed since placement
│
└─ Check appData
   └─ Hooks defined? → Pre/post hook might be failing
```

---

## 17. Common Root Causes

| Symptom | Likely Cause | Fix |
|---------|--------------|-----|
| No surplusFee | Quote computation failed | Check estimator logs |
| surplusFee stale | Background task stuck | Escalate to #backend |
| Filtered: insufficient_balance | User moved funds | Wait for rebalance |
| Filtered: invalid_signature | ERC-1271/presign state changed | User must re-sign or re-presign |
| Filtered: no_native_price | Can't price buy token in ETH | Token has no liquidity path to ETH |
| No solver bids | Price out of market | User adjusts limit |
| Solver bid reverted | Liquidity changed between auction and settlement | Normal MEV/timing |
| Quote outlier | Single estimator gave bad price | Check if quote was verified |
| Unverified quote accepted | Simulation failed but UI showed price anyway | User signed bad limit price |
| Pre-hook revert | User's pre-hook call failed | Check hook calldata + target |
| Gas estimate too low | API gas estimation bug | Known issue, being fixed |
