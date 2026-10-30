# fee-engine

Rule-based protocol fees. A `RuleSet` holds named lists and an ordered list
of rules. For every order every rule runs; the engine reports each rule whose
`when` is true, in rule order, with its fee. The caller picks which matches
to act on, e.g. the first.

## Rules

A rule is a `RuleDefinition`: a name and two [CEL](https://cel.dev)
expressions. `when` must return a bool. `fee` must return one fee or a list
of fees. `RuleSet::new` refuses the whole set when a rule name is empty or
repeats, a list takes a reserved name or an expression does not compile. A
rule whose expression fails at run time is skipped and reported to the
caller.

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
  as on every order the API accepts, and every timestamp is `now`. A fee
  computed from the order is checked only at those values:
  `volume(order.buyAmount / order.sellAmount)` passes, but on a real order
  it can give an out-of-range argument, and the rule is then skipped and
  reported.

```
name: Stablecoin swaps
when: order.sellToken in stablecoins && order.buyToken in stablecoins
fee:  volume(10)

name: Default
when: true
fee:  surplus(50, 1)
```

## The rules file

`load_rules_from_str(source)` reads a `RuleSetDefinition` from TOML:
`[lists]` holds named lists of strings and each `[[rule]]` table is a rule,
tried in file order. Unknown top-level and rule keys are refused; any key
under `[lists]` is a list name, so a list that no rule names loads without
error. The file goes through `RuleSet::new` and
`RuleSet::validate_on_sample_order`; any failure refuses the whole file as a
`LoadError`: `Toml` (does not parse, unknown key, missing field or a rule
field that is empty; names the line and column), `Rules` (a `CompileError`)
or `SampleOrder` (every `SampleOrderError`). A file without rules loads: no
order pays a protocol fee.

`load_rules_toml(RulesToml { lists, rules })` loads the same two tables as
raw `toml::Value`s, e.g. from a section of a host config, so the host need
not know their shape. `None` is an empty table, like an absent one in a
file. It refuses them as `load_rules_from_str` does, but without a line or
column: a malformed rule is a `MalformedRule` error that names the rule by
its number, and any other malformed value is a `Toml` error.

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

The fields of the API order that a fee can depend on, under the API names,
built field by field in `order_view.rs`. Amounts and prices are doubles,
whole numbers (`validTo`, `validFrom`, `ethflowData.userValidTo`) are
`uint`s and hex is lowercase. An order with every field set looks like this
(hashes shortened):

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
  "appData": "0x3a8f…09bc",
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

`inMarket` is `OrderView::in_market` as the host passes it. The engine does not
check it: the host must pass false when the order has no quote.

Execution state (`executed*`, `status`, `invalidated`, `availableBalance`,
`gasCost`), settlement fields (`signature`, `interactions`,
`settlementContract`), legacy fields (`class`, `isLiquidityOrder`) and the
app-data document (`fullAppData`) are not in the view. An unset `receiver`
is the zero address. Other optional fields are absent when unset, not
`null`: test them with `has(order.quote)`. `has` checks only its last
field, so `has(order.ethflowData.refundTxHash)` fails on an order without
`ethflowData`; write `has(order.ethflowData) &&
has(order.ethflowData.refundTxHash)`.

A `uint` compares with any number, but arithmetic needs `uint` operands,
so write `order.validTo - 3600u` or `int(order.validTo) - 3600`, not
`order.validTo - 3600`. `timestamp(int(order.validTo))` makes a timestamp
of it.

```
when: order.kind == "sell" && order.signingScheme == "eip1271" && order.sellAmount > 1e21
when: timestamp(int(order.validTo)) - now > duration("1h")
fee:  order.inMarket ? price_improvement(50, 1) : surplus(50, 1)
```

A hex literal compares equal to the view only in lowercase: write it so, or
call `lowerAscii()` on it. Addresses belong in lists, where case does not
matter.

## Outcome

`Evaluator::evaluate` returns an `Evaluation` with the matches and the
diagnostics, and logs nothing itself:

- `matched`: a `Match { rule, fees }` for every rule whose `when` held, in
  rule order. Empty means no rule matched, so the order pays no protocol
  fee. The caller decides which matches to act on.
- `skipped`: every rule that failed to evaluate, in rule order, each with
  the field (`when` or `fee`) and the `RuleError`. The caller decides how to
  log or count them.
