# fee-engine-compiled

A prototype. It runs the `fee_engine` rules as native Rust closures and does
not use the CEL interpreter. The `cel` crate only parses the rules.

## The idea

When the rules load, we walk the CEL AST once and make one typed closure for
each expression:

```text
order.appData.appCode == "trust wallet"
        ↓
|input| Ok(input.order.app_data.app_code() == Some("trust wallet"))
```

Each order then calls the closures. There is no `cel::Value`, no order view
and no interpreter at run time:

- A field name becomes a direct struct read. A rule reads only the fields
  that it uses.
- A list becomes a `HashSet<Address>`, so `order.owner in partners` is one
  hash lookup.
- Constant parts are folded when the rules load. For example, a constant fee
  becomes a `FeeFactor` one time, not one time for each order.
- Type errors (`order.sellAmount == "x"`, an unknown field) are found when
  the rules load, not for each order.

We do not need CEL parity. The crate supports the subset of CEL that the
rules use. Anything else does not compile.

## Supported CEL

- Literals, list literals, `order.*` fields, `now`, the named lists.
- `has(...)`, `.?` with `orValue` and `hasValue`.
- `&&`, `||`, `!`, `?:`, `==`, `!=`, `<`, `<=`, `>`, `>=`, `in`.
- Number arithmetic `+ - * / %` and timestamp/duration arithmetic.
- `timestamp`, `duration`, `int`, `uint`, `double`, `decimal`.
- `lowerAscii`, `upperAscii`, `startsWith`, `endsWith`, `contains`.
- `math.greatest`, `math.least`, `math.abs`, `math.ceil`, `math.floor`,
  `math.round`, `math.trunc`.
- `volume`, `surplus`, `price_improvement`, `no_fee`.

Not supported: macros (`all`, `exists`, `map`, ...), maps and indexing.

## Exact numbers

There are no `f64` values:

- `uint` is `U256`: `sellAmount`, `buyAmount`, `validTo`, `validFrom`.
- `int` is `I256`: `ethflowData.userValidTo`.
- A number with a fraction is `rust_decimal::Decimal`. This includes literals
  such as `0.3` and `1e21`, and the fee arguments.

Values of different types:

- All comparisons are exact.
- A constant takes the type of the other side if its value does not change,
  so `order.validTo - 3600` stays a `uint`.
- Otherwise the value goes to the wider type (`uint` < `int` < decimal).
- All arithmetic is checked. Overflow, a `uint` below zero or a division by
  zero skips the rule for that order.

The CEL parser gives number literals as `f64`. So we read each literal again
from the rule source text. This fixes a real mainnet order: as doubles,
`sellAmount = 999999999999999984204664` and `1e24` are equal.

## Results

The bench uses a real mainnet auction (6,244 orders) and the prod buckets.
The values are µs per order (mean / best of 30 passes):

```text
Rules      Count   CEL engine      Compiled        Speedup
────────   ─────   ─────────────   ─────────────   ───────
ternary    5       4.32 / 4.03     0.127 / 0.112   34x
split      9       3.00 / 2.83     0.168 / 0.157   18x
extended   12      4.27 / 3.99     0.255 / 0.240   17x
```

The legacy fee policies take 0.75 µs per order. For all orders and all
rules, the results are the same as the CEL engine (0 mismatches).

## Run it

```sh
# Unit tests, and a differential test against fee_engine (5,000 orders).
cargo nextest run -p fee-engine-compiled

# The real-auction bench. B is a folder with the two JSON files.
FEE_BENCH_ORDERS=$B/mainnet_auction_orders.json \
FEE_BENCH_BUCKETS=$B/mainnet_buckets.json \
cargo nextest run --release -p fee-engine-compiled bench_real_auction \
    --run-ignored ignored-only --no-capture
```

## Limits

- `Decimal` holds values up to about 7.9e28. If a rule does decimal
  arithmetic on a larger amount (for example `sellAmount / 1e20`), the rule
  is skipped for that order. Comparisons do not have this limit.
- No `math.sqrt`.
- An address compares with a literal in any letter case.
- Partner fees are not capped. The crate only evaluates the rules.
- It is not connected to the autopilot or the orderbook.
