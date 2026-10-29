# fee-engine

Rule-based protocol fees. A `RuleSet` holds named lists and an ordered list
of rules. For every order every rule runs; the engine reports each rule whose
`when` is true, in rule order, with its fee. The caller picks which matches
to act on, e.g. the first.

## Rules

A rule is a `RuleDefinition`: a name and two [CEL](https://cel.dev)
expressions. `when` must return a bool. `fee` must return one fee or a list
of fees. `RuleSet::new` refuses the whole set when a rule name repeats, a
list takes a reserved name or an expression does not compile. A rule whose
expression fails at run time is skipped and reported to the caller.

`RuleSet::validate_on_sample_order` is a separate pass: it runs both
expressions of every rule against a sample order (a default order with
every optional field present) and reports every `when` that does not return
a bool and every `fee` that does not return valid fees, so one run shows all
the mistakes. This catches a misspelt name or a bad fee argument such as
`volume(0.001)` at load time instead of on the first order.

The pass has limits:

- A ternary, `&&` or `||` runs only what the sample order reaches. The
  sample is out of market, so the bad argument in `order.inMarket ?
  volume(0.001) : volume(1)` is not found; a static check of every branch is
  planned.
- Every amount and price in the sample is 1, except `feeAmount`, which is 0
  as on every order the API accepts. A fee computed from the order is
  checked only at those values: `volume(order.buyAmount / order.sellAmount)`
  passes, but on a real order it can give an out-of-range argument, and the
  rule is then skipped and reported.

```
name: Stablecoin swaps
when: order.sellToken in stablecoins && order.buyToken in stablecoins
fee:  volume(10)

name: Default
when: true
fee:  surplus(50, 1)
```

## The rules file

`load_rules(source)` reads a set from TOML: `[lists]` holds named lists of
strings and each `[[rule]]` table is a rule, tried in file order. Unknown
top-level and rule keys are refused; any key under `[lists]` is a list
name, so a list that no rule names loads without error. The file goes
through `RuleSet::new` and `RuleSet::validate_on_sample_order`; any failure
refuses the whole file as a `LoadError`: `Toml` (does not parse, unknown key
or missing field; names the line and column), `Rules` (a `CompileError`) or
`SampleOrder` (every `SampleOrderError`).

```toml
[lists]
stablecoins = [
  "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", # USDC
  "0xdAC17F958D2ee523a2206206994597C13D831ec7", # USDT
]
jit_makers = ["0x1111111111111111111111111111111111111111"]

[[rule]]
name = "JIT makers"
when = 'order.owner in jit_makers'
fee  = 'no_fee()'

[[rule]]
name = "Stablecoin swaps"
when = 'order.sellToken in stablecoins && order.buyToken in stablecoins'
fee  = 'volume(10)'

[[rule]]
name = "Default"
when = 'true'
fee  = 'surplus(50, 1)'
```

`tests/mainnet.toml` is the mainnet policy as rules, with short token
lists. A fee decision is identified by `Match::rule.name()`. To resolve that
name to the rule text in git, the host must log which rules file it loaded.

## Names bound once

`RuleSet::evaluator` binds the names in this section once. Each order is
then evaluated in a child scope on top of them, which adds the names of the
next section.

### The CEL standard library

The operators (`==`, `&&`, `||`, `in`, `? :`, arithmetic), the standard
functions (`size`, `startsWith`, `endsWith`, `contains`, `matches`,
`timestamp`, `duration`, `string`, `int`) and the macros (`has`, `exists`,
`all`, `filter`, `map`). Optional values are off, so `a.?b` does not compile.

```
when: order.kind == "sell" && order.sellAmount > 1e21
fee:  surplus(order.partiallyFillable ? 40 : 50, 1)
```

### Fee functions

The only values a `fee` expression may return. Arguments are CEL expressions,
so a fee can depend on the order. A list stacks several fees. Every argument
must land on a grid of 1e-6 of the volume (two decimal places for `bps`,
four for a percent); a finer value is rejected, not rounded.

- `volume(bps)` — `bps / 10000` of the volume.
- `surplus(share, max_volume)` — `share` percent of the surplus over the limit
  price, at most `max_volume` percent of the volume.
- `price_improvement(share, max_volume)` — `share` percent of the improvement
  over the quote, with the same cap.
- `no_fee()` — no protocol fee.

```
fee: [volume(order.kind == "sell" ? 2 : 5), price_improvement(30, 0.5)]
fee: no_fee()
```

### The CEL math extension

`math.greatest`, `math.least`, `math.ceil`, `math.floor`, `math.round`,
`math.abs`, `math.sqrt` and the rest of the
[math extension](https://github.com/google/cel-spec/blob/master/doc/extensions/math.md).
`greatest` and `least` bound a computed fee inside the rule.

```
fee: volume(math.greatest(1.0, math.least(10.0, order.sellAmount / 1e20)))
```

### The CEL strings extension

`lowerAscii`, `upperAscii`, `split`, `join`, `substring`, `trim`, `replace`,
`format` and the rest of the
[strings extension](https://github.com/google/cel-spec/blob/master/doc/extensions/strings.md).
`lowerAscii` makes a pasted checksummed address compare equal to the
lowercase view.

```
when: order.sellToken == "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".lowerAscii()
```

### Lists

Each entry of the lists map is a CEL list of strings under its name. `in`
tests membership and the list macros work on it. Hex members are lowercased
when the set is built, as are the hex strings in `order`, so address case
does not matter. The names `order`, `now` and `math` are reserved.

```
when: order.sellToken in stablecoins && order.buyToken in stablecoins
when: jit_makers.exists(m, m == order.owner || m == order.receiver)
```

### `now`

The time passed to `RuleSet::evaluator` as a CEL timestamp, the same for
every order evaluated through it.
It compares with `timestamp("…")` and adds or subtracts a `duration("…")`.
Duration units are `h`, `m`, `s`, `ms`, `us` and `ns`; there is no `d`.

```
when: now < timestamp("2026-12-01T00:00:00Z")
when: now > timestamp(order.creationDate) + duration("24h")
```

## Names bound per order

`Evaluator::evaluate` binds these in a child scope and drops them after the
order.

### `order`

The fields of the API order that a fee can depend on, under the API names
except for the app data (see `order.appData`), built field by field in
`order_view/mod.rs`. Amounts and prices are doubles
and hex is lowercase. An order with every field set looks like this (hashes
shortened):

```json
{
  "uid": "0x5f2a…d1e6",
  "owner": "0x40a50cf069e992aa4536211b23f286ef88752187",
  "creationDate": "2026-10-28T14:03:11.520Z",
  "sellToken": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
  "buyToken": "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
  "receiver": "0x0000000000000000000000000000000000000000",
  "sellAmount": 2.5e9,
  "buyAmount": 6.21e17,
  "feeAmount": 0.0,
  "kind": "sell",
  "partiallyFillable": false,
  "sellTokenBalance": "erc20",
  "buyTokenBalance": "erc20",
  "signingScheme": "eip712",
  "validTo": 1793284991,
  "validFrom": 1793281391,
  "appDataHash": "0x3a8f…09bc",
  "appData": {
    "appCode": "CoW Swap",
    "metadata": {
      "signer": "0x40a50cf069e992aa4536211b23f286ef88752187",
      "replacedOrder": { "uid": "0x9b4c…e1f0" },
      "partnerFee": [
        { "volumeBps": 25, "recipient": "0x63695eee2c3141bde314c5a6f89b98e62808d716" },
        { "surplusBps": 50, "maxVolumeBps": 100, "recipient": "0x63695eee2c3141bde314c5a6f89b98e62808d716" },
        { "priceImprovementBps": 30, "maxVolumeBps": 90, "recipient": "0x63695eee2c3141bde314c5a6f89b98e62808d716" }
      ],
      "flashloan": {
        "liquidityProvider": "0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2",
        "protocolAdapter": "0x7d9c4dee56933151bc5c909cfe09def0d315cb4a",
        "receiver": "0x7d9c4dee56933151bc5c909cfe09def0d315cb4a",
        "token": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        "amount": 2.5e9
      }
    }
  },
  "fastPath": false,
  "inMarket": true,
  "quote": {
    "sellAmount": 2.5e9,
    "buyAmount": 6.24e17,
    "feeAmount": 1.83e6,
    "gasAmount": 145000.0,
    "gasPrice": 1.2e9,
    "sellTokenPrice": 2.5e8,
    "solver": "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
    "verified": true
  },
  "ethflowData": {
    "userValidTo": 1793284991,
    "refundTxHash": "0x7c1e…aa04",
    "isRefunded": true
  },
  "onchainOrderData": {
    "sender": "0x40a50cf069e992aa4536211b23f286ef88752187",
    "placementError": "invalidQuote"
  },
  "onchainUser": "0x40a50cf069e992aa4536211b23f286ef88752187"
}
```

`inMarket` is `Order::in_market` as the host passes it. The engine does not
check it: the host must pass false when the order has no quote.

Execution state (`executed*`, `status`, `invalidated`, `availableBalance`,
`gasCost`), settlement fields (`signature`, `interactions`,
`settlementContract`) and legacy fields (`class`, `isLiquidityOrder`) are
not in the view. An unset `receiver` is the zero address. Other optional
fields are absent when unset, not `null`: test them with `has(order.quote)`.

```
when: order.kind == "sell" && order.signingScheme == "eip1271" && order.sellAmount > 1e21
fee:  order.inMarket ? price_improvement(50, 1) : surplus(50, 1)
```

A hex literal compares equal to the view only in lowercase: write it so, or
call `lowerAscii()` on it. Addresses belong in lists, where case does not
matter.

### `order.appData`

`Order::app_data`: the app data a fee can depend on, which the host copies
from the protocol's parsed document, plus its `appCode`. The engine reads no
app-data text. The names
are those of an order-creation request: the document is `appData` and its
hash is `appDataHash`. The order the API returns calls the hash `appData`
and has the document as `fullAppData` text.

The order above shows every field. No other key of the document is here,
hooks and wrappers included. `partnerFee` is always present, empty when
unset; each entry has one of `volumeBps`, `surplusBps` or
`priceImprovementBps`. The host passes `AppData::default()` when the order
has no document or the protocol cannot parse it.

`metadata.enableFastPath` and `metadata.validFrom` are not here: read
`order.fastPath` and `order.validFrom`, which hold the values in effect.

```
when: has(order.appData.appCode) && order.appData.appCode == "CoW Swap"
when: order.appData.metadata.partnerFee.exists(f, has(f.volumeBps) && f.volumeBps > 10)
```

## Outcome

`Evaluator::evaluate` returns an `Evaluation` with the matches and the
diagnostics, and logs nothing itself:

- `matched`: a `Match { rule, fees }` for every rule whose `when` held, in
  rule order. Empty means no rule matched, so the order pays no protocol
  fee. The caller decides which matches to act on.
- `skipped`: every rule that failed to evaluate, in rule order, each with
  the field (`when` or `fee`) and the `RuleError`. The caller decides how to
  log or count them.
