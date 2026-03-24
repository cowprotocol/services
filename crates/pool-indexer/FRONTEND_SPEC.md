# Pool Indexer — Frontend Developer Spec

## What is it?

`pool-indexer` is a self-hosted Rust microservice that replaces the Uniswap V3 subgraph on The Graph. It reads directly from an Ethereum RPC node, stores Uniswap V3 pool state in Postgres, and exposes a REST API.

**Why it exists:** The Graph has reliability issues (rate limits, outages, chain support gaps). This is a drop-in replacement with the same data, served reliably from our own infrastructure.

**Current scope:** Uniswap V3 only. Balancer V2 planned but not implemented.

---

## Base URL

```
http://<host>:7777
```

Default port is `7777`. Configured via `bind-address` in the service config.

---

## Endpoints

### `GET /health`

Liveness check.

**Response:** `200 OK` (no body)

---

### `GET /api/v1/uniswap/v3/pools`

Returns all known Uniswap V3 pools with their current state (price, liquidity, tick). Uses cursor-based pagination.

**Query parameters:**

| Name    | Type   | Required | Default | Description                                        |
|---------|--------|----------|---------|----------------------------------------------------|
| `after` | string | no       | —       | Cursor: the `id` (address) of the last seen pool   |
| `limit` | int    | no       | 1000    | Page size. Clamped to `[1, 5000]`                  |

**Response `200 OK`:**
```json
{
  "block_number": 21800000,
  "pools": [
    {
      "id": "0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8",
      "token0": {
        "id": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        "decimals": 6
      },
      "token1": {
        "id": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
        "decimals": 18
      },
      "fee_tier": "3000",
      "liquidity": "12345678901234",
      "sqrt_price": "1234567890123456789",
      "tick": -201234,
      "ticks": null
    }
  ],
  "next_cursor": "0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8"
}
```

**Field notes:**

- `block_number` — the latest fully-indexed finalized block. Use this to know how fresh the data is.
- `id`, `token0.id`, `token1.id` — checksummed Ethereum addresses (`0x`-prefixed hex).
- `fee_tier` — fee in parts-per-million as a string. Common values: `"500"` (0.05%), `"3000"` (0.3%), `"10000"` (1%).
- `liquidity` — current active liquidity as a decimal string (uint128). Can be large; treat as `BigInt` or use a big-number library.
- `sqrt_price` — `sqrtPriceX96` as a decimal string (uint160). This is the square root of the price in Q64.96 fixed-point format. To get human price: `(sqrtPrice / 2^96)^2`, then adjust for decimal differences between token0 and token1.
- `tick` — current tick index (signed int32). Determines the current price bucket.
- `ticks` — always `null` in this endpoint. Use the dedicated `/ticks` endpoint per pool.
- `next_cursor` — the address to pass as `after` for the next page. `null` when on the last page.

**Pagination example:**
```
GET /api/v1/uniswap/v3/pools?limit=500
→ { "next_cursor": "0xabc...", "pools": [...] }

GET /api/v1/uniswap/v3/pools?limit=500&after=0xabc...
→ { "next_cursor": null, "pools": [...] }
```

**Error responses:**
- `400 Bad Request` — `{"error": "invalid cursor"}` if `after` is not a valid address.
- `503 Service Unavailable` — indexer hasn't processed any blocks yet (cold start).
- `500 Internal Server Error` — unexpected DB error.

---

### `GET /api/v1/uniswap/v3/pools/{pool_address}/ticks`

Returns all active ticks for a specific pool. Ticks define the liquidity boundaries within the pool.

**Path parameter:**
- `pool_address` — checksummed or lowercase hex address (`0x...`)

**Response `200 OK`:**
```json
{
  "block_number": 21800000,
  "pool": "0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8",
  "ticks": [
    { "tick_idx": -887272, "liquidity_net": "1000000000000" },
    { "tick_idx": -201240, "liquidity_net": "-500000000000" },
    { "tick_idx": 887272,  "liquidity_net": "-1000000000000" }
  ]
}
```

**Field notes:**
- `ticks` — sorted ascending by `tick_idx`. Only active ticks are present; ticks with `liquidity_net == 0` are pruned automatically.
- `liquidity_net` — signed int128 as a decimal string. Positive means liquidity is added when the price crosses this tick going right (up); negative means liquidity is removed. A standard Uniswap V3 liquidity math library will consume this directly.
- `block_number` — same semantics as the pools endpoint.

**Error responses:**
- `400 Bad Request` — `{"error": "invalid pool address"}` if the path param isn't a valid address.
- `503 Service Unavailable` — not yet indexed.
- `500 Internal Server Error` — unexpected DB error.

---

## Data freshness

The service only indexes **finalized blocks** (no reorg handling). On mainnet, "finalized" lags the chain tip by ~2 epochs (~12 minutes). The `block_number` field in every response tells you exactly how current the data is.

---

## Key numbers to know

| Concept | Value |
|---|---|
| Default port | `7777` |
| Mainnet Uniswap V3 factory | `0x1F98431c8aD98523631AE4a59f267346ea31F984` |
| Fee tiers (ppm) | `100`, `500`, `3000`, `10000` |
| `sqrtPriceX96` precision | Q64.96 (divide by `2^96` to get the raw sqrt price) |
| `liquidity` type | uint128 (max ~3.4 × 10³⁸) |
| `liquidity_net` type | int128 (signed, max ~1.7 × 10³⁸) |
| Tick range | `[-887272, 887272]` |

---

## Notes for UI implementation

1. **Big numbers:** `liquidity`, `sqrt_price`, and `liquidity_net` are all returned as decimal strings because they exceed JavaScript's safe integer range. Use `BigInt` or a library like `ethers.js` / `viem` for arithmetic.
2. **Price from sqrtPriceX96:** `price = (sqrtPriceX96 / 2n**96n) ** 2`, then adjust: `humanPrice = price * 10n**(token0Decimals - token1Decimals)`.
3. **Pagination:** Fetch all pools by following `next_cursor` until it's `null`. The service supports up to 5000 per page.
4. **No `block` filter on queries:** Unlike the subgraph, these endpoints always return the latest indexed state — there is no point-in-time query by block number (despite `block` being mentioned in the original plan spec, it is not implemented in the current code).
5. **503 on cold start:** If the indexer just launched and hasn't finished its first block range, all endpoints return `503`. Implement a retry or loading state.
